use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    process::Child,
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::{
    launch, manifest,
    manifest::{FileEntry, Manifest},
    models::{GameProfile, ReadinessStatus},
    readiness, safe_path, storage,
};

const JOURNAL_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SafeLaunchStatus {
    pub profile_id: String,
    pub active: bool,
    pub session_id: String,
    pub install_dir: String,
    pub game_process_id: u32,
    pub game_process_running: bool,
    pub disabled_files: usize,
    pub started_at: u64,
    pub recoverable: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SafeLaunchOutcome {
    pub profile_id: String,
    pub session_id: String,
    pub pid: u32,
    pub disabled: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SafeLaunchRecovery {
    pub profile_id: String,
    pub restored: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum SessionPhase {
    Prepared,
    Running,
    Exited,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MovedFile {
    path: String,
    disabled_path: String,
    size: u64,
    sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SafeLaunchJournal {
    schema_version: u32,
    session_id: String,
    profile_id: String,
    install_dir: String,
    started_at: u64,
    game_process_id: u32,
    phase: SessionPhase,
    files: Vec<MovedFile>,
}

pub fn status(app: &AppHandle, profile_id: &str) -> Result<SafeLaunchStatus, String> {
    let config = storage::load_or_create(app)?;
    let profile = config
        .profiles
        .iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| "That modpack profile does not exist".to_string())?;
    status_at(
        &storage::data_dir(app)?,
        profile_id,
        Some(Path::new(profile.install_dir.trim())),
    )
}

pub fn start(
    app: &AppHandle,
    profile_id: &str,
    confirmed: bool,
) -> Result<SafeLaunchOutcome, String> {
    require_confirmation(confirmed, "Starting Safe Launch")?;
    let config = storage::load_or_create(app)?;
    let profile = config
        .profiles
        .iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| "That modpack profile does not exist".to_string())?;
    let loaded = manifest::load_for_profile(app, profile);
    if !loaded.summary.valid {
        return Err(format!(
            "The trusted manifest cannot be used: {}",
            loaded.summary.errors.join("; ")
        ));
    }
    let health = readiness::assess(profile, Some(&loaded.summary));
    if health.status != ReadinessStatus::Ready {
        return Err(format!(
            "{} is not ready for Safe Launch: {}",
            profile.display_name, health.headline
        ));
    }
    start_at(
        profile,
        &loaded.manifest,
        &storage::data_dir(app)?,
        confirmed,
    )
}

pub fn recover(
    app: &AppHandle,
    profile_id: &str,
    confirmed: bool,
) -> Result<SafeLaunchRecovery, String> {
    require_confirmation(confirmed, "Safe Launch recovery")?;
    let config = storage::load_or_create(app)?;
    let profile = config
        .profiles
        .iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| "That modpack profile does not exist".to_string())?;
    recover_at(
        &storage::data_dir(app)?,
        profile_id,
        None,
        false,
        Some(Path::new(profile.install_dir.trim())),
    )
}

fn start_at(
    profile: &GameProfile,
    manifest: &Manifest,
    data_dir: &Path,
    confirmed: bool,
) -> Result<SafeLaunchOutcome, String> {
    require_confirmation(confirmed, "Starting Safe Launch")?;
    validate_component(&profile.id, "Profile id")?;
    let install_dir = PathBuf::from(profile.install_dir.trim());
    if !install_dir.is_dir() {
        return Err("Choose an existing modpack folder before using Safe Launch".into());
    }
    reject_root_link(&install_dir)?;
    ensure_data_outside_install(data_dir, &install_dir)?;
    if manifest.optional_files.is_empty() {
        return Err("This trusted manifest does not declare any optional files to disable".into());
    }
    let manifest_issues = manifest::validate(manifest, Some(profile));
    if !manifest_issues.is_empty() {
        return Err(format!(
            "The trusted manifest cannot be used: {}",
            manifest_issues.join("; ")
        ));
    }

    let journal_path = journal_path(data_dir, &profile.id)?;
    if journal_path.exists() {
        let existing = load_journal(&journal_path, Some(&profile.id))?;
        return Err(format!(
            "Safe Launch session {} is still recorded. Recover it before starting another.",
            existing.session_id
        ));
    }
    let session_id = format!("{}-safe-{:x}", profile.id, unix_nanos());
    let files = collect_optional_files(&install_dir, &session_id, &manifest.optional_files)?;
    if files.is_empty() {
        return Err("No installed optional files are currently available to disable".into());
    }
    let mut journal = SafeLaunchJournal {
        schema_version: JOURNAL_SCHEMA_VERSION,
        session_id: session_id.clone(),
        profile_id: profile.id.clone(),
        install_dir: install_dir.display().to_string(),
        started_at: unix_seconds(),
        game_process_id: 0,
        phase: SessionPhase::Prepared,
        files,
    };
    save_journal(&journal_path, &journal)?;

    if let Err(error) = move_optional_aside(&install_dir, &journal) {
        let recovery = recover_journal(&journal_path, &journal, Some(&install_dir));
        return match recovery {
            Ok(_) => Err(format!(
                "Safe Launch preparation failed and all moved files were restored: {error}"
            )),
            Err(recovery_error) => Err(format!(
                "Safe Launch preparation failed ({error}) and recovery also failed ({recovery_error})"
            )),
        };
    }

    let mut child = match launch::spawn(profile) {
        Ok(child) => child,
        Err(error) => {
            let recovery = recover_journal(&journal_path, &journal, Some(&install_dir));
            return match recovery {
                Ok(_) => Err(format!(
                    "The game did not start and optional files were restored: {error}"
                )),
                Err(recovery_error) => Err(format!(
                    "The game did not start ({error}) and optional-file recovery failed ({recovery_error})"
                )),
            };
        }
    };
    journal.game_process_id = child.id();
    journal.phase = SessionPhase::Running;
    if let Err(error) = save_journal(&journal_path, &journal) {
        let _ = child.kill();
        let _ = child.wait();
        let recovery = recover_journal(&journal_path, &journal, Some(&install_dir));
        return match recovery {
            Ok(_) => Err(format!(
                "The game was stopped because its Safe Launch session could not be persisted: {error}"
            )),
            Err(recovery_error) => Err(format!(
                "The Safe Launch session could not be persisted ({error}) and recovery failed ({recovery_error})"
            )),
        };
    }

    let watcher_data = data_dir.to_path_buf();
    let watcher_profile = profile.id.clone();
    let watcher_session = session_id.clone();
    let watcher_install = install_dir;
    thread::spawn(move || {
        watch_child(
            &mut child,
            &watcher_data,
            &watcher_profile,
            &watcher_session,
            &watcher_install,
        )
    });

    Ok(SafeLaunchOutcome {
        profile_id: profile.id.clone(),
        session_id,
        pid: journal.game_process_id,
        disabled: journal
            .files
            .iter()
            .map(|entry| entry.path.clone())
            .collect(),
        message: format!(
            "Started {} in Safe Launch with {} optional file(s) disabled. They will be restored after the game exits.",
            profile.display_name,
            journal.files.len()
        ),
    })
}

fn watch_child(
    child: &mut Child,
    data_dir: &Path,
    profile_id: &str,
    session_id: &str,
    expected_install: &Path,
) {
    let _ = child.wait();
    let Ok(path) = journal_path(data_dir, profile_id) else {
        return;
    };
    let Ok(mut journal) = load_journal(&path, Some(profile_id)) else {
        return;
    };
    if journal.session_id != session_id {
        return;
    }
    journal.phase = SessionPhase::Exited;
    if save_journal(&path, &journal).is_ok() {
        let _ = recover_journal(&path, &journal, Some(expected_install));
    }
}

fn status_at(
    data_dir: &Path,
    profile_id: &str,
    expected_install: Option<&Path>,
) -> Result<SafeLaunchStatus, String> {
    validate_component(profile_id, "Profile id")?;
    let path = journal_path(data_dir, profile_id)?;
    if !path.exists() {
        return Ok(inactive_status(profile_id));
    }
    reject_link(&path, "Safe Launch journal")?;
    let journal = load_journal(&path, Some(profile_id))?;
    require_expected_install(&journal, expected_install)?;
    let process_running =
        journal.phase == SessionPhase::Running && process_is_running(journal.game_process_id);
    let recoverable = !process_running;
    Ok(SafeLaunchStatus {
        profile_id: profile_id.into(),
        active: true,
        session_id: journal.session_id,
        install_dir: journal.install_dir,
        game_process_id: journal.game_process_id,
        game_process_running: process_running,
        disabled_files: journal.files.len(),
        started_at: journal.started_at,
        recoverable,
        message: if process_running {
            "Safe Launch is active. Optional files will be restored automatically after the recorded game process exits.".into()
        } else {
            "An interrupted Safe Launch session is recoverable. Review it and restore the disabled optional files.".into()
        },
    })
}

fn recover_at(
    data_dir: &Path,
    profile_id: &str,
    expected_session: Option<&str>,
    allow_running: bool,
    expected_install: Option<&Path>,
) -> Result<SafeLaunchRecovery, String> {
    let path = journal_path(data_dir, profile_id)?;
    if !path.is_file() {
        return Ok(SafeLaunchRecovery {
            profile_id: profile_id.into(),
            restored: Vec::new(),
            message: "No Safe Launch recovery is pending.".into(),
        });
    }
    reject_link(&path, "Safe Launch journal")?;
    let journal = load_journal(&path, Some(profile_id))?;
    if expected_session.is_some_and(|value| value != journal.session_id) {
        return Err("The Safe Launch session changed before recovery".into());
    }
    if !allow_running
        && journal.phase == SessionPhase::Running
        && process_is_running(journal.game_process_id)
    {
        return Err("The recorded game process is still running. Close the game before restoring optional files.".into());
    }
    recover_journal(&path, &journal, expected_install)
}

fn recover_journal(
    journal_path: &Path,
    journal: &SafeLaunchJournal,
    expected_install: Option<&Path>,
) -> Result<SafeLaunchRecovery, String> {
    validate_journal(journal, None)?;
    require_expected_install(journal, expected_install)?;
    let install_dir = PathBuf::from(&journal.install_dir);
    if !install_dir.is_dir() {
        return Err("The recorded Safe Launch modpack folder no longer exists".into());
    }
    reject_root_link(&install_dir)?;

    enum RecoveryState {
        MoveBack { disabled: PathBuf, target: PathBuf },
        AlreadyRestored,
    }
    let mut states = Vec::with_capacity(journal.files.len());
    for entry in &journal.files {
        let target = guarded_join(&install_dir, &entry.path)?;
        let disabled = guarded_join(&install_dir, &entry.disabled_path)?;
        let target_exists = target.exists();
        let disabled_exists = disabled.exists();
        match (target_exists, disabled_exists) {
            (false, true) => {
                verify_recorded_file(&disabled, entry)?;
                states.push(RecoveryState::MoveBack { disabled, target });
            }
            (true, false) => {
                verify_recorded_file(&target, entry)?;
                states.push(RecoveryState::AlreadyRestored);
            }
            (true, true) => {
                return Err(format!(
                    "Recovery conflict: both live and disabled copies exist for {}",
                    entry.path
                ));
            }
            (false, false) => {
                return Err(format!(
                    "Recovery cannot find either copy of {}",
                    entry.path
                ));
            }
        }
    }

    let mut restored = Vec::new();
    for (entry, state) in journal.files.iter().zip(states) {
        match state {
            RecoveryState::MoveBack { disabled, target } => {
                fs::create_dir_all(target.parent().unwrap_or(&install_dir)).map_err(|error| {
                    format!(
                        "Could not recreate optional-file folder for {}: {error}",
                        entry.path
                    )
                })?;
                fs::rename(&disabled, &target).map_err(|error| {
                    format!("Could not restore optional file {}: {error}", entry.path)
                })?;
                verify_recorded_file(&target, entry)?;
                restored.push(entry.path.clone());
                remove_empty_parents(&install_dir, disabled.parent());
            }
            RecoveryState::AlreadyRestored => restored.push(entry.path.clone()),
        }
    }
    if journal_path.exists() {
        fs::remove_file(journal_path).map_err(|error| {
            format!(
                "Optional files were restored but the recovery journal could not be cleared: {error}"
            )
        })?;
    }
    if let Some(parent) = journal_path.parent() {
        fs::remove_dir(parent).ok();
    }
    Ok(SafeLaunchRecovery {
        profile_id: journal.profile_id.clone(),
        restored,
        message: "Safe Launch ended and all recorded optional files were restored.".into(),
    })
}

fn collect_optional_files(
    install_dir: &Path,
    session_id: &str,
    optional_files: &[FileEntry],
) -> Result<Vec<MovedFile>, String> {
    validate_component(session_id, "Safe Launch session id")?;
    let mut files = Vec::new();
    for entry in optional_files {
        let relative = safe_path::normalize_relative(&entry.path)?;
        let source = guarded_join(install_dir, &relative)?;
        if !source.exists() {
            continue;
        }
        if !source.is_file() {
            return Err(format!("Optional path is not a regular file: {relative}"));
        }
        let disabled_path = format!(".mythic-loot-disabled/safe-launch/{session_id}/{relative}");
        let disabled = guarded_join(install_dir, &disabled_path)?;
        if disabled.exists() {
            return Err(format!(
                "Safe Launch destination already exists: {relative}"
            ));
        }
        let metadata = source
            .metadata()
            .map_err(|error| format!("Could not inspect optional file {relative}: {error}"))?;
        files.push(MovedFile {
            path: relative,
            disabled_path,
            size: metadata.len(),
            sha256: manifest::sha256(&source)?,
        });
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

fn move_optional_aside(install_dir: &Path, journal: &SafeLaunchJournal) -> Result<(), String> {
    for entry in &journal.files {
        let source = guarded_join(install_dir, &entry.path)?;
        let target = guarded_join(install_dir, &entry.disabled_path)?;
        verify_recorded_file(&source, entry)?;
        fs::create_dir_all(target.parent().unwrap_or(install_dir))
            .map_err(|error| format!("Could not create Safe Launch storage: {error}"))?;
        fs::rename(&source, &target)
            .map_err(|error| format!("Could not disable optional file {}: {error}", entry.path))?;
        verify_recorded_file(&target, entry)?;
    }
    Ok(())
}

fn verify_recorded_file(path: &Path, entry: &MovedFile) -> Result<(), String> {
    if !path.is_file() {
        return Err(format!("Recorded optional file is missing: {}", entry.path));
    }
    let size = path
        .metadata()
        .map_err(|error| format!("Could not inspect optional file {}: {error}", entry.path))?
        .len();
    if size != entry.size || manifest::sha256(path)? != entry.sha256 {
        return Err(format!("Recorded optional file changed: {}", entry.path));
    }
    Ok(())
}

fn load_journal(path: &Path, expected_profile: Option<&str>) -> Result<SafeLaunchJournal, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("Could not read Safe Launch journal: {error}"))?;
    let journal: SafeLaunchJournal = serde_json::from_slice(&bytes)
        .map_err(|error| format!("Safe Launch journal is invalid: {error}"))?;
    validate_journal(&journal, expected_profile)?;
    Ok(journal)
}

fn validate_journal(
    journal: &SafeLaunchJournal,
    expected_profile: Option<&str>,
) -> Result<(), String> {
    if journal.schema_version != JOURNAL_SCHEMA_VERSION {
        return Err(format!(
            "Unsupported Safe Launch journal schema: {}",
            journal.schema_version
        ));
    }
    validate_component(&journal.profile_id, "Profile id")?;
    validate_component(&journal.session_id, "Safe Launch session id")?;
    if expected_profile.is_some_and(|value| value != journal.profile_id) {
        return Err("Safe Launch journal belongs to another profile".into());
    }
    if journal.install_dir.trim().is_empty() || journal.files.is_empty() {
        return Err("Safe Launch journal is incomplete".into());
    }
    let expected_prefix = format!(".mythic-loot-disabled/safe-launch/{}/", journal.session_id);
    let mut seen = std::collections::HashSet::new();
    for entry in &journal.files {
        let path = safe_path::normalize_relative(&entry.path)?;
        let disabled = safe_path::normalize_relative(&entry.disabled_path)?;
        if path != entry.path
            || disabled != entry.disabled_path
            || !disabled.starts_with(&expected_prefix)
            || entry.sha256.len() != 64
            || !entry.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
            || !seen.insert(path.to_ascii_lowercase())
        {
            return Err(format!(
                "Safe Launch journal contains an invalid file: {}",
                entry.path
            ));
        }
    }
    Ok(())
}

fn save_journal(path: &Path, journal: &SafeLaunchJournal) -> Result<(), String> {
    validate_journal(journal, Some(&journal.profile_id))?;
    let parent = path
        .parent()
        .ok_or_else(|| "Safe Launch journal has no parent folder".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Could not create Safe Launch data folder: {error}"))?;
    reject_link(parent, "Safe Launch data folder")?;
    let temporary = path.with_extension("json.partial");
    if path.exists() {
        reject_link(path, "Safe Launch journal")?;
    }
    if temporary.exists() {
        reject_link(&temporary, "Safe Launch staged journal")?;
    }
    let encoded = serde_json::to_vec_pretty(journal)
        .map_err(|error| format!("Could not encode Safe Launch journal: {error}"))?;
    {
        let mut output = File::create(&temporary)
            .map_err(|error| format!("Could not stage Safe Launch journal: {error}"))?;
        output
            .write_all(&encoded)
            .map_err(|error| format!("Could not write Safe Launch journal: {error}"))?;
        output
            .sync_all()
            .map_err(|error| format!("Could not sync Safe Launch journal: {error}"))?;
    }
    fs::rename(&temporary, path)
        .map_err(|error| format!("Could not activate Safe Launch journal: {error}"))
}

fn journal_path(data_dir: &Path, profile_id: &str) -> Result<PathBuf, String> {
    validate_component(profile_id, "Profile id")?;
    Ok(data_dir
        .join("safe-launch")
        .join(format!("{profile_id}.json")))
}

fn require_expected_install(
    journal: &SafeLaunchJournal,
    expected_install: Option<&Path>,
) -> Result<(), String> {
    if expected_install.is_some_and(|path| path != Path::new(journal.install_dir.trim())) {
        return Err(
            "The configured modpack folder changed after Safe Launch. Restore the recorded folder setting before recovery."
                .into(),
        );
    }
    Ok(())
}

fn ensure_data_outside_install(data_dir: &Path, install_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(data_dir)
        .map_err(|error| format!("Could not create launcher data folder: {error}"))?;
    let install = fs::canonicalize(install_dir)
        .map_err(|error| format!("Could not resolve modpack folder: {error}"))?;
    let data = fs::canonicalize(data_dir)
        .map_err(|error| format!("Could not resolve launcher data folder: {error}"))?;
    if data.starts_with(&install) {
        return Err("Safe Launch journals must be stored outside the live modpack folder".into());
    }
    Ok(())
}

fn guarded_join(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let target = safe_path::safe_join(root, relative)?;
    let canonical_root = fs::canonicalize(root)
        .map_err(|error| format!("Could not resolve modpack folder: {error}"))?;
    let mut current = root.to_path_buf();
    for part in safe_path::normalize_relative(relative)?.split('/') {
        current.push(part);
        if !current.exists() {
            continue;
        }
        let metadata = fs::symlink_metadata(&current)
            .map_err(|error| format!("Could not inspect {}: {error}", current.display()))?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "Symbolic links are not allowed in Safe Launch paths: {}",
                current.display()
            ));
        }
        let resolved = fs::canonicalize(&current)
            .map_err(|error| format!("Could not resolve {}: {error}", current.display()))?;
        if !resolved.starts_with(&canonical_root) {
            return Err(format!(
                "Safe Launch path escapes the configured folder: {relative}"
            ));
        }
    }
    Ok(target)
}

fn reject_root_link(root: &Path) -> Result<(), String> {
    reject_link(root, "Modpack folder")
}

fn reject_link(path: &Path, label: &str) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("Could not inspect {label}: {error}"))?;
    if metadata.file_type().is_symlink() {
        return Err(format!("{label} cannot be a symbolic link"));
    }
    Ok(())
}

fn remove_empty_parents(root: &Path, start: Option<&Path>) {
    let mut current = start.map(Path::to_path_buf);
    while let Some(path) = current {
        if path == root || fs::remove_dir(&path).is_err() {
            break;
        }
        current = path.parent().map(Path::to_path_buf);
    }
}

fn require_confirmation(confirmed: bool, action: &str) -> Result<(), String> {
    if confirmed {
        Ok(())
    } else {
        Err(format!("{action} requires explicit confirmation"))
    }
}

fn validate_component(value: &str, label: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 160
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte))
    {
        return Err(format!("{label} is not safe for launcher-owned storage"));
    }
    Ok(())
}

fn inactive_status(profile_id: &str) -> SafeLaunchStatus {
    SafeLaunchStatus {
        profile_id: profile_id.into(),
        active: false,
        session_id: String::new(),
        install_dir: String::new(),
        game_process_id: 0,
        game_process_running: false,
        disabled_files: 0,
        started_at: 0,
        recoverable: false,
        message: "No Safe Launch session is active.".into(),
    }
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn unix_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
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
    // SAFETY: OpenProcess is called with a numeric PID and query-only access. The returned handle
    // is checked for null and closed exactly once after GetExitCodeProcess.
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

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use tempfile::TempDir;

    fn hash(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    fn profile(install: &Path) -> GameProfile {
        let mut profile = crate::models::LauncherConfig::default().profiles.remove(0);
        profile.id = "fixture".into();
        profile.install_dir = install.display().to_string();
        profile.game_exe_path = "unused.exe".into();
        profile
    }

    fn optional_manifest(bytes: &[u8]) -> Manifest {
        let mut manifest = Manifest {
            manifest_version: "1.0".into(),
            profile_id: "fixture".into(),
            game: "minecraft".into(),
            modpack_version: "1.0.0".into(),
            ..Manifest::default()
        };
        manifest.optional_files.push(FileEntry {
            path: "mods/voice.jar".into(),
            size: bytes.len() as i64,
            hash: hash(bytes),
            download_url: String::new(),
            required: false,
            category: "Voice Chat".into(),
        });
        manifest
    }

    fn prepare_without_launch(
        profile: &GameProfile,
        manifest: &Manifest,
        data: &Path,
    ) -> SafeLaunchJournal {
        let install = PathBuf::from(&profile.install_dir);
        let session_id = "fixture-safe-session";
        let journal = SafeLaunchJournal {
            schema_version: JOURNAL_SCHEMA_VERSION,
            session_id: session_id.into(),
            profile_id: profile.id.clone(),
            install_dir: profile.install_dir.clone(),
            started_at: unix_seconds(),
            game_process_id: 0,
            phase: SessionPhase::Prepared,
            files: collect_optional_files(&install, session_id, &manifest.optional_files).unwrap(),
        };
        let path = journal_path(data, &profile.id).unwrap();
        save_journal(&path, &journal).unwrap();
        move_optional_aside(&install, &journal).unwrap();
        journal
    }

    #[test]
    fn persisted_session_recovers_exact_optional_file_after_process_loss() {
        let temp = TempDir::new().unwrap();
        let install = temp.path().join("install");
        let data = temp.path().join("data");
        fs::create_dir_all(install.join("mods")).unwrap();
        fs::write(install.join("mods/voice.jar"), b"voice").unwrap();
        let profile = profile(&install);
        let manifest = optional_manifest(b"voice");
        let journal = prepare_without_launch(&profile, &manifest, &data);

        assert!(!install.join("mods/voice.jar").exists());
        assert!(install.join(&journal.files[0].disabled_path).is_file());
        let status = status_at(&data, &profile.id, Some(&install)).unwrap();
        assert!(status.active);
        assert!(status.recoverable);

        let outcome = recover_at(&data, &profile.id, None, false, Some(&install)).unwrap();
        assert_eq!(outcome.restored, vec!["mods/voice.jar"]);
        assert_eq!(fs::read(install.join("mods/voice.jar")).unwrap(), b"voice");
        assert!(!journal_path(&data, &profile.id).unwrap().exists());
    }

    #[test]
    fn recovery_refuses_conflicting_live_copy_without_mutation() {
        let temp = TempDir::new().unwrap();
        let install = temp.path().join("install");
        let data = temp.path().join("data");
        fs::create_dir_all(install.join("mods")).unwrap();
        fs::write(install.join("mods/voice.jar"), b"voice").unwrap();
        let profile = profile(&install);
        let manifest = optional_manifest(b"voice");
        let journal = prepare_without_launch(&profile, &manifest, &data);
        fs::write(install.join("mods/voice.jar"), b"conflict").unwrap();

        let error = recover_at(&data, &profile.id, None, false, Some(&install)).unwrap_err();
        assert!(error.contains("both live and disabled copies"));
        assert_eq!(
            fs::read(install.join("mods/voice.jar")).unwrap(),
            b"conflict"
        );
        assert_eq!(
            fs::read(install.join(&journal.files[0].disabled_path)).unwrap(),
            b"voice"
        );
        assert!(journal_path(&data, &profile.id).unwrap().exists());
    }

    #[test]
    fn recovery_rejects_changed_disabled_copy() {
        let temp = TempDir::new().unwrap();
        let install = temp.path().join("install");
        let data = temp.path().join("data");
        fs::create_dir_all(install.join("mods")).unwrap();
        fs::write(install.join("mods/voice.jar"), b"voice").unwrap();
        let profile = profile(&install);
        let manifest = optional_manifest(b"voice");
        let journal = prepare_without_launch(&profile, &manifest, &data);
        fs::write(install.join(&journal.files[0].disabled_path), b"changed").unwrap();

        let error = recover_at(&data, &profile.id, None, false, Some(&install)).unwrap_err();
        assert!(error.contains("changed"));
        assert!(!install.join("mods/voice.jar").exists());
        assert!(journal_path(&data, &profile.id).unwrap().exists());
    }

    #[test]
    fn confirmation_is_fail_closed() {
        assert!(require_confirmation(false, "Safe Launch").is_err());
        assert!(require_confirmation(true, "Safe Launch").is_ok());
    }

    #[test]
    fn launch_failure_restores_optional_files_and_clears_journal() {
        let temp = TempDir::new().unwrap();
        let install = temp.path().join("install");
        let data = temp.path().join("data");
        fs::create_dir_all(install.join("mods")).unwrap();
        fs::write(install.join("mods/voice.jar"), b"voice").unwrap();
        let mut profile = profile(&install);
        profile.game_exe_path = install.join("missing.exe").display().to_string();
        let manifest = optional_manifest(b"voice");

        let error = start_at(&profile, &manifest, &data, true).unwrap_err();
        assert!(error.contains("game did not start"));
        assert_eq!(fs::read(install.join("mods/voice.jar")).unwrap(), b"voice");
        assert!(!journal_path(&data, &profile.id).unwrap().exists());
    }

    #[test]
    fn recovery_journal_cannot_redirect_to_another_installation() {
        let temp = TempDir::new().unwrap();
        let install = temp.path().join("install");
        let other = temp.path().join("other");
        let data = temp.path().join("data");
        fs::create_dir_all(install.join("mods")).unwrap();
        fs::create_dir_all(other.join("mods")).unwrap();
        fs::write(install.join("mods/voice.jar"), b"voice").unwrap();
        let profile = profile(&install);
        let manifest = optional_manifest(b"voice");
        prepare_without_launch(&profile, &manifest, &data);
        let path = journal_path(&data, &profile.id).unwrap();
        let mut journal = load_journal(&path, Some(&profile.id)).unwrap();
        journal.install_dir = other.display().to_string();
        save_journal(&path, &journal).unwrap();

        let error = recover_at(&data, &profile.id, None, false, Some(&install)).unwrap_err();
        assert!(error.contains("configured modpack folder changed"));
        assert!(fs::read_dir(other.join("mods")).unwrap().next().is_none());
        assert!(path.exists());
    }

    #[cfg(windows)]
    #[test]
    fn windows_process_probe_distinguishes_current_and_invalid_pid() {
        assert!(process_is_running(std::process::id()));
        assert!(!process_is_running(0));
    }
}
