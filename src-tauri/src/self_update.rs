use std::{
    collections::HashMap,
    env,
    ffi::OsStr,
    fs::{self, File, Metadata},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::{Mutex, OnceLock},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[cfg(windows)]
use std::os::windows::fs::MetadataExt;

use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::AppHandle;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{manifest, remote, storage};

pub const APP_UPDATE_REPOSITORY: &str = "HixxyDubz/Mythic-Loot-Launcher-Rust-";
#[cfg_attr(not(feature = "developer"), allow(dead_code))]
pub const APP_UPDATE_FEED_NAME: &str = "launcher-update-player.json";
pub const APP_UPDATE_ASSET_NAME: &str = "Mythic-Loot-Launcher-Player.exe";
#[cfg_attr(not(feature = "developer"), allow(dead_code))]
pub const APP_UPDATE_INSTALLER_NAME: &str = "Mythic-Loot-Launcher-Player-Setup.exe";
pub const APP_UPDATE_FEED_URL: &str = "https://github.com/HixxyDubz/Mythic-Loot-Launcher-Rust-/releases/latest/download/launcher-update-player.json";
const APP_UPDATE_PRODUCT: &str = "Mythic Loot Launcher";
const APP_UPDATE_EDITION: &str = "player";
const UPDATE_STAGING_DIRECTORY: &str = "app-update-staging";
const UPDATE_RESULT_FILE: &str = "app-update-last-result.json";
const FEED_SCHEMA_VERSION: u32 = 1;
#[cfg_attr(not(feature = "developer"), allow(dead_code))]
const MINIMUM_DIRECT_UPDATE_VERSION: &str = "0.1.0";
const HELPER_SCHEMA_VERSION: u32 = 1;
const MAX_FEED_BYTES: usize = 256 * 1024;
const MAX_APP_BYTES: u64 = 256 * 1024 * 1024;
const MAX_CACHED_PLANS: usize = 4;
const PARENT_EXIT_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppUpdateAsset {
    pub file_name: String,
    pub url: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppUpdateFeed {
    pub schema_version: u32,
    pub product: String,
    pub edition: String,
    pub version: String,
    pub release_notes: String,
    pub published_at: String,
    pub mandatory: bool,
    pub minimum_supported_version: String,
    pub asset: AppUpdateAsset,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdatePreview {
    pub preview_id: String,
    pub feed_url: String,
    pub current_version: String,
    pub latest_version: String,
    pub release_notes: String,
    pub published_at: String,
    pub mandatory: bool,
    pub minimum_supported_version: String,
    pub asset_bytes: u64,
    pub asset_sha256: String,
    pub update_available: bool,
    pub supported: bool,
    pub can_install: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateStage {
    pub stage_id: String,
    pub version: String,
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
    pub ready: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateApplyOutcome {
    pub version: String,
    pub helper_started: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateResult {
    pub version: String,
    pub success: bool,
    pub restart_process_id: Option<u32>,
    pub message: String,
    pub recorded_at: i64,
}

#[derive(Debug, Clone)]
struct FeedPlan {
    feed: AppUpdateFeed,
}

#[derive(Debug, Clone)]
#[cfg_attr(debug_assertions, allow(dead_code))]
struct StagedPlan {
    feed: AppUpdateFeed,
    stage_id: String,
    stage_dir: PathBuf,
    staged_exe: PathBuf,
    target_exe: PathBuf,
    target_sha256: String,
    result_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HelperJournal {
    schema_version: u32,
    version: String,
    target_exe: String,
    target_sha256: String,
    staged_exe: String,
    staged_bytes: u64,
    staged_sha256: String,
    backup_exe: String,
    result_path: String,
    #[serde(default)]
    restart_probe: bool,
}

static FEED_PLANS: OnceLock<Mutex<HashMap<String, FeedPlan>>> = OnceLock::new();
static STAGED_PLANS: OnceLock<Mutex<HashMap<String, StagedPlan>>> = OnceLock::new();

pub fn check() -> Result<AppUpdatePreview, String> {
    let bytes = remote::fetch_https(APP_UPDATE_FEED_URL, MAX_FEED_BYTES)?;
    let feed = parse_feed(&bytes)?;
    preview_feed(feed, true)
}

pub fn prepare(app: &AppHandle, preview_id: &str) -> Result<AppUpdateStage, String> {
    let plan = feed_plans()
        .lock()
        .map_err(|_| "App update preview cache is unavailable".to_string())?
        .get(preview_id)
        .cloned()
        .ok_or_else(|| "App update preview expired; check again before downloading".to_string())?;
    if current_edition() != APP_UPDATE_EDITION {
        return Err("Developer edition cannot install the public Player update feed".into());
    }
    if !update_available(&plan.feed.version)? {
        return Err("The reviewed feed does not contain a newer Player version".into());
    }

    let data_dir = storage::data_dir(app)?;
    let root = data_dir.join(UPDATE_STAGING_DIRECTORY);
    create_safe_directory(&root, "App update staging")?;
    let stage_id = format!(
        "{}-{}",
        safe_version_component(&plan.feed.version),
        unix_nanos()
    );
    let stage_dir = root.join(&stage_id);
    create_safe_directory(&stage_dir, "App update preview")?;
    let stage_dir = fs::canonicalize(&stage_dir)
        .map_err(|error| format!("Could not resolve app update staging: {error}"))?;
    let partial = stage_dir.join("player.next.exe.partial");
    let staged_exe = stage_dir.join("player.next.exe");
    let digest = download_asset(&plan.feed.asset, &partial)?;
    if digest != plan.feed.asset.sha256.to_ascii_lowercase() {
        fs::remove_file(&partial).ok();
        return Err("Player update download failed SHA-256 verification".into());
    }
    verify_pe_file(&partial, &plan.feed.asset)?;
    fs::rename(&partial, &staged_exe)
        .map_err(|error| format!("Could not activate verified Player update staging: {error}"))?;

    let target_exe = trusted_current_exe()?;
    let target_sha256 = manifest::sha256(&target_exe)?;
    let result_path = data_dir.join(UPDATE_RESULT_FILE);
    let staged = StagedPlan {
        feed: plan.feed.clone(),
        stage_id: stage_id.clone(),
        stage_dir,
        staged_exe: staged_exe.clone(),
        target_exe,
        target_sha256,
        result_path,
    };
    cache_staged_plan(staged)?;
    Ok(AppUpdateStage {
        stage_id,
        version: plan.feed.version,
        path: staged_exe.display().to_string(),
        bytes: plan.feed.asset.bytes,
        sha256: plan.feed.asset.sha256,
        ready: true,
        message: "The Player update was downloaded and SHA-256 verified. The running launcher is unchanged.".into(),
    })
}

pub fn apply(stage_id: &str, confirmed: bool) -> Result<AppUpdateApplyOutcome, String> {
    require_confirmation(confirmed)?;
    if current_edition() != APP_UPDATE_EDITION {
        return Err("Developer edition cannot install the public Player update feed".into());
    }
    #[cfg(debug_assertions)]
    {
        let _ = stage_id;
        Err("App self-update is disabled in debug builds; test a packaged Player executable".into())
    }

    #[cfg(not(debug_assertions))]
    {
        let plan = staged_plans()
            .lock()
            .map_err(|_| "Staged app update cache is unavailable".to_string())?
            .get(stage_id)
            .cloned()
            .ok_or_else(|| "Staged app update expired; download it again".to_string())?;
        validate_staged_plan(&plan)?;
        let helper = plan.stage_dir.join("mythic-update-helper.exe");
        fs::copy(&plan.target_exe, &helper)
            .map_err(|error| format!("Could not create the isolated app update helper: {error}"))?;
        if manifest::sha256(&helper)? != plan.target_sha256 {
            fs::remove_file(&helper).ok();
            return Err("The isolated app update helper failed verification".into());
        }
        let backup = plan.stage_dir.join("player.previous.exe");
        let journal = HelperJournal {
            schema_version: HELPER_SCHEMA_VERSION,
            version: plan.feed.version.clone(),
            target_exe: plan.target_exe.display().to_string(),
            target_sha256: plan.target_sha256.clone(),
            staged_exe: plan.staged_exe.display().to_string(),
            staged_bytes: plan.feed.asset.bytes,
            staged_sha256: plan.feed.asset.sha256.clone(),
            backup_exe: backup.display().to_string(),
            result_path: plan.result_path.display().to_string(),
            restart_probe: false,
        };
        let journal_path = plan.stage_dir.join("apply-update.json");
        write_json_atomic(&journal_path, &journal)?;
        Command::new(&helper)
            .arg("--mythic-loot-apply-update")
            .arg(&journal_path)
            .arg(std::process::id().to_string())
            .spawn()
            .map_err(|error| format!("Could not start the isolated app update helper: {error}"))?;
        staged_plans()
            .lock()
            .map_err(|_| "Staged app update cache is unavailable".to_string())?
            .remove(stage_id);
        Ok(AppUpdateApplyOutcome {
            version: plan.feed.version,
            helper_started: true,
            message: "The verified update helper started. Mythic Loot Launcher will close, replace the Player executable, verify it and restart. A backup is retained for rollback.".into(),
        })
    }
}

pub fn last_result(app: &AppHandle) -> Result<Option<AppUpdateResult>, String> {
    let path = storage::data_dir(app)?.join(UPDATE_RESULT_FILE);
    if !path.exists() {
        return Ok(None);
    }
    let metadata = fs::symlink_metadata(&path)
        .map_err(|error| format!("Could not inspect the app update result: {error}"))?;
    if is_link_like(&metadata) || !metadata.is_file() || metadata.len() > 64 * 1024 {
        return Err("The app update result is not a safe launcher-owned file".into());
    }
    let bytes =
        fs::read(path).map_err(|error| format!("Could not read the app update result: {error}"))?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| format!("The app update result is invalid: {error}"))
}

pub fn try_run_helper() -> Option<i32> {
    let mut arguments = env::args_os().skip(1);
    let first = arguments.next();
    if first.as_deref() == Some(OsStr::new("--mythic-loot-update-restart-probe"))
        && arguments.next().is_none()
    {
        return Some(0);
    }
    if first.as_deref() != Some(OsStr::new("--mythic-loot-apply-update")) {
        return None;
    }
    let journal = arguments.next().map(PathBuf::from);
    let parent_pid = arguments
        .next()
        .and_then(|value| value.to_string_lossy().parse::<u32>().ok());
    if journal.is_none() || parent_pid.is_none() || arguments.next().is_some() {
        return Some(2);
    }
    let result = run_helper(&journal.unwrap(), parent_pid.unwrap());
    Some(if result.is_ok() { 0 } else { 1 })
}

fn run_helper(journal_path: &Path, parent_pid: u32) -> Result<(), String> {
    let journal = load_helper_journal(journal_path)?;
    wait_for_process_exit(parent_pid)?;
    let result = apply_replacement(&journal, false);
    let target = PathBuf::from(&journal.target_exe);
    let mut restart_command = Command::new(&target);
    if journal.restart_probe {
        restart_command.arg("--mythic-loot-update-restart-probe");
    }
    let restart = restart_command.spawn();
    let restart_process_id = restart.as_ref().ok().map(std::process::Child::id);
    let update_result = AppUpdateResult {
        version: journal.version.clone(),
        success: result.is_ok(),
        restart_process_id,
        message: match (&result, &restart) {
            (Ok(()), Ok(_)) => format!(
                "Mythic Loot Launcher Player {} was installed and verified.",
                journal.version
            ),
            (Ok(()), Err(error)) => format!(
                "Mythic Loot Launcher Player {} was installed and verified, but could not restart automatically: {error}",
                journal.version
            ),
            (Err(error), _) => format!(
                "Player update {} failed and the previous launcher was restored: {error}",
                journal.version
            ),
        },
        recorded_at: unix_millis(SystemTime::now()),
    };
    let result_path = PathBuf::from(&journal.result_path);
    let _ = write_json_atomic(&result_path, &update_result);
    result?;
    restart
        .map(|_| ())
        .map_err(|error| format!("The Player update installed but could not restart: {error}"))
}

fn apply_replacement(journal: &HelperJournal, fail_after_move: bool) -> Result<(), String> {
    validate_helper_journal(journal)?;
    let target = PathBuf::from(&journal.target_exe);
    let staged = PathBuf::from(&journal.staged_exe);
    let backup = PathBuf::from(&journal.backup_exe);
    verify_file(
        &target,
        0,
        &journal.target_sha256,
        "current Player executable",
    )?;
    verify_file(
        &staged,
        journal.staged_bytes,
        &journal.staged_sha256,
        "staged Player executable",
    )?;
    safe_copy(&target, &backup, "previous Player backup")?;
    verify_file(&backup, 0, &journal.target_sha256, "previous Player backup")?;

    let file_name = target
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "Player executable filename is invalid".to_string())?;
    let parent = target
        .parent()
        .ok_or_else(|| "Player executable has no installation folder".to_string())?;
    let next = parent.join(format!(".{file_name}.mythic-next"));
    let previous = parent.join(format!(".{file_name}.mythic-previous"));
    remove_regular_if_exists(&next)?;
    remove_regular_if_exists(&previous)?;
    safe_copy(&staged, &next, "next Player executable")?;
    verify_file(
        &next,
        journal.staged_bytes,
        &journal.staged_sha256,
        "next Player executable",
    )?;
    fs::rename(&target, &previous)
        .map_err(|error| format!("Could not preserve the installed Player executable: {error}"))?;
    let activation = if fail_after_move {
        Err("Controlled activation failure".to_string())
    } else {
        fs::rename(&next, &target)
            .map_err(|error| format!("Could not activate the new Player executable: {error}"))
    };
    if let Err(error) = activation {
        rollback_replacement(&target, &previous, &backup, &journal.target_sha256)?;
        return Err(error);
    }
    if let Err(error) = verify_file(
        &target,
        journal.staged_bytes,
        &journal.staged_sha256,
        "installed Player executable",
    ) {
        rollback_replacement(&target, &previous, &backup, &journal.target_sha256)?;
        return Err(error);
    }
    fs::remove_file(previous).ok();
    Ok(())
}

fn rollback_replacement(
    target: &Path,
    previous: &Path,
    backup: &Path,
    expected_sha256: &str,
) -> Result<(), String> {
    remove_regular_if_exists(target)?;
    if previous.exists() {
        fs::rename(previous, target).map_err(|error| {
            format!("Could not roll back the previous Player executable: {error}")
        })?;
    } else {
        safe_copy(backup, target, "rolled-back Player executable")?;
    }
    verify_file(target, 0, expected_sha256, "rolled-back Player executable")
}

fn preview_feed(feed: AppUpdateFeed, remember: bool) -> Result<AppUpdatePreview, String> {
    validate_feed(&feed)?;
    let update_available = update_available(&feed.version)?;
    let supported = minimum_supported(&feed.minimum_supported_version)?;
    let compatible = current_edition() == feed.edition;
    let can_install = update_available && supported && compatible;
    let bytes = serde_json::to_vec(&feed)
        .map_err(|error| format!("Could not fingerprint the app update feed: {error}"))?;
    let digest = format!("{:x}", Sha256::digest(bytes));
    let preview_id = digest[..16].to_string();
    if remember && can_install {
        let mut plans = feed_plans()
            .lock()
            .map_err(|_| "App update preview cache is unavailable".to_string())?;
        if plans.len() >= MAX_CACHED_PLANS {
            plans.clear();
        }
        plans.insert(preview_id.clone(), FeedPlan { feed: feed.clone() });
    }
    let message = if !compatible {
        "Developer edition manages the public Player feed but never installs a Player executable over itself.".into()
    } else if !supported {
        "This Player version is below the release's supported direct-update range. Install the latest Player installer manually.".into()
    } else if update_available {
        "A newer checksum-protected Player release is available for review.".into()
    } else {
        "Mythic Loot Launcher Player is current.".into()
    };
    Ok(AppUpdatePreview {
        preview_id,
        feed_url: APP_UPDATE_FEED_URL.into(),
        current_version: env!("CARGO_PKG_VERSION").into(),
        latest_version: feed.version,
        release_notes: feed.release_notes,
        published_at: feed.published_at,
        mandatory: feed.mandatory,
        minimum_supported_version: feed.minimum_supported_version,
        asset_bytes: feed.asset.bytes,
        asset_sha256: feed.asset.sha256,
        update_available,
        supported,
        can_install,
        message,
    })
}

pub(crate) fn parse_feed(bytes: &[u8]) -> Result<AppUpdateFeed, String> {
    if bytes.is_empty() || bytes.len() > MAX_FEED_BYTES {
        return Err("App update feed is empty or exceeds its safety limit".into());
    }
    let feed: AppUpdateFeed = serde_json::from_slice(bytes)
        .map_err(|error| format!("App update feed is invalid: {error}"))?;
    validate_feed(&feed)?;
    Ok(feed)
}

pub(crate) fn validate_feed(feed: &AppUpdateFeed) -> Result<(), String> {
    if feed.schema_version != FEED_SCHEMA_VERSION
        || feed.product != APP_UPDATE_PRODUCT
        || feed.edition != APP_UPDATE_EDITION
    {
        return Err("App update feed identity or schema is unsupported".into());
    }
    parse_version(&feed.version)?;
    if !feed.minimum_supported_version.is_empty() {
        parse_version(&feed.minimum_supported_version)?;
    }
    if feed.release_notes.chars().count() > 20_000 {
        return Err("App update release notes exceed the safety limit".into());
    }
    OffsetDateTime::parse(&feed.published_at, &Rfc3339)
        .map_err(|_| "App update publish time must use RFC 3339".to_string())?;
    validate_asset(&feed.asset)
}

fn validate_asset(asset: &AppUpdateAsset) -> Result<(), String> {
    let prefix = format!("https://github.com/{APP_UPDATE_REPOSITORY}/releases/download/");
    if asset.file_name != APP_UPDATE_ASSET_NAME
        || !asset.url.starts_with(&prefix)
        || !asset.url.ends_with(&format!("/{APP_UPDATE_ASSET_NAME}"))
        || asset.url.chars().any(char::is_whitespace)
        || asset.url.contains(['?', '#'])
        || asset.bytes == 0
        || asset.bytes > MAX_APP_BYTES
        || !valid_sha256(&asset.sha256)
    {
        return Err("App update asset does not satisfy the fixed Player release contract".into());
    }
    Ok(())
}

fn download_asset(asset: &AppUpdateAsset, destination: &Path) -> Result<String, String> {
    validate_asset(asset)?;
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(120)))
        .build();
    let agent: ureq::Agent = config.into();
    let mut response = agent
        .get(&asset.url)
        .header("User-Agent", "Mythic-Loot-Launcher/0.1")
        .call()
        .map_err(|error| format!("Player update download failed: {error}"))?;
    if let Some(length) = response
        .headers()
        .get("content-length")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        && length != asset.bytes
    {
        return Err("Player update Content-Length does not match the reviewed feed".into());
    }
    let mut output = File::create(destination)
        .map_err(|error| format!("Could not create Player update staging: {error}"))?;
    let mut reader = response.body_mut().as_reader();
    let mut digest = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| format!("Could not read Player update data: {error}"))?;
        if read == 0 {
            break;
        }
        total = total.saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
        if total > asset.bytes || total > MAX_APP_BYTES {
            fs::remove_file(destination).ok();
            return Err("Player update exceeded the reviewed size".into());
        }
        output
            .write_all(&buffer[..read])
            .map_err(|error| format!("Could not stage Player update data: {error}"))?;
        digest.update(&buffer[..read]);
    }
    output
        .sync_all()
        .map_err(|error| format!("Could not flush Player update staging: {error}"))?;
    if total != asset.bytes {
        fs::remove_file(destination).ok();
        return Err("Player update size does not match the reviewed feed".into());
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn verify_pe_file(path: &Path, asset: &AppUpdateAsset) -> Result<(), String> {
    verify_file(path, asset.bytes, &asset.sha256, "Player update")?;
    let mut file = File::open(path)
        .map_err(|error| format!("Could not inspect Player update header: {error}"))?;
    let mut header = [0_u8; 2];
    file.read_exact(&mut header)
        .map_err(|error| format!("Could not read Player update header: {error}"))?;
    if header != *b"MZ" {
        return Err("Player update is not a Windows executable".into());
    }
    Ok(())
}

#[cfg_attr(debug_assertions, allow(dead_code))]
fn validate_staged_plan(plan: &StagedPlan) -> Result<(), String> {
    if plan.stage_id.is_empty() || plan.stage_dir.file_name() != Some(OsStr::new(&plan.stage_id)) {
        return Err("Staged app update identity is invalid".into());
    }
    if trusted_current_exe()? != plan.target_exe
        || manifest::sha256(&plan.target_exe)? != plan.target_sha256
    {
        return Err("The running Player executable changed after update review".into());
    }
    verify_pe_file(&plan.staged_exe, &plan.feed.asset)
}

fn load_helper_journal(path: &Path) -> Result<HelperJournal, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("Could not inspect app update journal: {error}"))?;
    if is_link_like(&metadata) || !metadata.is_file() || metadata.len() > 64 * 1024 {
        return Err("App update journal is not a safe regular file".into());
    }
    let bytes =
        fs::read(path).map_err(|error| format!("Could not read app update journal: {error}"))?;
    let journal: HelperJournal = serde_json::from_slice(&bytes)
        .map_err(|error| format!("App update journal is invalid: {error}"))?;
    validate_helper_journal(&journal)?;
    let journal_parent = path
        .parent()
        .ok_or_else(|| "App update journal has no staging folder".to_string())?;
    if Path::new(&journal.staged_exe).parent() != Some(journal_parent)
        || Path::new(&journal.backup_exe).parent() != Some(journal_parent)
        || Path::new(&journal.result_path).parent()
            != journal_parent.parent().and_then(Path::parent)
        || Path::new(&journal.result_path).file_name() != Some(OsStr::new(UPDATE_RESULT_FILE))
    {
        return Err("App update journal paths escaped launcher-owned staging".into());
    }
    Ok(journal)
}

fn validate_helper_journal(journal: &HelperJournal) -> Result<(), String> {
    if journal.schema_version != HELPER_SCHEMA_VERSION
        || parse_version(&journal.version).is_err()
        || journal.staged_bytes == 0
        || journal.staged_bytes > MAX_APP_BYTES
        || !valid_sha256(&journal.target_sha256)
        || !valid_sha256(&journal.staged_sha256)
    {
        return Err("App update journal contract is invalid".into());
    }
    for value in [
        &journal.target_exe,
        &journal.staged_exe,
        &journal.backup_exe,
        &journal.result_path,
    ] {
        if value.trim().is_empty() || !Path::new(value).is_absolute() {
            return Err("App update journal contains a non-absolute path".into());
        }
    }
    let target_name = Path::new(&journal.target_exe)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if !matches!(
        target_name,
        "Mythic Loot Launcher Player.exe" | "mythic-loot-launcher.exe"
    ) {
        return Err("App update target is not a Mythic Loot Player executable".into());
    }
    Ok(())
}

fn trusted_current_exe() -> Result<PathBuf, String> {
    let path = env::current_exe()
        .map_err(|error| format!("Could not resolve the running launcher: {error}"))?;
    let metadata = fs::symlink_metadata(&path)
        .map_err(|error| format!("Could not inspect the running launcher: {error}"))?;
    if is_link_like(&metadata) || !metadata.is_file() {
        return Err("The running launcher is linked, redirected or not a regular file".into());
    }
    fs::canonicalize(path)
        .map_err(|error| format!("Could not resolve the running launcher: {error}"))
}

fn cache_staged_plan(plan: StagedPlan) -> Result<(), String> {
    let mut plans = staged_plans()
        .lock()
        .map_err(|_| "Staged app update cache is unavailable".to_string())?;
    if plans.len() >= MAX_CACHED_PLANS {
        plans.clear();
    }
    plans.insert(plan.stage_id.clone(), plan);
    Ok(())
}

fn feed_plans() -> &'static Mutex<HashMap<String, FeedPlan>> {
    FEED_PLANS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn staged_plans() -> &'static Mutex<HashMap<String, StagedPlan>> {
    STAGED_PLANS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn current_edition() -> &'static str {
    if cfg!(feature = "developer") {
        "developer"
    } else {
        APP_UPDATE_EDITION
    }
}

fn update_available(latest: &str) -> Result<bool, String> {
    Ok(parse_version(latest)? > parse_version(env!("CARGO_PKG_VERSION"))?)
}

fn minimum_supported(minimum: &str) -> Result<bool, String> {
    if minimum.is_empty() {
        return Ok(true);
    }
    Ok(parse_version(env!("CARGO_PKG_VERSION"))? >= parse_version(minimum)?)
}

pub(crate) fn parse_version(value: &str) -> Result<Version, String> {
    Version::parse(value.trim().trim_start_matches(['v', 'V']))
        .map_err(|_| format!("Invalid semantic version: {value}"))
}

#[cfg_attr(not(feature = "developer"), allow(dead_code))]
pub(crate) fn build_player_feed(
    version: &str,
    release_notes: &str,
    published_at: &str,
    bytes: u64,
    sha256: &str,
) -> Result<AppUpdateFeed, String> {
    let clean_version = parse_version(version)?.to_string();
    let feed = AppUpdateFeed {
        schema_version: FEED_SCHEMA_VERSION,
        product: APP_UPDATE_PRODUCT.into(),
        edition: APP_UPDATE_EDITION.into(),
        version: clean_version.clone(),
        release_notes: release_notes.trim().into(),
        published_at: published_at.into(),
        mandatory: false,
        minimum_supported_version: MINIMUM_DIRECT_UPDATE_VERSION.into(),
        asset: AppUpdateAsset {
            file_name: APP_UPDATE_ASSET_NAME.into(),
            url: format!(
                "https://github.com/{APP_UPDATE_REPOSITORY}/releases/download/v{clean_version}/{APP_UPDATE_ASSET_NAME}"
            ),
            bytes,
            sha256: sha256.to_ascii_lowercase(),
        },
    };
    validate_feed(&feed)?;
    Ok(feed)
}

fn safe_version_component(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '-'))
        .take(64)
        .collect()
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn require_confirmation(confirmed: bool) -> Result<(), String> {
    if confirmed {
        Ok(())
    } else {
        Err("App update installation requires explicit confirmation".into())
    }
}

fn create_safe_directory(path: &Path, label: &str) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|error| format!("Could not create {label}: {error}"))?;
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("Could not inspect {label}: {error}"))?;
    if is_link_like(&metadata) || !metadata.is_dir() {
        return Err(format!("{label} is linked, redirected or not a directory"));
    }
    Ok(())
}

fn safe_copy(source: &Path, destination: &Path, label: &str) -> Result<(), String> {
    if let Some(parent) = destination.parent() {
        create_safe_directory(parent, label)?;
    }
    remove_regular_if_exists(destination)?;
    let mut input = File::open(source)
        .map_err(|error| format!("Could not open source for {label}: {error}"))?;
    let mut output =
        File::create(destination).map_err(|error| format!("Could not create {label}: {error}"))?;
    std::io::copy(&mut input, &mut output)
        .and_then(|_| output.sync_all())
        .map_err(|error| format!("Could not write {label}: {error}"))?;
    Ok(())
}

fn verify_file(path: &Path, bytes: u64, sha256: &str, label: &str) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("Could not inspect {label}: {error}"))?;
    if is_link_like(&metadata) || !metadata.is_file() {
        return Err(format!(
            "{label} is linked, redirected or not a regular file"
        ));
    }
    if (bytes != 0 && metadata.len() != bytes)
        || manifest::sha256(path)? != sha256.to_ascii_lowercase()
    {
        return Err(format!("{label} failed size or SHA-256 verification"));
    }
    Ok(())
}

fn remove_regular_if_exists(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("Could not inspect prior app update file: {error}"))?;
    if is_link_like(&metadata) || !metadata.is_file() {
        return Err("Prior app update path is not a safe regular file".into());
    }
    fs::remove_file(path).map_err(|error| format!("Could not clear prior app update file: {error}"))
}

fn write_json_atomic(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("Could not serialize app update state: {error}"))?;
    let parent = path
        .parent()
        .ok_or_else(|| "App update state path has no parent".to_string())?;
    create_safe_directory(parent, "app update state folder")?;
    let temporary = parent.join(format!(
        ".{}.partial",
        path.file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("update")
    ));
    remove_regular_if_exists(&temporary)?;
    let mut file = File::create(&temporary)
        .map_err(|error| format!("Could not create app update state: {error}"))?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|error| format!("Could not flush app update state: {error}"))?;
    if path.exists() {
        remove_regular_if_exists(path)?;
    }
    fs::rename(temporary, path)
        .map_err(|error| format!("Could not activate app update state: {error}"))
}

fn wait_for_process_exit(pid: u32) -> Result<(), String> {
    let started = std::time::Instant::now();
    while process_is_running(pid) {
        if started.elapsed() >= PARENT_EXIT_TIMEOUT {
            return Err("Timed out waiting for the running launcher to close".into());
        }
        thread::sleep(Duration::from_millis(250));
    }
    Ok(())
}

#[cfg(windows)]
fn process_is_running(pid: u32) -> bool {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, STILL_ACTIVE},
        System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
    };
    if pid == 0 {
        return false;
    }
    // SAFETY: the handle is query-only, checked for null, and closed exactly once.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return false;
        }
        let mut exit_code = 0_u32;
        let read = GetExitCodeProcess(handle, &mut exit_code);
        CloseHandle(handle);
        read != 0 && exit_code == STILL_ACTIVE as u32
    }
}

#[cfg(not(windows))]
fn process_is_running(_pid: u32) -> bool {
    false
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

fn unix_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

fn unix_millis(value: SystemTime) -> i64 {
    value
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    fn feed(version: &str, bytes: &[u8]) -> AppUpdateFeed {
        AppUpdateFeed {
            schema_version: FEED_SCHEMA_VERSION,
            product: APP_UPDATE_PRODUCT.into(),
            edition: APP_UPDATE_EDITION.into(),
            version: version.into(),
            release_notes: "Verified release".into(),
            published_at: "2026-09-01T00:00:00Z".into(),
            mandatory: false,
            minimum_supported_version: "0.1.0".into(),
            asset: AppUpdateAsset {
                file_name: APP_UPDATE_ASSET_NAME.into(),
                url: format!(
                    "https://github.com/{APP_UPDATE_REPOSITORY}/releases/download/v{version}/{APP_UPDATE_ASSET_NAME}"
                ),
                bytes: u64::try_from(bytes.len()).unwrap(),
                sha256: hash(bytes),
            },
        }
    }

    #[test]
    fn feed_rejects_wrong_identity_repository_and_hash() {
        let bytes = b"MZnew";
        let valid = feed("0.2.0", bytes);
        assert!(validate_feed(&valid).is_ok());
        let mut wrong = valid.clone();
        wrong.edition = "developer".into();
        assert!(validate_feed(&wrong).is_err());
        let mut wrong = valid.clone();
        wrong.asset.url = "https://example.invalid/update.exe".into();
        assert!(validate_feed(&wrong).is_err());
        let mut wrong = valid;
        wrong.asset.sha256 = "0".repeat(63);
        assert!(validate_feed(&wrong).is_err());
    }

    #[test]
    fn version_comparison_uses_compiled_current_version() {
        assert!(!update_available("0.1.0").unwrap());
        assert!(update_available("0.2.0").unwrap());
        assert!(!update_available("0.0.9").unwrap());
        assert!(parse_version("not-a-version").is_err());
    }

    #[test]
    fn app_update_confirmation_is_fail_closed() {
        assert!(require_confirmation(false).is_err());
        assert!(require_confirmation(true).is_ok());
    }

    #[test]
    fn replacement_is_verified_and_mid_activation_failure_rolls_back() {
        let root = tempfile::tempdir().unwrap();
        let stage = root.path().join("app-update-staging/fixture");
        fs::create_dir_all(&stage).unwrap();
        let target = root.path().join("Mythic Loot Launcher Player.exe");
        let staged = stage.join("player.next.exe");
        let backup = stage.join("player.previous.exe");
        fs::write(&target, b"MZold").unwrap();
        fs::write(&staged, b"MZnew").unwrap();
        let journal = HelperJournal {
            schema_version: HELPER_SCHEMA_VERSION,
            version: "0.2.0".into(),
            target_exe: target.display().to_string(),
            target_sha256: hash(b"MZold"),
            staged_exe: staged.display().to_string(),
            staged_bytes: 5,
            staged_sha256: hash(b"MZnew"),
            backup_exe: backup.display().to_string(),
            result_path: root.path().join(UPDATE_RESULT_FILE).display().to_string(),
            restart_probe: false,
        };
        apply_replacement(&journal, false).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"MZnew");

        fs::write(&target, b"MZold").unwrap();
        fs::write(&staged, b"MZnew").unwrap();
        let error = apply_replacement(&journal, true).unwrap_err();
        assert!(error.contains("Controlled"));
        assert_eq!(fs::read(&target).unwrap(), b"MZold");
    }

    #[test]
    fn corrupt_staged_executable_never_mutates_target() {
        let root = tempfile::tempdir().unwrap();
        let stage = root.path().join("app-update-staging/fixture");
        fs::create_dir_all(&stage).unwrap();
        let target = root.path().join("Mythic Loot Launcher Player.exe");
        let staged = stage.join("player.next.exe");
        fs::write(&target, b"MZold").unwrap();
        fs::write(&staged, b"tampered").unwrap();
        let journal = HelperJournal {
            schema_version: HELPER_SCHEMA_VERSION,
            version: "0.2.0".into(),
            target_exe: target.display().to_string(),
            target_sha256: hash(b"MZold"),
            staged_exe: staged.display().to_string(),
            staged_bytes: 5,
            staged_sha256: hash(b"MZnew"),
            backup_exe: stage.join("backup.exe").display().to_string(),
            result_path: root.path().join(UPDATE_RESULT_FILE).display().to_string(),
            restart_probe: false,
        };
        assert!(apply_replacement(&journal, false).is_err());
        assert_eq!(fs::read(target).unwrap(), b"MZold");
    }
}
