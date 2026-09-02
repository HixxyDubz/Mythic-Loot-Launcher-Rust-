use std::{
    collections::HashMap,
    env,
    ffi::OsStr,
    fs::{self, File, Metadata},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

#[cfg(windows)]
use std::os::windows::fs::MetadataExt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::AppHandle;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{manifest, publisher, self_update, storage};

const BUILD_MANIFEST_MAX_BYTES: u64 = 1024 * 1024;
const RELEASE_NOTES_MAX_CHARS: usize = 4_000;
const MAX_CACHED_PREVIEWS: usize = 4;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppReleaseRequest {
    pub build_manifest_path: String,
    pub release_notes: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppReleaseAssetPreview {
    pub file_name: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppReleasePreview {
    pub preview_id: String,
    pub repository: String,
    pub tag: String,
    pub version: String,
    pub release_notes: String,
    pub feed_url: String,
    pub output_directory: String,
    pub assets: Vec<AppReleaseAssetPreview>,
    pub ready: bool,
    pub issues: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppReleasePublication {
    pub repository: String,
    pub tag: String,
    pub version: String,
    pub url: String,
    pub assets: usize,
    pub message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WindowsBuildManifest {
    product: String,
    version: String,
    editions: Vec<String>,
    artifacts: Vec<WindowsBuildArtifact>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WindowsBuildArtifact {
    edition: String,
    kind: String,
    path: String,
    bytes: u64,
    sha256: String,
}

#[derive(Debug, Clone)]
struct AppReleasePlan {
    preview_id: String,
    version: String,
    tag: String,
    title: String,
    notes: String,
    output_directory: PathBuf,
    executable_path: PathBuf,
    executable_sha256: String,
    installer_path: PathBuf,
    installer_sha256: String,
    feed_path: PathBuf,
    feed_sha256: String,
}

static RELEASE_PLANS: OnceLock<Mutex<HashMap<String, AppReleasePlan>>> = OnceLock::new();

pub fn prepare(app: &AppHandle, request: &AppReleaseRequest) -> Result<AppReleasePreview, String> {
    let manifest_path = resolve_manifest_path(&request.build_manifest_path)?;
    let output_root = storage::data_dir(app)?.join("app-update-release-previews");
    let published_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| format!("Could not format the app release timestamp: {error}"))?;
    prepare_at(
        &manifest_path,
        &output_root,
        &request.release_notes,
        &published_at,
        true,
    )
}

pub fn publish(preview_id: &str, confirmed: bool) -> Result<AppReleasePublication, String> {
    if !confirmed {
        return Err("Public Player app release publication requires explicit confirmation".into());
    }
    let plan = release_plans()
        .lock()
        .map_err(|_| "App release preview cache is unavailable".to_string())?
        .get(preview_id)
        .cloned()
        .ok_or_else(|| "Prepare a fresh Player app release before publishing".to_string())?;
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
        self_update::APP_UPDATE_REPOSITORY,
    ])?;
    if existing.status.success() {
        return Err(format!(
            "Immutable app release {} already exists; bump the app version and rebuild",
            plan.tag
        ));
    }
    let existing_message = publisher::output_message(&existing, "GitHub release lookup failed");
    let lower = existing_message.to_ascii_lowercase();
    if !lower.contains("not found") && !lower.contains("could not resolve") {
        return Err(format!(
            "Could not safely confirm that the app release tag is unused: {existing_message}"
        ));
    }

    let arguments = [
        "release".to_string(),
        "create".to_string(),
        plan.tag.clone(),
        plan.executable_path.to_string_lossy().to_string(),
        plan.installer_path.to_string_lossy().to_string(),
        plan.feed_path.to_string_lossy().to_string(),
        "--repo".to_string(),
        self_update::APP_UPDATE_REPOSITORY.to_string(),
        "--title".to_string(),
        plan.title.clone(),
        "--notes".to_string(),
        plan.notes.clone(),
        "--latest".to_string(),
    ];
    let output = publisher::run_gh(arguments.iter().map(String::as_str))?;
    if !output.status.success() {
        return Err(publisher::output_message(
            &output,
            "GitHub could not publish the Player app release",
        ));
    }
    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    release_plans()
        .lock()
        .map_err(|_| "App release preview cache is unavailable".to_string())?
        .remove(preview_id);
    Ok(AppReleasePublication {
        repository: self_update::APP_UPDATE_REPOSITORY.into(),
        tag: plan.tag,
        version: plan.version,
        url,
        assets: 3,
        message: "The immutable Player executable, installer and checksum-protected latest feed were published.".into(),
    })
}

fn prepare_at(
    manifest_path: &Path,
    output_root: &Path,
    release_notes: &str,
    published_at: &str,
    remember: bool,
) -> Result<AppReleasePreview, String> {
    if release_notes.chars().count() > RELEASE_NOTES_MAX_CHARS {
        return Err(format!(
            "App release notes must be {RELEASE_NOTES_MAX_CHARS} characters or fewer"
        ));
    }
    let build = load_build_manifest(manifest_path)?;
    let clean_version = self_update::parse_version(&build.version)?.to_string();
    let current_version = self_update::parse_version(env!("CARGO_PKG_VERSION"))?;
    let mut issues = Vec::new();
    if build.product != "Mythic Loot Launcher" {
        issues.push("Windows build manifest belongs to another product".into());
    }
    if !build.editions.iter().any(|edition| edition == "player") {
        issues.push("Windows build manifest does not include the Player edition".into());
    }
    if self_update::parse_version(&clean_version)? != current_version {
        issues.push(format!(
            "Built Player version {clean_version} does not match this Developer edition {}",
            env!("CARGO_PKG_VERSION")
        ));
    }
    let executable = find_artifact(&build, "win-unpacked");
    let installer = find_artifact(&build, "installer");
    if executable.is_none() {
        issues.push("Windows build manifest has no unique Player win-unpacked executable".into());
    }
    if installer.is_none() {
        issues.push("Windows build manifest has no unique Player installer".into());
    }
    if !issues.is_empty() {
        return Ok(blocked_preview(&clean_version, release_notes, issues));
    }
    let executable = executable.expect("checked above");
    let installer = installer.expect("checked above");
    verify_artifact(executable)?;
    verify_artifact(installer)?;

    let fingerprint = format!(
        "{}:{}:{}:{}",
        clean_version,
        executable.sha256.to_ascii_lowercase(),
        installer.sha256.to_ascii_lowercase(),
        release_notes.trim()
    );
    let digest = format!("{:x}", Sha256::digest(fingerprint.as_bytes()));
    let preview_id = digest[..16].to_string();
    let output_directory = output_root.join(&preview_id);
    create_safe_directory(&output_directory)?;
    let output_directory = fs::canonicalize(&output_directory)
        .map_err(|error| format!("Could not resolve app release preview folder: {error}"))?;
    let executable_path = output_directory.join(self_update::APP_UPDATE_ASSET_NAME);
    let installer_path = output_directory.join(self_update::APP_UPDATE_INSTALLER_NAME);
    copy_verified(Path::new(&executable.path), &executable_path, executable)?;
    copy_verified(Path::new(&installer.path), &installer_path, installer)?;
    let executable_sha256 = manifest::sha256(&executable_path)?;
    let installer_sha256 = manifest::sha256(&installer_path)?;
    let feed = self_update::build_player_feed(
        &clean_version,
        release_notes,
        published_at,
        executable.bytes,
        &executable_sha256,
    )?;
    let mut feed_bytes = serde_json::to_vec_pretty(&feed)
        .map_err(|error| format!("Could not serialize Player app update feed: {error}"))?;
    feed_bytes.push(b'\n');
    self_update::parse_feed(&feed_bytes)?;
    let feed_path = output_directory.join(self_update::APP_UPDATE_FEED_NAME);
    let mut feed_file = File::create(&feed_path)
        .map_err(|error| format!("Could not create Player app update feed: {error}"))?;
    feed_file
        .write_all(&feed_bytes)
        .and_then(|_| feed_file.sync_all())
        .map_err(|error| format!("Could not write Player app update feed: {error}"))?;
    let feed_sha256 = manifest::sha256(&feed_path)?;
    let plan = AppReleasePlan {
        preview_id: preview_id.clone(),
        version: clean_version.clone(),
        tag: format!("v{clean_version}"),
        title: format!("Mythic Loot Launcher {clean_version}"),
        notes: if release_notes.trim().is_empty() {
            format!("Mythic Loot Launcher Player {clean_version}")
        } else {
            release_notes.trim().into()
        },
        output_directory: output_directory.clone(),
        executable_path: executable_path.clone(),
        executable_sha256: executable_sha256.clone(),
        installer_path: installer_path.clone(),
        installer_sha256: installer_sha256.clone(),
        feed_path: feed_path.clone(),
        feed_sha256: feed_sha256.clone(),
    };
    if remember {
        let mut plans = release_plans()
            .lock()
            .map_err(|_| "App release preview cache is unavailable".to_string())?;
        if plans.len() >= MAX_CACHED_PREVIEWS {
            plans.clear();
        }
        plans.insert(preview_id.clone(), plan);
    }
    Ok(AppReleasePreview {
        preview_id,
        repository: self_update::APP_UPDATE_REPOSITORY.into(),
        tag: format!("v{clean_version}"),
        version: clean_version,
        release_notes: release_notes.trim().into(),
        feed_url: self_update::APP_UPDATE_FEED_URL.into(),
        output_directory: output_directory.display().to_string(),
        assets: vec![
            AppReleaseAssetPreview {
                file_name: self_update::APP_UPDATE_ASSET_NAME.into(),
                bytes: executable.bytes,
                sha256: executable_sha256,
            },
            AppReleaseAssetPreview {
                file_name: self_update::APP_UPDATE_INSTALLER_NAME.into(),
                bytes: installer.bytes,
                sha256: installer_sha256,
            },
            AppReleaseAssetPreview {
                file_name: self_update::APP_UPDATE_FEED_NAME.into(),
                bytes: u64::try_from(feed_bytes.len()).unwrap_or(u64::MAX),
                sha256: feed_sha256,
            },
        ],
        ready: true,
        issues: Vec::new(),
        message: "The exact Player executable, installer and checksum-protected feed are staged for review. GitHub is unchanged.".into(),
    })
}

fn blocked_preview(version: &str, release_notes: &str, issues: Vec<String>) -> AppReleasePreview {
    AppReleasePreview {
        preview_id: String::new(),
        repository: self_update::APP_UPDATE_REPOSITORY.into(),
        tag: format!("v{version}"),
        version: version.into(),
        release_notes: release_notes.trim().into(),
        feed_url: self_update::APP_UPDATE_FEED_URL.into(),
        output_directory: String::new(),
        assets: Vec::new(),
        ready: false,
        issues,
        message: "The Player app release is blocked until every build artifact is verified.".into(),
    }
}

fn validate_plan(plan: &AppReleasePlan) -> Result<(), String> {
    if plan.preview_id.is_empty()
        || plan.output_directory.file_name() != Some(OsStr::new(&plan.preview_id))
    {
        return Err("App release preview identity is invalid".into());
    }
    verify_staged(
        &plan.executable_path,
        &plan.executable_sha256,
        self_update::APP_UPDATE_ASSET_NAME,
    )?;
    verify_staged(
        &plan.installer_path,
        &plan.installer_sha256,
        self_update::APP_UPDATE_INSTALLER_NAME,
    )?;
    verify_staged(
        &plan.feed_path,
        &plan.feed_sha256,
        self_update::APP_UPDATE_FEED_NAME,
    )?;
    let feed_bytes = fs::read(&plan.feed_path)
        .map_err(|error| format!("Could not reopen reviewed app update feed: {error}"))?;
    let feed = self_update::parse_feed(&feed_bytes)?;
    let executable_bytes = plan
        .executable_path
        .metadata()
        .map_err(|error| format!("Could not inspect reviewed Player executable: {error}"))?
        .len();
    if feed.version != plan.version
        || feed.asset.sha256 != plan.executable_sha256
        || feed.asset.bytes != executable_bytes
    {
        return Err(
            "Reviewed app update feed no longer matches the staged Player executable".into(),
        );
    }
    Ok(())
}

fn resolve_manifest_path(value: &str) -> Result<PathBuf, String> {
    let path = if value.trim().is_empty() {
        env::current_dir()
            .map_err(|error| format!("Could not inspect the current project folder: {error}"))?
            .join("artifacts/windows/build-manifest.json")
    } else {
        PathBuf::from(value.trim())
    };
    if !path.is_absolute() {
        return Err(
            "Windows build manifest path must be absolute or blank for project auto-detection"
                .into(),
        );
    }
    Ok(path)
}

fn load_build_manifest(path: &Path) -> Result<WindowsBuildManifest, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("Could not inspect Windows build manifest: {error}"))?;
    if is_link_like(&metadata) || !metadata.is_file() || metadata.len() > BUILD_MANIFEST_MAX_BYTES {
        return Err("Windows build manifest is linked, missing or exceeds its safety limit".into());
    }
    let bytes = fs::read(path)
        .map_err(|error| format!("Could not read Windows build manifest: {error}"))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("Windows build manifest is invalid: {error}"))
}

fn find_artifact<'a>(
    build: &'a WindowsBuildManifest,
    kind: &str,
) -> Option<&'a WindowsBuildArtifact> {
    let mut matches = build
        .artifacts
        .iter()
        .filter(|artifact| artifact.edition == "player" && artifact.kind == kind);
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

fn verify_artifact(artifact: &WindowsBuildArtifact) -> Result<(), String> {
    let path = Path::new(&artifact.path);
    if !path.is_absolute() || artifact.bytes == 0 || !valid_sha256(&artifact.sha256) {
        return Err(format!(
            "Player {} artifact metadata is invalid",
            artifact.kind
        ));
    }
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        format!(
            "Could not inspect Player {} artifact: {error}",
            artifact.kind
        )
    })?;
    if is_link_like(&metadata)
        || !metadata.is_file()
        || metadata.len() != artifact.bytes
        || manifest::sha256(path)? != artifact.sha256.to_ascii_lowercase()
    {
        return Err(format!(
            "Player {} artifact failed regular-file, size or SHA-256 verification",
            artifact.kind
        ));
    }
    let mut file = File::open(path)
        .map_err(|error| format!("Could not open Player {} artifact: {error}", artifact.kind))?;
    let mut header = [0_u8; 2];
    file.read_exact(&mut header)
        .map_err(|error| format!("Could not read Player {} artifact: {error}", artifact.kind))?;
    if header != *b"MZ" {
        return Err(format!(
            "Player {} artifact is not a Windows executable",
            artifact.kind
        ));
    }
    Ok(())
}

fn copy_verified(
    source: &Path,
    destination: &Path,
    artifact: &WindowsBuildArtifact,
) -> Result<(), String> {
    remove_regular_if_exists(destination)?;
    fs::copy(source, destination)
        .map_err(|error| format!("Could not stage Player {} artifact: {error}", artifact.kind))?;
    if destination
        .metadata()
        .map_err(|error| format!("Could not inspect staged Player artifact: {error}"))?
        .len()
        != artifact.bytes
        || manifest::sha256(destination)? != artifact.sha256.to_ascii_lowercase()
    {
        fs::remove_file(destination).ok();
        return Err(format!(
            "Staged Player {} artifact failed verification",
            artifact.kind
        ));
    }
    Ok(())
}

fn verify_staged(path: &Path, sha256: &str, expected_name: &str) -> Result<(), String> {
    if path.file_name() != Some(OsStr::new(expected_name)) {
        return Err("Reviewed app release asset name changed".into());
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("Could not inspect reviewed app release asset: {error}"))?;
    if is_link_like(&metadata) || !metadata.is_file() || manifest::sha256(path)? != sha256 {
        return Err(format!(
            "Reviewed app release asset changed: {expected_name}"
        ));
    }
    Ok(())
}

fn create_safe_directory(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path)
        .map_err(|error| format!("Could not create app release preview folder: {error}"))?;
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("Could not inspect app release preview folder: {error}"))?;
    if is_link_like(&metadata) || !metadata.is_dir() {
        return Err("App release preview folder is linked, redirected or not a directory".into());
    }
    Ok(())
}

fn remove_regular_if_exists(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("Could not inspect prior app release asset: {error}"))?;
    if is_link_like(&metadata) || !metadata.is_file() {
        return Err("Prior app release asset is not a safe regular file".into());
    }
    fs::remove_file(path)
        .map_err(|error| format!("Could not clear prior app release asset: {error}"))
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn release_plans() -> &'static Mutex<HashMap<String, AppReleasePlan>> {
    RELEASE_PLANS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn is_link_like(metadata: &Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    fn fixture(root: &Path, valid_hash: bool) -> PathBuf {
        let player = root.join("Mythic Loot Launcher Player.exe");
        let installer = root.join("Mythic Loot Launcher Player Setup 0.1.0.exe");
        fs::write(&player, b"MZplayer").unwrap();
        fs::write(&installer, b"MZinstaller").unwrap();
        let path = root.join("build-manifest.json");
        let player_hash = if valid_hash {
            hash(b"MZplayer")
        } else {
            "0".repeat(64)
        };
        fs::write(
            &path,
            serde_json::to_vec_pretty(&serde_json::json!({
                "product": "Mythic Loot Launcher",
                "version": env!("CARGO_PKG_VERSION"),
                "editions": ["player", "developer"],
                "artifacts": [
                    {"edition":"player", "kind":"win-unpacked", "path":player, "bytes":8, "sha256":player_hash},
                    {"edition":"player", "kind":"installer", "path":installer, "bytes":11, "sha256":hash(b"MZinstaller")}
                ]
            }))
            .unwrap(),
        )
        .unwrap();
        path
    }

    #[test]
    fn prepares_exact_player_assets_and_checksum_feed() {
        let root = tempfile::tempdir().unwrap();
        let manifest = fixture(root.path(), true);
        let preview = prepare_at(
            &manifest,
            &root.path().join("out"),
            "Release notes",
            "2026-09-01T00:00:00Z",
            false,
        )
        .unwrap();
        assert!(preview.ready, "{:?}", preview.issues);
        assert_eq!(preview.assets.len(), 3);
        assert_eq!(preview.repository, self_update::APP_UPDATE_REPOSITORY);
        let feed =
            fs::read(Path::new(&preview.output_directory).join(self_update::APP_UPDATE_FEED_NAME))
                .unwrap();
        let feed = self_update::parse_feed(&feed).unwrap();
        assert_eq!(feed.asset.sha256, hash(b"MZplayer"));
        assert_eq!(feed.asset.bytes, 8);
    }

    #[test]
    fn wrong_build_manifest_hash_blocks_before_staging() {
        let root = tempfile::tempdir().unwrap();
        let manifest = fixture(root.path(), false);
        assert!(
            prepare_at(
                &manifest,
                &root.path().join("out"),
                "",
                "2026-09-01T00:00:00Z",
                false,
            )
            .is_err()
        );
    }

    #[test]
    fn publication_is_fail_closed_before_github() {
        assert!(
            publish("missing", false)
                .unwrap_err()
                .contains("confirmation")
        );
    }
}
