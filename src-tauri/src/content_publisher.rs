use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::AppHandle;
use time::OffsetDateTime;

use crate::{
    manifest::{self, Manifest},
    models::GameProfile,
    publisher, storage,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentReleasePreview {
    pub preview_id: String,
    pub profile_id: String,
    pub repository: String,
    pub tag: String,
    pub manifest_url: String,
    pub manifest_path: String,
    pub modpack_version: String,
    pub bytes: u64,
    pub sha256: String,
    pub package_assets_preserved: usize,
    pub required_file_count: usize,
    pub rules_count: usize,
    pub changelog_count: usize,
    pub issues: Vec<String>,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentReleasePublication {
    pub profile_id: String,
    pub repository: String,
    pub tag: String,
    pub manifest_url: String,
    pub url: String,
    pub message: String,
}

#[derive(Debug, Clone)]
struct ContentReleasePlan {
    output_dir: PathBuf,
    manifest_path: PathBuf,
    manifest_sha256: String,
    manifest_file_name: String,
    profile_id: String,
    game: String,
    modpack_version: String,
    repository: String,
    tag: String,
    title: String,
    notes: String,
    manifest_url: String,
}

static CONTENT_RELEASE_PLANS: OnceLock<Mutex<HashMap<String, ContentReleasePlan>>> =
    OnceLock::new();

pub fn prepare(app: &AppHandle, profile_id: &str) -> Result<ContentReleasePreview, String> {
    let config = storage::load_or_create(app)?;
    let profile = config
        .profiles
        .iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| "That modpack profile does not exist".to_string())?;
    let loaded = manifest::load_for_profile(app, profile);
    let output_root = storage::data_dir(app)?.join("content-release-previews");
    prepare_at(
        profile,
        &loaded.manifest,
        &loaded.summary.errors,
        &output_root,
        OffsetDateTime::now_utc().unix_timestamp(),
        true,
    )
}

pub fn publish(preview_id: &str, confirmed: bool) -> Result<ContentReleasePublication, String> {
    if !confirmed {
        return Err("Content-only release publication requires explicit confirmation".into());
    }
    let plan = content_release_plans()
        .lock()
        .map_err(|_| "Content release preview cache is unavailable".to_string())?
        .get(preview_id)
        .cloned()
        .ok_or_else(|| {
            "Prepare a fresh content-only release preview before publishing".to_string()
        })?;
    validate_plan(&plan)?;

    let status = publisher::status();
    if !status.gh_available || !status.authenticated {
        return Err(status.message);
    }
    let existing = publisher::run_gh([
        "release",
        "view",
        plan.tag.as_str(),
        "--repo",
        plan.repository.as_str(),
    ])?;
    if existing.status.success() {
        return Err(format!(
            "Release {} already exists in {}; content release tags are immutable",
            plan.tag, plan.repository
        ));
    }
    let lookup_message = publisher::output_message(&existing, "GitHub release lookup failed");
    let lookup_lower = lookup_message.to_ascii_lowercase();
    if !lookup_lower.contains("not found") && !lookup_lower.contains("no release found") {
        return Err(format!(
            "Could not prove that content release {} is absent: {lookup_message}",
            plan.tag
        ));
    }

    let manifest_path = plan.manifest_path.to_string_lossy().to_string();
    let output = publisher::run_gh([
        "release",
        "create",
        plan.tag.as_str(),
        manifest_path.as_str(),
        "--repo",
        plan.repository.as_str(),
        "--title",
        plan.title.as_str(),
        "--notes",
        plan.notes.as_str(),
        "--latest",
    ])?;
    if !output.status.success() {
        return Err(publisher::output_message(
            &output,
            "GitHub CLI could not create the content-only release",
        ));
    }
    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    content_release_plans()
        .lock()
        .map_err(|_| "Content release preview cache is unavailable".to_string())?
        .remove(preview_id);
    Ok(ContentReleasePublication {
        profile_id: plan.profile_id,
        repository: plan.repository,
        tag: plan.tag,
        manifest_url: plan.manifest_url,
        url,
        message: "Published the reviewed manifest as the latest release without uploading any modpack package asset.".into(),
    })
}

fn prepare_at(
    profile: &GameProfile,
    source_manifest: &Manifest,
    source_errors: &[String],
    output_root: &Path,
    unix_timestamp: i64,
    remember_plan: bool,
) -> Result<ContentReleasePreview, String> {
    let (repository, manifest_file_name) = match release_destination(profile) {
        Ok(destination) => destination,
        Err(error) => {
            return Ok(blocked_preview(profile, vec![error]));
        }
    };
    let mut issues = source_errors.to_vec();
    if source_manifest.modpack_version != profile.required_modpack_version {
        issues.push(format!(
            "The local trusted manifest is for modpack version {}, but the profile expects {}; refresh or publish the current package first",
            source_manifest.modpack_version, profile.required_modpack_version
        ));
    }
    issues.extend(distribution_issues(source_manifest));
    issues.sort();
    issues.dedup();
    if !issues.is_empty() {
        let mut preview = blocked_preview(profile, issues);
        preview.repository = repository;
        preview.manifest_url = profile.manifest_url.clone();
        return Ok(preview);
    }

    let mut bytes = serde_json::to_vec_pretty(source_manifest)
        .map_err(|error| format!("Could not serialize the content manifest: {error}"))?;
    bytes.push(b'\n');
    let parsed: Manifest = serde_json::from_slice(&bytes)
        .map_err(|error| format!("Could not re-open the content manifest: {error}"))?;
    let validation = manifest::validate(&parsed, Some(profile));
    if !validation.is_empty() {
        return Ok(blocked_preview(profile, validation));
    }
    let sha256 = format!("{:x}", Sha256::digest(&bytes));
    let tag = format!("content-{unix_timestamp}-{}", &sha256[..10]);
    let preview_id = format!("{:x}", Sha256::digest(format!("{tag}:{sha256}")))[..16].to_string();
    let output_dir = output_root.join(&preview_id);
    fs::create_dir_all(&output_dir)
        .map_err(|error| format!("Could not create the content preview folder: {error}"))?;
    let output_dir = fs::canonicalize(&output_dir)
        .map_err(|error| format!("Could not resolve the content preview folder: {error}"))?;
    let manifest_path = output_dir.join(&manifest_file_name);
    fs::write(&manifest_path, &bytes)
        .map_err(|error| format!("Could not write the content manifest preview: {error}"))?;

    let package_assets_preserved = if source_manifest.update_parts.is_empty() {
        usize::from(!source_manifest.update_url.trim().is_empty())
    } else {
        source_manifest.update_parts.len()
    };
    let preview = ContentReleasePreview {
        preview_id: preview_id.clone(),
        profile_id: profile.id.clone(),
        repository: repository.clone(),
        tag: tag.clone(),
        manifest_url: profile.manifest_url.clone(),
        manifest_path: manifest_path.display().to_string(),
        modpack_version: source_manifest.modpack_version.clone(),
        bytes: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        sha256: sha256.clone(),
        package_assets_preserved,
        required_file_count: source_manifest.files.len(),
        rules_count: source_manifest.rules_guide.rules.len(),
        changelog_count: source_manifest.changelog.len(),
        issues: Vec::new(),
        ready: true,
    };
    if remember_plan {
        content_release_plans()
            .lock()
            .map_err(|_| "Content release preview cache is unavailable".to_string())?
            .insert(
                preview_id,
                ContentReleasePlan {
                    output_dir,
                    manifest_path,
                    manifest_sha256: sha256,
                    manifest_file_name,
                    profile_id: profile.id.clone(),
                    game: profile.game.clone(),
                    modpack_version: source_manifest.modpack_version.clone(),
                    repository,
                    tag,
                    title: format!("{} content update", profile.display_name),
                    notes: format!(
                        "News, rules and changelog update for {} modpack version {}. Existing package assets are referenced unchanged.",
                        profile.display_name, source_manifest.modpack_version
                    ),
                    manifest_url: profile.manifest_url.clone(),
                },
            );
    }
    Ok(preview)
}

fn blocked_preview(profile: &GameProfile, issues: Vec<String>) -> ContentReleasePreview {
    ContentReleasePreview {
        preview_id: String::new(),
        profile_id: profile.id.clone(),
        repository: String::new(),
        tag: String::new(),
        manifest_url: profile.manifest_url.clone(),
        manifest_path: String::new(),
        modpack_version: String::new(),
        bytes: 0,
        sha256: String::new(),
        package_assets_preserved: 0,
        required_file_count: 0,
        rules_count: 0,
        changelog_count: 0,
        issues,
        ready: false,
    }
}

fn release_destination(profile: &GameProfile) -> Result<(String, String), String> {
    let remainder = profile
        .manifest_url
        .strip_prefix("https://github.com/")
        .ok_or_else(|| {
            "Content-only publication requires the profile's exact GitHub latest-release manifest URL"
                .to_string()
        })?;
    let parts = remainder.split('/').collect::<Vec<_>>();
    if parts.len() != 6 || parts[2] != "releases" || parts[3] != "latest" || parts[4] != "download"
    {
        return Err(
            "Manifest URL must use https://github.com/owner/repository/releases/latest/download/profile-manifest.json"
                .into(),
        );
    }
    let repository = format!("{}/{}", parts[0], parts[1]);
    publisher::validate_repository_name(&repository)?;
    let expected_file_name = format!("{}-manifest.json", profile.id);
    if parts[5] != expected_file_name {
        return Err(format!(
            "Manifest URL must end with the profile asset {expected_file_name}"
        ));
    }
    Ok((repository, expected_file_name))
}

fn distribution_issues(manifest: &Manifest) -> Vec<String> {
    let mut issues = Vec::new();
    let single = !manifest.update_url.trim().is_empty();
    let multipart = !manifest.update_parts.is_empty();
    if single == multipart {
        issues.push(
            "A published manifest must reference either one package URL or multipart package assets"
                .into(),
        );
    }
    if single && !manifest.update_url.starts_with("https://") {
        issues.push("The preserved package URL must use HTTPS".into());
    }
    if multipart
        && manifest
            .update_parts
            .iter()
            .any(|part| !part.url.starts_with("https://"))
    {
        issues.push("Every preserved multipart package URL must use HTTPS".into());
    }
    if manifest.update_sha256.trim().is_empty() {
        issues.push("The published package SHA-256 is missing".into());
    }
    issues
}

fn validate_plan(plan: &ContentReleasePlan) -> Result<(), String> {
    let path = fs::canonicalize(&plan.manifest_path)
        .map_err(|error| format!("The reviewed content manifest is unavailable: {error}"))?;
    if !path.starts_with(&plan.output_dir)
        || !path.is_file()
        || path.file_name().and_then(|name| name.to_str()) != Some(&plan.manifest_file_name)
    {
        return Err("The reviewed content manifest escaped its native preview folder".into());
    }
    let bytes = fs::read(&path)
        .map_err(|error| format!("Could not re-read the reviewed content manifest: {error}"))?;
    if format!("{:x}", Sha256::digest(&bytes)) != plan.manifest_sha256 {
        return Err("The reviewed content manifest changed after preview; prepare it again".into());
    }
    let manifest: Manifest = serde_json::from_slice(&bytes)
        .map_err(|error| format!("The reviewed content manifest is invalid JSON: {error}"))?;
    let errors = manifest::validate(&manifest, None);
    if !errors.is_empty()
        || manifest.profile_id != plan.profile_id
        || manifest.game != plan.game
        || manifest.modpack_version != plan.modpack_version
        || !distribution_issues(&manifest).is_empty()
    {
        return Err(
            "The reviewed content manifest no longer satisfies its native release plan".into(),
        );
    }
    Ok(())
}

fn content_release_plans() -> &'static Mutex<HashMap<String, ContentReleasePlan>> {
    CONTENT_RELEASE_PLANS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (GameProfile, Manifest) {
        let profile = crate::models::LauncherConfig::default().profiles.remove(0);
        let mut manifest: Manifest =
            serde_json::from_str(include_str!("../resources/manifests/minecraft_main.json"))
                .unwrap();
        manifest.announcement = "A real content update".into();
        manifest.rules_guide.rules = vec!["Be kind".into()];
        (profile, manifest)
    }

    #[test]
    fn content_preview_contains_only_a_manifest_and_preserves_package_references() {
        let root = tempfile::tempdir().unwrap();
        let (profile, manifest) = fixture();
        let preview =
            prepare_at(&profile, &manifest, &[], root.path(), 1_788_134_400, false).unwrap();
        assert!(preview.ready, "{:?}", preview.issues);
        assert_eq!(preview.package_assets_preserved, 1);
        assert_eq!(preview.required_file_count, 2_067);
        assert_eq!(preview.rules_count, 1);
        assert_eq!(preview.changelog_count, 0);
        let reviewed: Manifest =
            serde_json::from_slice(&fs::read(&preview.manifest_path).unwrap()).unwrap();
        assert_eq!(reviewed.update_url, manifest.update_url);
        assert_eq!(reviewed.update_sha256, manifest.update_sha256);
        assert_eq!(reviewed.files.len(), manifest.files.len());
        assert_eq!(reviewed.announcement, "A real content update");
    }

    #[test]
    fn draft_without_published_package_assets_is_blocked() {
        let root = tempfile::tempdir().unwrap();
        let (profile, mut manifest) = fixture();
        manifest.update_url.clear();
        manifest.update_sha256.clear();
        let preview = prepare_at(&profile, &manifest, &[], root.path(), 1, false).unwrap();
        assert!(!preview.ready);
        assert!(
            preview
                .issues
                .iter()
                .any(|issue| issue.contains("package URL"))
        );
        assert!(preview.issues.iter().any(|issue| issue.contains("SHA-256")));
    }

    #[test]
    fn destination_is_derived_from_the_exact_profile_manifest_url() {
        let (mut profile, _) = fixture();
        assert_eq!(
            release_destination(&profile).unwrap().0,
            "HixxyDubz/Mythic-Loot-Minecraft-Modpack"
        );
        profile.manifest_url = "https://example.com/manifest.json".into();
        assert!(release_destination(&profile).is_err());
        profile.manifest_url =
            "https://github.com/owner/repo/releases/latest/download/other.json".into();
        assert!(release_destination(&profile).is_err());
    }

    #[test]
    fn publication_is_fail_closed_before_github_is_called() {
        assert!(
            publish("not-a-preview", false)
                .unwrap_err()
                .contains("explicit confirmation")
        );
    }

    #[test]
    fn reviewed_manifest_is_rehashed_before_any_github_call() {
        let root = tempfile::tempdir().unwrap();
        let (profile, manifest) = fixture();
        let preview = prepare_at(&profile, &manifest, &[], root.path(), 42, true).unwrap();
        fs::write(&preview.manifest_path, b"tampered").unwrap();
        let error = publish(&preview.preview_id, true).unwrap_err();
        assert!(error.contains("changed after preview"));
    }
}
