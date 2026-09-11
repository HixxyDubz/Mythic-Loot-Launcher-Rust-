use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::{
    manifest::{self, ChangelogEntry, LoadedManifest, Manifest, RulesGuide},
    models::GameProfile,
    remote, safe_path, storage,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestContentInput {
    pub announcement: String,
    pub news_banner_url: String,
    pub rules_guide: RulesGuide,
    pub changelog: Vec<ChangelogEntry>,
}

pub fn save_for_profile(
    app: &AppHandle,
    profile: &GameProfile,
    content: ManifestContentInput,
) -> Result<bool, String> {
    save_at(
        &storage::data_dir(app)?,
        profile,
        manifest::load_for_profile(app, profile),
        content,
    )
}

pub(crate) fn draft_path(root: &Path, profile: &GameProfile) -> Result<PathBuf, String> {
    crate::validate_profile_id(&profile.id)?;
    safe_path::safe_join(root, &format!("content-drafts/{}.json", profile.id))
}

pub fn load_authoring(app: &AppHandle, profile: &GameProfile) -> LoadedManifest {
    let published = manifest::load_for_profile(app, profile);
    match storage::data_dir(app) {
        Ok(root) => load_authoring_at(&root, profile, published),
        Err(error) => manifest::invalid_loaded(profile, error),
    }
}

fn authoring_base(
    root: &Path,
    profile: &GameProfile,
    loaded: LoadedManifest,
) -> Result<Manifest, String> {
    let published_path = safe_path::safe_join(root, &profile.manifest_path)?;
    safe_path::reject_link_path(&published_path)?;
    if loaded.summary.valid {
        return Ok(loaded.manifest);
    }
    // Only new, unpublished profiles may start empty. Never conceal a damaged local manifest.
    if !published_path.exists() {
        return draft_manifest(profile);
    }
    Err(format!(
        "The existing local manifest is invalid and was left unchanged: {}",
        loaded.summary.errors.join("; ")
    ))
}

fn load_authoring_at(
    root: &Path,
    profile: &GameProfile,
    published: LoadedManifest,
) -> LoadedManifest {
    let source = published.summary.source.clone();
    let result = (|| {
        let path = draft_path(root, profile)?;
        safe_path::reject_link_path(&path)?;
        if !path
            .try_exists()
            .map_err(|error| format!("Could not inspect content draft: {error}"))?
        {
            return Ok(None);
        }
        if fs::metadata(&path)
            .map_err(|error| error.to_string())?
            .len()
            > 8 * 1024 * 1024
        {
            return Err("The saved content draft exceeds the size limit".into());
        }
        let content = serde_json::from_slice::<ManifestContentInput>(
            &fs::read(&path).map_err(|error| error.to_string())?,
        )
        .map_err(|error| format!("Could not read content draft: {error}"))?;
        Ok(Some((path, content)))
    })();
    match result {
        Ok(None) => published,
        Ok(Some((path, content))) => {
            let candidate = authoring_base(root, profile, published).and_then(|mut candidate| {
                apply_content(&mut candidate, profile, content)?;
                Ok(candidate)
            });
            match candidate {
                Ok(manifest) => LoadedManifest {
                    summary: manifest::summarize(
                        &manifest,
                        format!("Local content draft {} over {source}", path.display()),
                        Vec::new(),
                    ),
                    manifest,
                },
                Err(error) => manifest::invalid_loaded(profile, error),
            }
        }
        Err(error) => manifest::invalid_loaded(profile, error),
    }
}

fn save_at(
    root: &Path,
    profile: &GameProfile,
    published: LoadedManifest,
    content: ManifestContentInput,
) -> Result<bool, String> {
    let mut candidate = authoring_base(root, profile, published)?;
    apply_content(&mut candidate, profile, content.clone())?;
    // Persist only presentation fields, never inventory or download URLs that a refresh may change.
    let path = draft_path(root, profile)?;
    safe_path::reject_link_path(&path)?;
    let mut bytes = serde_json::to_vec_pretty(&content).map_err(|error| error.to_string())?;
    bytes.push(b'\n');
    remote::write_atomic(&path, &bytes)
}

fn draft_manifest(profile: &GameProfile) -> Result<Manifest, String> {
    if profile.required_modpack_version.trim().is_empty() {
        return Err("Set a modpack version before saving public content".into());
    }
    Ok(Manifest {
        manifest_version: "1.0".into(),
        profile_id: profile.id.clone(),
        game: profile.game.clone(),
        display_name: profile.display_name.clone(),
        required_game_version: profile.required_game_version.clone(),
        modpack_version: profile.required_modpack_version.clone(),
        discord_invite: profile.discord_invite.clone(),
        ..Manifest::default()
    })
}

fn apply_content(
    manifest: &mut Manifest,
    profile: &GameProfile,
    content: ManifestContentInput,
) -> Result<(), String> {
    let mut candidate = manifest.clone();
    candidate.announcement = content.announcement;
    candidate.news_banner_url = content.news_banner_url;
    candidate.rules_guide = content.rules_guide;
    candidate.changelog = content.changelog;
    let errors = manifest::validate(&candidate, Some(profile));
    if !errors.is_empty() {
        return Err(format!(
            "Content changes failed manifest validation: {}",
            errors.join("; ")
        ));
    }
    *manifest = candidate;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{FileEntry, UpdatePart};

    fn profile() -> GameProfile {
        let mut profile = crate::models::LauncherConfig::default().profiles.remove(0);
        profile.id = "fixture".into();
        profile.game = "minecraft".into();
        profile
    }

    fn manifest() -> Manifest {
        Manifest {
            manifest_version: "1.0".into(),
            profile_id: "fixture".into(),
            game: "minecraft".into(),
            modpack_version: "4.2.0".into(),
            update_url: "https://example.invalid/pack.zip".into(),
            update_sha256: "a".repeat(64),
            update_parts: vec![UpdatePart {
                url: "https://example.invalid/pack.zip.001".into(),
                sha256: "b".repeat(64),
                size: 42,
            }],
            files: vec![FileEntry {
                path: "mods/example.jar".into(),
                size: 42,
                hash: "c".repeat(64),
                download_url: "https://example.invalid/example.jar".into(),
                required: true,
                category: "mods".into(),
            }],
            obsolete_files: vec!["mods/old.jar".into()],
            ..Manifest::default()
        }
    }

    fn content() -> ManifestContentInput {
        ManifestContentInput {
            announcement: "A real announcement".into(),
            news_banner_url: "https://example.invalid/banner.webp".into(),
            rules_guide: RulesGuide {
                how_to_join: "Install the current pack.".into(),
                rules: vec!["Be kind.".into()],
                common_fixes: vec!["Run Repair.".into()],
            },
            changelog: vec![ChangelogEntry {
                version: "4.2.0".into(),
                date: "2026-08-31".into(),
                added: vec!["New balance pass".into()],
                changed: Vec::new(),
                fixed: vec!["Startup issue".into()],
                notes: "Release notes".into(),
            }],
        }
    }

    #[test]
    fn content_changes_preserve_every_distribution_field() {
        let mut manifest = manifest();
        let before = serde_json::json!({
            "manifestVersion": manifest.manifest_version,
            "profileId": manifest.profile_id,
            "game": manifest.game,
            "displayName": manifest.display_name,
            "requiredGameVersion": manifest.required_game_version,
            "modpackVersion": manifest.modpack_version,
            "updateUrl": manifest.update_url,
            "updateSha256": manifest.update_sha256,
            "updateParts": manifest.update_parts,
            "releaseDate": manifest.release_date,
            "discordInvite": manifest.discord_invite,
            "newsBannerPath": manifest.news_banner_path,
            "minecraftBaseModLoader": manifest.minecraft_base_mod_loader,
            "minecraftInstanceName": manifest.minecraft_instance_name,
            "files": manifest.files,
            "obsoleteFiles": manifest.obsolete_files,
            "optionalFiles": manifest.optional_files,
        });
        apply_content(&mut manifest, &profile(), content()).unwrap();
        let after = serde_json::json!({
            "manifestVersion": manifest.manifest_version,
            "profileId": manifest.profile_id,
            "game": manifest.game,
            "displayName": manifest.display_name,
            "requiredGameVersion": manifest.required_game_version,
            "modpackVersion": manifest.modpack_version,
            "updateUrl": manifest.update_url,
            "updateSha256": manifest.update_sha256,
            "updateParts": manifest.update_parts,
            "releaseDate": manifest.release_date,
            "discordInvite": manifest.discord_invite,
            "newsBannerPath": manifest.news_banner_path,
            "minecraftBaseModLoader": manifest.minecraft_base_mod_loader,
            "minecraftInstanceName": manifest.minecraft_instance_name,
            "files": manifest.files,
            "obsoleteFiles": manifest.obsolete_files,
            "optionalFiles": manifest.optional_files,
        });
        assert_eq!(before, after);
        assert_eq!(manifest.announcement, "A real announcement");
        assert_eq!(manifest.rules_guide.rules, vec!["Be kind."]);
    }

    #[test]
    fn unsafe_banner_and_oversized_content_are_rejected_without_mutation() {
        let mut manifest = manifest();
        let original = manifest.clone();
        let mut input = content();
        input.news_banner_url = "http://example.invalid/banner.png".into();
        input.announcement = "x".repeat(20_001);
        let error = apply_content(&mut manifest, &profile(), input).unwrap_err();
        assert!(error.contains("newsBannerUrl uses an unsupported URL scheme"));
        assert!(error.contains("announcement exceeds 20000 characters"));
        assert_eq!(manifest.announcement, original.announcement);
        assert_eq!(manifest.update_url, original.update_url);
    }

    #[test]
    fn drafts_use_real_profile_identity_before_the_first_package_release() {
        let profile = profile();
        let manifest = draft_manifest(&profile).unwrap();
        assert_eq!(manifest.profile_id, profile.id);
        assert_eq!(manifest.game, profile.game);
        assert_eq!(manifest.modpack_version, profile.required_modpack_version);
        assert!(manifest::validate(&manifest, Some(&profile)).is_empty());
    }

    fn loaded(manifest: Manifest) -> LoadedManifest {
        LoadedManifest {
            summary: manifest::summarize(&manifest, "published".into(), Vec::new()),
            manifest,
        }
    }

    #[test]
    fn refresh_keeps_draft_presentation_and_uses_new_distribution() {
        let root = tempfile::TempDir::new().unwrap();
        let profile = profile();
        let published_path = safe_path::safe_join(root.path(), &profile.manifest_path).unwrap();
        remote::write_atomic(&published_path, &serde_json::to_vec(&manifest()).unwrap()).unwrap();
        let old_bytes = fs::read(&published_path).unwrap();
        save_at(root.path(), &profile, loaded(manifest()), content()).unwrap();
        assert_eq!(
            fs::read(&published_path).unwrap(),
            old_bytes,
            "Saving a draft must not edit the downloaded manifest"
        );
        let mut refreshed = manifest();
        refreshed.modpack_version = "5.0.0".into();
        refreshed.update_url = "https://example.invalid/new.zip".into();
        refreshed.files[0].hash = "d".repeat(64);
        refreshed.announcement = "Published older news".into();
        remote::write_atomic(&published_path, &serde_json::to_vec(&refreshed).unwrap()).unwrap();
        let authoring = load_authoring_at(root.path(), &profile, loaded(refreshed));
        assert!(authoring.summary.valid, "{:?}", authoring.summary.errors);
        assert_eq!(authoring.manifest.announcement, "A real announcement");
        assert_eq!(authoring.manifest.modpack_version, "5.0.0");
        assert_eq!(
            authoring.manifest.update_url,
            "https://example.invalid/new.zip"
        );
        assert_eq!(authoring.manifest.files[0].hash, "d".repeat(64));
        let raw_draft = fs::read_to_string(draft_path(root.path(), &profile).unwrap()).unwrap();
        assert!(!raw_draft.contains("updateUrl"));
        assert!(!raw_draft.contains("optionalFiles"));
    }

    #[test]
    fn invalid_draft_is_reported_instead_of_silently_discarded() {
        let root = tempfile::TempDir::new().unwrap();
        let profile = profile();
        let draft = draft_path(root.path(), &profile).unwrap();
        remote::write_atomic(&draft, b"broken json").unwrap();
        let result = load_authoring_at(root.path(), &profile, loaded(manifest()));
        assert!(!result.summary.valid);
        assert!(result.summary.errors[0].contains("draft"));
        assert_eq!(fs::read(draft).unwrap(), b"broken json");
    }
}
