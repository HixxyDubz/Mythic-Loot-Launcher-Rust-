//! Developer-only, local presentation recovery. Never restores distribution inventory.
use crate::{
    content_editor::{self, ManifestContentInput},
    manifest::Manifest,
    models::GameProfile,
    remote, safe_path,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

const LIMIT: u64 = 8 * 1024 * 1024;

// A per-data-directory OS lock coordinates saves, recovery and refresh commits,
// including separate Developer processes. Network fetching happens before this lock.
pub fn lock(root: &Path) -> Result<File, String> {
    let path = root.join("content-authoring.lock");
    safe_path::reject_link_path(&path)?;
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|e| e.to_string())?;
    fs2::FileExt::try_lock_exclusive(&file).map_err(|_| {
        "Another content save, recovery or manifest refresh is active; try again".to_string()
    })?;
    Ok(file)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryCandidate {
    id: String,
    label: String,
    content: Option<ManifestContentInput>,
    problem: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentRecoveryState {
    pub profile_id: String,
    pub draft_revision: String,
    pub has_draft: bool,
    candidates: Vec<RecoveryCandidate>,
    limited: bool,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn history(root: &Path, profile: &GameProfile) -> Result<PathBuf, String> {
    crate::validate_profile_id(&profile.id)?;
    let path = root.join("content-recovery").join(&profile.id);
    safe_path::reject_link_path(&path)?;
    Ok(path)
}
fn read(path: &Path, limit: u64) -> Result<Option<Vec<u8>>, String> {
    safe_path::reject_link_path(path)?;
    let file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("Could not read local content; it was left unchanged".into()),
    };
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > limit {
        return Err(
            "Local content exceeds the recovery size limit or is not a regular file".into(),
        );
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("Local content grew beyond the recovery size limit".into());
    }
    Ok(Some(bytes))
}
fn revision(bytes: &Option<Vec<u8>>) -> String {
    bytes
        .as_ref()
        .map(|b| digest(b))
        .unwrap_or_else(|| "absent".into())
}
fn project(manifest: &Manifest) -> ManifestContentInput {
    ManifestContentInput {
        announcement: manifest.announcement.clone(),
        news_banner_url: manifest.news_banner_url.clone(),
        rules_guide: manifest.rules_guide.clone(),
        changelog: manifest.changelog.clone(),
    }
}
fn cached(root: &Path, profile: &GameProfile) -> Result<Option<Vec<u8>>, String> {
    let path = safe_path::safe_join(root, &profile.manifest_path)?;
    let Some(bytes) = read(&path, 64 * 1024 * 1024)? else {
        return Ok(None);
    };
    let manifest: Manifest = serde_json::from_slice(&bytes)
        .map_err(|_| "The cached manifest is damaged; it was preserved unchanged")?;
    if manifest.profile_id != profile.id || manifest.game != profile.game {
        return Err("Cached manifest identity does not match this profile".into());
    }
    let content = project(&manifest);
    content_editor::validate_content(profile, content.clone())?;
    let bytes = serde_json::to_vec_pretty(&content).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > LIMIT {
        return Err("Cached content exceeds the recovery limit".into());
    }
    Ok(Some(bytes))
}
fn archive(root: &Path, profile: &GameProfile, kind: &str, bytes: &[u8]) -> Result<String, String> {
    let id = format!("{kind}-{}", digest(bytes));
    let dir = history(root, profile)?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{id}.json"));
    if let Some(existing) = read(&path, LIMIT)? {
        if existing != bytes {
            return Err(
                "A content recovery copy is damaged; existing files were not overwritten".into(),
            );
        }
        return Ok(id);
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| format!("Could not preserve content recovery copy: {e}"))?;
    Ok(id)
}

// Call with the authoring lock held. Failure to preserve aborts replacement.
pub fn draft_matches(root: &Path, profile: &GameProfile, bytes: &[u8]) -> Result<bool, String> {
    Ok(read(&content_editor::draft_path(root, profile)?, LIMIT)?.as_deref() == Some(bytes))
}

pub fn archive_draft(root: &Path, profile: &GameProfile) -> Result<(), String> {
    if let Some(bytes) = read(&content_editor::draft_path(root, profile)?, LIMIT)? {
        archive(root, profile, "draft", &bytes)?;
    }
    Ok(())
}

pub fn write_published(root: &Path, profile: &GameProfile, bytes: &[u8]) -> Result<bool, String> {
    let _lock = lock(root)?;
    let path = safe_path::safe_join(root, &profile.manifest_path)?;
    if read(&path, 64 * 1024 * 1024)?.as_deref() == Some(bytes) {
        return Ok(false);
    }
    if let Some(content) = cached(root, profile)? {
        archive(root, profile, "legacy", &content)?;
    }
    for suffix in ["download", "previous"] {
        safe_path::reject_link_path(&path.with_file_name(format!(
            "{}.{}",
            path.file_name().unwrap().to_string_lossy(),
            suffix
        )))?;
    }
    remote::write_atomic(&path, bytes)
}

fn candidate(id: String, bytes: &[u8], profile: &GameProfile) -> RecoveryCandidate {
    let parsed = serde_json::from_slice::<ManifestContentInput>(bytes)
        .map_err(|_| {
            "This saved draft is damaged; its exact bytes remain in local recovery storage"
                .to_string()
        })
        .and_then(|content| {
            content_editor::validate_content(profile, content.clone())?;
            Ok(content)
        });
    let label = if id.starts_with("draft-") {
        "Previous or discarded local draft"
    } else {
        "Cached manifest content (may already have been published)"
    };
    match parsed {
        Ok(content) => RecoveryCandidate {
            id,
            label: label.into(),
            content: Some(content),
            problem: None,
        },
        Err(problem) => RecoveryCandidate {
            id,
            label: label.into(),
            content: None,
            problem: Some(problem),
        },
    }
}
fn valid_id(id: &str) -> bool {
    ["draft-", "legacy-", "cached-"].iter().any(|prefix| {
        id.strip_prefix(prefix).is_some_and(|hash| {
            hash.len() == 64
                && hash
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        })
    })
}
fn candidate_bytes(root: &Path, profile: &GameProfile, id: &str) -> Result<Vec<u8>, String> {
    if !valid_id(id) {
        return Err("Choose a recognised content recovery entry".into());
    }
    let bytes = if id.starts_with("cached-") {
        cached(root, profile)?
    } else {
        read(&history(root, profile)?.join(format!("{id}.json")), LIMIT)?
    }
    .ok_or("The recovery entry is no longer available; review again")?;
    if !id.ends_with(&digest(&bytes)) {
        return Err("Recovery content changed or is damaged; review again".into());
    }
    Ok(bytes)
}

pub fn inspect(root: &Path, profile: &GameProfile) -> Result<ContentRecoveryState, String> {
    let _lock = lock(root)?;
    let draft = read(&content_editor::draft_path(root, profile)?, LIMIT)?;
    let mut candidates = Vec::new();
    // No embedded fallback is represented as a recovered local edit.
    match cached(root, profile) {
        Ok(Some(bytes)) => candidates.push(candidate(
            format!("cached-{}", digest(&bytes)),
            &bytes,
            profile,
        )),
        Ok(None) => {}
        Err(error) => candidates.push(RecoveryCandidate {
            id: "unavailable".into(),
            label: "Cached manifest unavailable".into(),
            content: None,
            problem: Some(error),
        }),
    }
    let directory = history(root, profile)?;
    let mut limited = false;
    if directory.try_exists().map_err(|e| e.to_string())? {
        let mut entries = Vec::new();
        for (index, entry) in fs::read_dir(directory)
            .map_err(|e| e.to_string())?
            .take(1001)
            .enumerate()
        {
            if index == 1000 {
                limited = true;
                break;
            }
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(id) = name
                .strip_suffix(".json")
                .filter(|id| valid_id(id) && !id.starts_with("cached-"))
            {
                entries.push((
                    entry.metadata().and_then(|m| m.modified()).ok(),
                    id.to_string(),
                ));
            }
        }
        if entries.len() > 50 {
            limited = true;
        }
        entries.sort_by(|a, b| b.cmp(a));
        let mut budget = LIMIT;
        for (_, id) in entries.into_iter().take(50) {
            match candidate_bytes(root, profile, &id) {
                Ok(bytes) if bytes.len() as u64 <= budget => {
                    budget -= bytes.len() as u64;
                    candidates.push(candidate(id, &bytes, profile));
                }
                Ok(_) => {
                    limited = true;
                    break;
                }
                Err(problem) => candidates.push(RecoveryCandidate {
                    id,
                    label: "Unavailable recovery entry".into(),
                    content: None,
                    problem: Some(problem),
                }),
            }
        }
    }
    Ok(ContentRecoveryState {
        profile_id: profile.id.clone(),
        draft_revision: revision(&draft),
        has_draft: draft.is_some(),
        candidates,
        limited,
    })
}

// Caller holds the authoring lock until the mutation completes.
pub fn check_review(
    root: &Path,
    profile: &GameProfile,
    expected: &str,
    confirmed: bool,
) -> Result<(), String> {
    if !confirmed {
        return Err("Content recovery/discard requires explicit confirmation".into());
    }
    if revision(&read(&content_editor::draft_path(root, profile)?, LIMIT)?) != expected {
        return Err(
            "The saved draft changed since review; review again before replacing it".into(),
        );
    }
    Ok(())
}
pub fn recover_content(
    root: &Path,
    profile: &GameProfile,
    id: &str,
) -> Result<ManifestContentInput, String> {
    let bytes = candidate_bytes(root, profile, id)?;
    let content: ManifestContentInput = serde_json::from_slice(&bytes)
        .map_err(|_| "Damaged recovery entries cannot be applied automatically")?;
    content_editor::validate_content(profile, content.clone())?;
    Ok(content)
}
pub fn discard(root: &Path, profile: &GameProfile) -> Result<bool, String> {
    let path = content_editor::draft_path(root, profile)?;
    if read(&path, LIMIT)?.is_none() {
        return Ok(false);
    }
    archive_draft(root, profile)?;
    fs::remove_file(path)
        .map_err(|e| format!("Draft preserved but could not be discarded: {e}"))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{self, LoadedManifest};
    fn profile() -> GameProfile {
        let mut p = crate::models::LauncherConfig::default().profiles.remove(0);
        p.id = "recovery_fixture".into();
        p.manifest_path = "manifests/recovery_fixture.json".into();
        p.required_modpack_version = "1.0.0".into();
        p
    }
    fn content(text: &str) -> ManifestContentInput {
        ManifestContentInput {
            announcement: text.into(),
            news_banner_url: String::new(),
            rules_guide: Default::default(),
            changelog: vec![],
        }
    }
    fn published(p: &GameProfile, text: &str) -> Manifest {
        Manifest {
            manifest_version: "1.0".into(),
            profile_id: p.id.clone(),
            game: p.game.clone(),
            modpack_version: "2.0.0".into(),
            announcement: text.into(),
            update_url: "https://example.invalid/current.zip".into(),
            update_sha256: "a".repeat(64),
            ..Manifest::default()
        }
    }
    fn loaded(m: Manifest) -> LoadedManifest {
        LoadedManifest {
            summary: manifest::summarize(&m, "fixture".into(), vec![]),
            manifest: m,
        }
    }
    fn put(root: &Path, p: &GameProfile, m: &Manifest) -> Vec<u8> {
        let bytes = serde_json::to_vec(m).unwrap();
        remote::write_atomic(&root.join(&p.manifest_path), &bytes).unwrap();
        bytes
    }

    #[test]
    fn refresh_preserves_legacy_presentation_without_distribution_and_deduplicates() {
        let root = tempfile::tempdir().unwrap();
        let p = profile();
        put(root.path(), &p, &published(&p, "Old local edits"));
        let latest = serde_json::to_vec(&published(&p, "Published news")).unwrap();
        assert!(write_published(root.path(), &p, &latest).unwrap());
        assert!(!write_published(root.path(), &p, &latest).unwrap());
        let state = inspect(root.path(), &p).unwrap();
        let legacy = state
            .candidates
            .iter()
            .find(|c| c.id.starts_with("legacy-"))
            .unwrap();
        assert_eq!(
            legacy.content.as_ref().unwrap().announcement,
            "Old local edits"
        );
        let backup = candidate_bytes(root.path(), &p, &legacy.id).unwrap();
        assert!(!String::from_utf8(backup).unwrap().contains("updateUrl"));
        assert_eq!(
            fs::read(root.path().join(&p.manifest_path)).unwrap(),
            latest
        );
        assert_eq!(
            fs::read_dir(history(root.path(), &p).unwrap())
                .unwrap()
                .count(),
            1
        );
    }

    #[test]
    fn recovery_uses_current_inventory_and_retains_replaced_draft() {
        let root = tempfile::tempdir().unwrap();
        let p = profile();
        let current = published(&p, "Current news");
        let before = put(root.path(), &p, &current);
        content_editor::save_at(
            root.path(),
            &p,
            loaded(current.clone()),
            content("Existing draft"),
        )
        .unwrap();
        let id = archive(
            root.path(),
            &p,
            "legacy",
            &serde_json::to_vec(&content("Recovered old news")).unwrap(),
        )
        .unwrap();
        let state = inspect(root.path(), &p).unwrap();
        check_review(root.path(), &p, &state.draft_revision, true).unwrap();
        let recovered = recover_content(root.path(), &p, &id).unwrap();
        content_editor::save_at(root.path(), &p, loaded(current), recovered).unwrap();
        assert_eq!(
            fs::read(root.path().join(&p.manifest_path)).unwrap(),
            before
        );
        let draft =
            fs::read_to_string(content_editor::draft_path(root.path(), &p).unwrap()).unwrap();
        assert!(draft.contains("Recovered old news"));
        assert!(!draft.contains("updateUrl"));
        assert!(
            inspect(root.path(), &p)
                .unwrap()
                .candidates
                .iter()
                .any(|c| c
                    .content
                    .as_ref()
                    .is_some_and(|c| c.announcement == "Existing draft"))
        );
    }

    #[test]
    fn discard_is_recoverable_including_malformed_drafts() {
        for bytes in [
            serde_json::to_vec(&content("Discard me")).unwrap(),
            b"invalid private-json".to_vec(),
        ] {
            let root = tempfile::tempdir().unwrap();
            let p = profile();
            remote::write_atomic(
                &content_editor::draft_path(root.path(), &p).unwrap(),
                &bytes,
            )
            .unwrap();
            assert!(discard(root.path(), &p).unwrap());
            assert!(!discard(root.path(), &p).unwrap());
            let state = inspect(root.path(), &p).unwrap();
            assert!(!state.has_draft);
            assert_eq!(state.candidates.len(), 1);
            assert_eq!(
                candidate_bytes(root.path(), &p, &state.candidates[0].id).unwrap(),
                bytes
            );
            if state.candidates[0].content.is_none() {
                assert!(
                    !serde_json::to_string(&state)
                        .unwrap()
                        .contains("private-json")
                );
                assert!(recover_content(root.path(), &p, &state.candidates[0].id).is_err());
            }
        }
    }

    #[test]
    fn stale_reviews_missing_confirmation_and_tampered_recovery_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        let p = profile();
        assert!(check_review(root.path(), &p, "absent", false).is_err());
        check_review(root.path(), &p, "absent", true).unwrap();
        remote::write_atomic(
            &content_editor::draft_path(root.path(), &p).unwrap(),
            b"new draft",
        )
        .unwrap();
        assert!(check_review(root.path(), &p, "absent", true).is_err());
        for id in ["../elsewhere", "draft-../../outside", "cached-deadbeef"] {
            assert!(recover_content(root.path(), &p, id).is_err());
        }
        let id = archive(
            root.path(),
            &p,
            "draft",
            &serde_json::to_vec(&content("reviewed")).unwrap(),
        )
        .unwrap();
        fs::write(
            history(root.path(), &p).unwrap().join(format!("{id}.json")),
            b"tampered",
        )
        .unwrap();
        assert!(recover_content(root.path(), &p, &id).is_err());
    }

    #[test]
    fn damaged_cache_blocks_refresh_and_damaged_backup_blocks_discard() {
        let root = tempfile::tempdir().unwrap();
        let p = profile();
        let path = root.path().join(&p.manifest_path);
        remote::write_atomic(&path, b"broken manifest").unwrap();
        assert!(
            write_published(
                root.path(),
                &p,
                &serde_json::to_vec(&published(&p, "new")).unwrap()
            )
            .is_err()
        );
        assert_eq!(fs::read(path).unwrap(), b"broken manifest");
        let bytes = b"broken draft";
        remote::write_atomic(&content_editor::draft_path(root.path(), &p).unwrap(), bytes).unwrap();
        let id = archive(root.path(), &p, "draft", bytes).unwrap();
        fs::write(
            history(root.path(), &p).unwrap().join(format!("{id}.json")),
            b"different",
        )
        .unwrap();
        assert!(discard(root.path(), &p).is_err());
        assert_eq!(
            fs::read(content_editor::draft_path(root.path(), &p).unwrap()).unwrap(),
            bytes
        );
    }

    #[test]
    fn changed_cached_candidate_is_rejected_and_history_is_bounded_without_deletion() {
        let root = tempfile::tempdir().unwrap();
        let p = profile();
        put(root.path(), &p, &published(&p, "Before review"));
        let reviewed = inspect(root.path(), &p).unwrap().candidates.remove(0).id;
        put(root.path(), &p, &published(&p, "After review"));
        assert!(recover_content(root.path(), &p, &reviewed).is_err());
        for number in 0..52 {
            archive(
                root.path(),
                &p,
                "draft",
                &serde_json::to_vec(&content(&format!("Draft {number}"))).unwrap(),
            )
            .unwrap();
        }
        let state = inspect(root.path(), &p).unwrap();
        assert!(state.limited);
        assert_eq!(state.candidates.len(), 51); // 50 archived plus current cached content
        assert_eq!(
            fs::read_dir(history(root.path(), &p).unwrap())
                .unwrap()
                .count(),
            52
        );
    }

    #[test]
    fn bounded_reads_identity_validation_and_lock_exclusion() {
        let root = tempfile::tempdir().unwrap();
        let p = profile();
        let mut wrong = published(&p, "wrong");
        wrong.profile_id = "another_profile".into();
        put(root.path(), &p, &wrong);
        assert!(cached(root.path(), &p).is_err());
        let oversized = root.path().join("oversized");
        File::create(&oversized)
            .unwrap()
            .set_len(LIMIT + 1)
            .unwrap();
        assert!(read(&oversized, LIMIT).is_err());
        let held = lock(root.path()).unwrap();
        assert!(lock(root.path()).is_err());
        drop(held);
        assert!(lock(root.path()).is_ok());
    }
}
