use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{self, BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use fs2::available_space;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use zip::{CompressionMethod, DateTime, ZipArchive, ZipWriter, write::SimpleFileOptions};

use crate::{manifest, models::GameProfile, safe_path, storage};

const BACKUP_SCHEMA_VERSION: u32 = 1;
const BACKUP_METADATA_PATH: &str = ".mythic-loot-backup.json";
const DEFAULT_KEEP_PER_PROFILE: usize = 5;
const FREE_SPACE_BUFFER: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestorePointSummary {
    pub backup_id: String,
    pub profile_id: String,
    pub label: String,
    pub created_at: u64,
    pub size_bytes: u64,
    pub file_count: usize,
    pub removes_on_restore: usize,
    pub local_modpack_version: String,
    pub valid: bool,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestorePreview {
    pub preview_id: String,
    pub backup_id: String,
    pub profile_id: String,
    pub label: String,
    pub created_at: u64,
    pub local_modpack_version: String,
    pub staged_files: usize,
    pub staged_bytes: u64,
    pub existing_files_to_backup: usize,
    pub files_to_remove: usize,
    pub ready: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreOutcome {
    pub profile_id: String,
    pub backup_id: String,
    pub success: bool,
    pub restored: Vec<String>,
    pub removed: Vec<String>,
    pub recovery_backup_path: String,
    pub rolled_back: bool,
    pub rollback_error: String,
    pub message: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupFile {
    path: String,
    size: u64,
    sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupMetadata {
    schema_version: u32,
    backup_id: String,
    profile_id: String,
    label: String,
    created_at: u64,
    local_modpack_version: String,
    files: Vec<BackupFile>,
    remove_on_restore: Vec<String>,
}

#[derive(Debug, Clone)]
struct RestorePlan {
    preview_id: String,
    backup_id: String,
    profile_id: String,
    install_dir: PathBuf,
    backups_dir: PathBuf,
    stage_root: PathBuf,
    content_dir: PathBuf,
    metadata: BackupMetadata,
}

static RESTORE_PLANS: OnceLock<Mutex<HashMap<String, RestorePlan>>> = OnceLock::new();

pub(crate) fn create_archive(
    backups_dir: &Path,
    backup_id: &str,
    profile_id: &str,
    label: &str,
    local_modpack_version: &str,
    files: &[(String, PathBuf)],
    remove_on_restore: &[String],
) -> Result<PathBuf, String> {
    validate_component(backup_id, "Backup id")?;
    validate_component(profile_id, "Profile id")?;
    validate_component(label, "Backup label")?;

    let mut normalized_files = Vec::with_capacity(files.len());
    let mut seen = HashSet::new();
    let mut estimated = 0_u64;
    for (relative, source) in files {
        let relative = safe_path::normalize_relative(relative)?;
        if relative.eq_ignore_ascii_case(BACKUP_METADATA_PATH)
            || !seen.insert(relative.to_ascii_lowercase())
        {
            return Err(format!("Duplicate or reserved backup path: {relative}"));
        }
        let metadata = source
            .metadata()
            .map_err(|error| format!("Could not inspect backup input {relative}: {error}"))?;
        if !metadata.is_file() {
            return Err(format!("Backup input is not a regular file: {relative}"));
        }
        estimated = estimated
            .checked_add(metadata.len())
            .ok_or_else(|| "Backup size overflowed".to_string())?;
        normalized_files.push((
            BackupFile {
                path: relative,
                size: metadata.len(),
                sha256: manifest::sha256(source)?,
            },
            source.clone(),
        ));
    }
    normalized_files.sort_by(|left, right| left.0.path.cmp(&right.0.path));

    let mut normalized_remove = Vec::new();
    for relative in remove_on_restore {
        let relative = safe_path::normalize_relative(relative)?;
        if relative.eq_ignore_ascii_case(BACKUP_METADATA_PATH)
            || seen.contains(&relative.to_ascii_lowercase())
        {
            return Err(format!(
                "A backup cannot both restore and remove the same path: {relative}"
            ));
        }
        if !normalized_remove
            .iter()
            .any(|value: &String| value.eq_ignore_ascii_case(&relative))
        {
            normalized_remove.push(relative);
        }
    }
    normalized_remove.sort_by_key(|path| path.to_ascii_lowercase());

    fs::create_dir_all(backups_dir)
        .map_err(|error| format!("Could not create backup folder: {error}"))?;
    ensure_space(backups_dir, estimated)?;
    let backup_path = backups_dir.join(format!("{backup_id}.zip"));
    let temporary = backups_dir.join(format!(".{backup_id}.partial"));
    let metadata = BackupMetadata {
        schema_version: BACKUP_SCHEMA_VERSION,
        backup_id: backup_id.into(),
        profile_id: profile_id.into(),
        label: label.into(),
        created_at: unix_seconds(),
        local_modpack_version: local_modpack_version.into(),
        files: normalized_files
            .iter()
            .map(|(file, _)| file.clone())
            .collect(),
        remove_on_restore: normalized_remove,
    };
    let metadata_json = serde_json::to_vec_pretty(&metadata)
        .map_err(|error| format!("Could not serialize backup metadata: {error}"))?;

    let output = BufWriter::new(
        File::create(&temporary)
            .map_err(|error| format!("Could not create pre-change backup: {error}"))?,
    );
    let write_result = (|| {
        let mut archive = ZipWriter::new(output);
        for (entry, source) in &normalized_files {
            archive
                .start_file(&entry.path, zip_options(entry.size))
                .map_err(|error| format!("Could not add {} to backup: {error}", entry.path))?;
            io::copy(
                &mut BufReader::new(File::open(source).map_err(|error| {
                    format!("Could not read {} for backup: {error}", entry.path)
                })?),
                &mut archive,
            )
            .map_err(|error| format!("Could not back up {}: {error}", entry.path))?;
        }
        archive
            .start_file(
                BACKUP_METADATA_PATH,
                zip_options(metadata_json.len() as u64),
            )
            .map_err(|error| format!("Could not add backup metadata: {error}"))?;
        archive
            .write_all(&metadata_json)
            .map_err(|error| format!("Could not write backup metadata: {error}"))?;
        let mut output = archive
            .finish()
            .map_err(|error| format!("Could not finalize backup: {error}"))?;
        output
            .flush()
            .map_err(|error| format!("Could not flush backup: {error}"))?;
        Ok::<(), String>(())
    })();
    if let Err(error) = write_result {
        fs::remove_file(&temporary).ok();
        return Err(error);
    }
    load_metadata(&temporary, Some(profile_id), Some(backup_id))?;
    if backup_path.exists() {
        fs::remove_file(&temporary).ok();
        return Err("A backup with that identifier already exists".into());
    }
    fs::rename(&temporary, &backup_path)
        .map_err(|error| format!("Could not activate backup: {error}"))?;
    prune(backups_dir, DEFAULT_KEEP_PER_PROFILE, Some(&backup_path))?;
    Ok(backup_path)
}

pub fn list(app: &AppHandle, profile_id: &str) -> Result<Vec<RestorePointSummary>, String> {
    let config = storage::load_or_create(app)?;
    if !config
        .profiles
        .iter()
        .any(|profile| profile.id == profile_id)
    {
        return Err("That modpack profile does not exist".into());
    }
    validate_component(profile_id, "Profile id")?;
    let backups_dir = storage::data_dir(app)?.join("backups").join(profile_id);
    list_at(&backups_dir, profile_id)
}

pub fn prepare(
    app: &AppHandle,
    profile_id: &str,
    backup_id: &str,
) -> Result<RestorePreview, String> {
    let config = storage::load_or_create(app)?;
    let profile = config
        .profiles
        .iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| "That modpack profile does not exist".to_string())?;
    prepare_at(profile, backup_id, &storage::data_dir(app)?, true)
}

pub fn apply(app: &AppHandle, preview_id: &str, confirmed: bool) -> Result<RestoreOutcome, String> {
    if !confirmed {
        return Err("Restoring a backup requires explicit confirmation".into());
    }
    let plan = restore_plans()
        .lock()
        .map_err(|_| "Restore preview cache is unavailable".to_string())?
        .remove(preview_id)
        .ok_or_else(|| "Prepare a fresh restore preview before applying".to_string())?;
    let mut config = storage::load_or_create(app)?;
    let profile = config
        .profiles
        .iter_mut()
        .find(|profile| profile.id == plan.profile_id)
        .ok_or_else(|| "The previewed modpack profile no longer exists".to_string())?;
    if plan.install_dir != Path::new(profile.install_dir.trim()) {
        return Err("The modpack folder changed after preview; prepare again".into());
    }
    profile.local_modpack_version = plan.metadata.local_modpack_version.clone();
    execute_plan(&plan, None, || storage::save(app, &config))
}

pub fn delete(
    app: &AppHandle,
    profile_id: &str,
    backup_id: &str,
    confirmed: bool,
) -> Result<String, String> {
    if !confirmed {
        return Err("Deleting a restore point requires explicit confirmation".into());
    }
    let config = storage::load_or_create(app)?;
    if !config
        .profiles
        .iter()
        .any(|profile| profile.id == profile_id)
    {
        return Err("That modpack profile does not exist".into());
    }
    let backups_dir = storage::data_dir(app)?.join("backups").join(profile_id);
    let path = backup_path(&backups_dir, backup_id)?;
    reject_link(&path, "Restore point")?;
    fs::remove_file(&path).map_err(|error| format!("Could not delete restore point: {error}"))?;
    Ok("Restore point deleted.".into())
}

fn list_at(backups_dir: &Path, profile_id: &str) -> Result<Vec<RestorePointSummary>, String> {
    if !backups_dir.exists() {
        return Ok(Vec::new());
    }
    reject_link(backups_dir, "Backup folder")?;
    let mut summaries = Vec::new();
    for entry in fs::read_dir(backups_dir)
        .map_err(|error| format!("Could not list restore points: {error}"))?
    {
        let entry = entry.map_err(|error| format!("Could not inspect restore point: {error}"))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| format!("Could not inspect restore point type: {error}"))?;
        if file_type.is_symlink()
            || !file_type.is_file()
            || path.extension().and_then(|value| value.to_str()) != Some("zip")
        {
            continue;
        }
        let backup_id = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_string();
        let size_bytes = entry.metadata().map(|value| value.len()).unwrap_or(0);
        match load_metadata(&path, Some(profile_id), Some(&backup_id)) {
            Ok(metadata) => summaries.push(RestorePointSummary {
                backup_id,
                profile_id: profile_id.into(),
                label: metadata.label,
                created_at: metadata.created_at,
                size_bytes,
                file_count: metadata.files.len(),
                removes_on_restore: metadata.remove_on_restore.len(),
                local_modpack_version: metadata.local_modpack_version,
                valid: true,
                issues: Vec::new(),
            }),
            Err(error) => summaries.push(RestorePointSummary {
                backup_id,
                profile_id: profile_id.into(),
                label: "Unreadable backup".into(),
                created_at: entry
                    .metadata()
                    .ok()
                    .and_then(|value| value.modified().ok())
                    .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
                    .map(|value| value.as_secs())
                    .unwrap_or_default(),
                size_bytes,
                file_count: 0,
                removes_on_restore: 0,
                local_modpack_version: String::new(),
                valid: false,
                issues: vec![error],
            }),
        }
    }
    summaries.sort_by(|left, right| {
        right
            .created_at
            .cmp(&left.created_at)
            .then_with(|| right.backup_id.cmp(&left.backup_id))
    });
    Ok(summaries)
}

fn prepare_at(
    profile: &GameProfile,
    backup_id: &str,
    data_dir: &Path,
    remember_plan: bool,
) -> Result<RestorePreview, String> {
    validate_component(&profile.id, "Profile id")?;
    let install_dir = PathBuf::from(profile.install_dir.trim());
    if !install_dir.is_dir() {
        return Err("Choose an existing modpack folder before restoring a backup".into());
    }
    reject_link(&install_dir, "Modpack folder")?;
    ensure_data_outside_install(data_dir, &install_dir)?;
    let backups_dir = data_dir.join("backups").join(&profile.id);
    let source = backup_path(&backups_dir, backup_id)?;
    reject_link(&source, "Restore point")?;
    let metadata = load_metadata(&source, Some(&profile.id), Some(backup_id))?;

    let preview_id = format!("{}-restore-{:x}", profile.id, unix_nanos());
    let stage_root = data_dir.join("restore-staging").join(&preview_id);
    let content_dir = stage_root.join("content");
    fs::create_dir_all(&content_dir)
        .map_err(|error| format!("Could not create isolated restore staging: {error}"))?;
    if let Err(error) = extract_and_verify(&source, &content_dir, &metadata) {
        fs::remove_dir_all(&stage_root).ok();
        return Err(error);
    }

    let staged_bytes = metadata
        .files
        .iter()
        .try_fold(0_u64, |total, entry| total.checked_add(entry.size))
        .ok_or_else(|| "Restore size overflowed".to_string())?;
    let mut existing_files_to_backup = 0;
    for entry in &metadata.files {
        if guarded_join(&install_dir, &entry.path)?.exists() {
            existing_files_to_backup += 1;
        }
    }
    let mut files_to_remove = 0;
    for relative in &metadata.remove_on_restore {
        if guarded_join(&install_dir, relative)?.exists() {
            existing_files_to_backup += 1;
            files_to_remove += 1;
        }
    }
    let plan = RestorePlan {
        preview_id: preview_id.clone(),
        backup_id: backup_id.into(),
        profile_id: profile.id.clone(),
        install_dir,
        backups_dir,
        stage_root,
        content_dir,
        metadata: metadata.clone(),
    };
    if remember_plan {
        restore_plans()
            .lock()
            .map_err(|_| "Restore preview cache is unavailable".to_string())?
            .insert(preview_id.clone(), plan);
    }
    Ok(RestorePreview {
        preview_id,
        backup_id: backup_id.into(),
        profile_id: profile.id.clone(),
        label: metadata.label,
        created_at: metadata.created_at,
        local_modpack_version: metadata.local_modpack_version,
        staged_files: metadata.files.len(),
        staged_bytes,
        existing_files_to_backup,
        files_to_remove,
        ready: true,
        message: "The restore point is path-safe, CRC-valid, staged and SHA-256 verified. The live modpack has not been changed.".into(),
    })
}

fn execute_plan<F>(
    plan: &RestorePlan,
    fail_after: Option<usize>,
    finalize: F,
) -> Result<RestoreOutcome, String>
where
    F: FnOnce() -> Result<(), String>,
{
    let mut outcome = RestoreOutcome {
        profile_id: plan.profile_id.clone(),
        backup_id: plan.backup_id.clone(),
        success: false,
        restored: Vec::new(),
        removed: Vec::new(),
        recovery_backup_path: String::new(),
        rolled_back: false,
        rollback_error: String::new(),
        message: String::new(),
        error: String::new(),
    };
    let mut created = HashSet::new();
    let mut recovery_backup = None;
    let operation = (|| {
        verify_stage(plan)?;
        ensure_space(&plan.install_dir, staged_bytes(&plan.metadata.files)?)?;
        let affected = affected_paths(&plan.metadata);
        let existing = collect_existing_files(&plan.install_dir, &affected)?;
        let remove_after_rollback: Vec<_> = plan
            .metadata
            .files
            .iter()
            .filter_map(|entry| {
                guarded_join(&plan.install_dir, &entry.path)
                    .ok()
                    .filter(|path| !path.exists())
                    .map(|_| entry.path.clone())
            })
            .collect();
        let recovery_id = format!("{}-pre-restore", plan.preview_id);
        recovery_backup = Some(create_archive(
            &plan.backups_dir,
            &recovery_id,
            &plan.profile_id,
            "pre_restore",
            "",
            &existing,
            &remove_after_rollback,
        )?);
        outcome.recovery_backup_path = recovery_backup
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_default();

        for relative in &plan.metadata.remove_on_restore {
            remove_live_path(&plan.install_dir, relative, &mut outcome.removed)?;
        }
        for (index, entry) in plan.metadata.files.iter().enumerate() {
            let source = safe_path::safe_join(&plan.content_dir, &entry.path)?;
            let target = guarded_join(&plan.install_dir, &entry.path)?;
            if target.is_dir() {
                return Err(format!(
                    "Cannot restore a file over a live folder: {}",
                    entry.path
                ));
            }
            if !target.exists() {
                created.insert(entry.path.clone());
            }
            fs::create_dir_all(target.parent().unwrap_or(&plan.install_dir))
                .map_err(|error| format!("Could not create restore destination: {error}"))?;
            atomic_copy(&source, &target, &plan.preview_id)?;
            outcome.restored.push(entry.path.clone());
            if fail_after == Some(index + 1) {
                return Err("Forced restore failure".into());
            }
        }
        verify_live(&plan.install_dir, &plan.metadata)?;
        finalize()?;
        Ok::<(), String>(())
    })();

    match operation {
        Ok(()) => {
            outcome.success = true;
            outcome.message = "Restore completed successfully and every restored file passed SHA-256 verification.".into();
        }
        Err(error) => {
            outcome.error = error;
            let mutated = !outcome.restored.is_empty() || !outcome.removed.is_empty();
            if mutated {
                match rollback_restore(
                    &plan.install_dir,
                    recovery_backup.as_deref(),
                    &created,
                    &plan.preview_id,
                ) {
                    Ok(()) => {
                        outcome.rolled_back = true;
                        outcome.message =
                            "Restore failed and every recorded live change was rolled back.".into();
                    }
                    Err(error) => {
                        outcome.rollback_error = error;
                        outcome.message = "Restore failed and automatic rollback could not complete. Do not launch the game until the recovery backup is reviewed.".into();
                    }
                }
            } else {
                outcome.message = "Restore stopped before the live modpack was changed.".into();
            }
        }
    }
    fs::remove_dir_all(&plan.stage_root).ok();
    Ok(outcome)
}

fn load_metadata(
    path: &Path,
    expected_profile: Option<&str>,
    expected_id: Option<&str>,
) -> Result<BackupMetadata, String> {
    let file = File::open(path).map_err(|error| format!("Could not open backup: {error}"))?;
    let mut archive =
        ZipArchive::new(file).map_err(|error| format!("Backup is invalid: {error}"))?;
    let mut names = HashSet::new();
    let mut archive_files = HashSet::new();
    let mut metadata_json = None;
    for index in 0..archive.len() {
        let mut member = archive
            .by_index(index)
            .map_err(|error| format!("Could not inspect backup member: {error}"))?;
        let normalized = safe_path::validate_archive_member(member.name(), member.is_dir())?;
        if !names.insert(normalized.to_ascii_lowercase())
            || member.encrypted()
            || member.is_symlink()
        {
            return Err(format!("Backup contains an unsafe member: {normalized}"));
        }
        if member.is_dir() {
            continue;
        }
        if normalized.eq_ignore_ascii_case(BACKUP_METADATA_PATH) {
            let mut bytes = Vec::new();
            member
                .by_ref()
                .take(1024 * 1024)
                .read_to_end(&mut bytes)
                .map_err(|error| format!("Could not read backup metadata: {error}"))?;
            metadata_json = Some(bytes);
        } else {
            archive_files.insert(normalized.to_ascii_lowercase());
        }
        io::copy(&mut member, &mut io::sink())
            .map_err(|error| format!("Backup CRC validation failed for {normalized}: {error}"))?;
    }
    let bytes = metadata_json.ok_or_else(|| {
        "This backup predates the transactional restore format and cannot be restored safely"
            .to_string()
    })?;
    let metadata: BackupMetadata = serde_json::from_slice(&bytes)
        .map_err(|error| format!("Backup metadata is invalid: {error}"))?;
    if metadata.schema_version != BACKUP_SCHEMA_VERSION {
        return Err(format!(
            "Unsupported backup schema version: {}",
            metadata.schema_version
        ));
    }
    validate_component(&metadata.backup_id, "Backup id")?;
    validate_component(&metadata.profile_id, "Profile id")?;
    validate_component(&metadata.label, "Backup label")?;
    if expected_profile.is_some_and(|value| value != metadata.profile_id) {
        return Err("Restore point belongs to a different modpack profile".into());
    }
    if expected_id.is_some_and(|value| value != metadata.backup_id) {
        return Err("Restore point identity does not match its filename".into());
    }
    let mut declared = HashSet::new();
    for entry in &metadata.files {
        let normalized = safe_path::normalize_relative(&entry.path)?;
        if normalized != entry.path
            || normalized.eq_ignore_ascii_case(BACKUP_METADATA_PATH)
            || entry.sha256.len() != 64
            || !entry.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
            || !declared.insert(normalized.to_ascii_lowercase())
        {
            return Err(format!(
                "Backup metadata contains an invalid file: {}",
                entry.path
            ));
        }
    }
    if declared != archive_files {
        return Err("Backup contents do not exactly match the declared inventory".into());
    }
    let mut removals = HashSet::new();
    for relative in &metadata.remove_on_restore {
        let normalized = safe_path::normalize_relative(relative)?;
        if normalized != *relative
            || declared.contains(&normalized.to_ascii_lowercase())
            || !removals.insert(normalized.to_ascii_lowercase())
        {
            return Err(format!(
                "Backup metadata contains an invalid removal: {relative}"
            ));
        }
    }
    Ok(metadata)
}

fn extract_and_verify(
    source: &Path,
    content_dir: &Path,
    metadata: &BackupMetadata,
) -> Result<(), String> {
    let expected: HashMap<_, _> = metadata
        .files
        .iter()
        .map(|entry| (entry.path.to_ascii_lowercase(), entry))
        .collect();
    let file =
        File::open(source).map_err(|error| format!("Could not open restore point: {error}"))?;
    let mut archive =
        ZipArchive::new(file).map_err(|error| format!("Backup is invalid: {error}"))?;
    for index in 0..archive.len() {
        let mut member = archive
            .by_index(index)
            .map_err(|error| format!("Could not read backup member: {error}"))?;
        if member.is_dir() {
            continue;
        }
        let relative = safe_path::validate_archive_member(member.name(), false)?;
        if relative.eq_ignore_ascii_case(BACKUP_METADATA_PATH) {
            continue;
        }
        let entry = expected
            .get(&relative.to_ascii_lowercase())
            .ok_or_else(|| format!("Undeclared backup member: {relative}"))?;
        let target = safe_path::safe_join(content_dir, &relative)?;
        fs::create_dir_all(target.parent().unwrap_or(content_dir))
            .map_err(|error| format!("Could not create restore staging folder: {error}"))?;
        let mut output = BufWriter::new(
            File::create(&target)
                .map_err(|error| format!("Could not stage restored {relative}: {error}"))?,
        );
        io::copy(&mut member, &mut output)
            .map_err(|error| format!("Could not extract restored {relative}: {error}"))?;
        output
            .flush()
            .map_err(|error| format!("Could not flush restored {relative}: {error}"))?;
        if target
            .metadata()
            .map(|value| value.len())
            .unwrap_or(u64::MAX)
            != entry.size
            || manifest::sha256(&target)? != entry.sha256.to_ascii_lowercase()
        {
            return Err(format!("Restored staging hash mismatch: {relative}"));
        }
    }
    verify_stage_inventory(content_dir, &metadata.files)
}

fn verify_stage(plan: &RestorePlan) -> Result<(), String> {
    verify_stage_inventory(&plan.content_dir, &plan.metadata.files)
}

fn verify_stage_inventory(content_dir: &Path, files: &[BackupFile]) -> Result<(), String> {
    let mut found = HashSet::new();
    for entry in walk_files(content_dir)? {
        found.insert(entry.0.to_ascii_lowercase());
        let expected = files
            .iter()
            .find(|expected| expected.path.eq_ignore_ascii_case(&entry.0))
            .ok_or_else(|| format!("Unexpected restore staging file: {}", entry.0))?;
        if entry
            .1
            .metadata()
            .map(|value| value.len())
            .unwrap_or(u64::MAX)
            != expected.size
            || manifest::sha256(&entry.1)? != expected.sha256.to_ascii_lowercase()
        {
            return Err(format!(
                "Restore staging changed after preview: {}",
                entry.0
            ));
        }
    }
    if found.len() != files.len() {
        return Err("Restore staging inventory changed after preview".into());
    }
    Ok(())
}

fn verify_live(install_dir: &Path, metadata: &BackupMetadata) -> Result<(), String> {
    for entry in &metadata.files {
        let target = guarded_join(install_dir, &entry.path)?;
        if !target.is_file()
            || target
                .metadata()
                .map(|value| value.len())
                .unwrap_or(u64::MAX)
                != entry.size
            || manifest::sha256(&target)? != entry.sha256.to_ascii_lowercase()
        {
            return Err(format!("Post-restore verification failed: {}", entry.path));
        }
    }
    for relative in &metadata.remove_on_restore {
        if guarded_join(install_dir, relative)?.exists() {
            return Err(format!("Post-restore removal failed: {relative}"));
        }
    }
    Ok(())
}

fn rollback_restore(
    install_dir: &Path,
    recovery_backup: Option<&Path>,
    created: &HashSet<String>,
    transaction_id: &str,
) -> Result<(), String> {
    let mut errors = Vec::new();
    let mut created: Vec<_> = created.iter().cloned().collect();
    created.sort_by_key(|path| std::cmp::Reverse(path.matches('/').count()));
    for relative in created {
        match guarded_join(install_dir, &relative) {
            Ok(path) if path.is_file() => {
                if let Err(error) = fs::remove_file(&path) {
                    errors.push(format!("Could not remove restored {relative}: {error}"));
                } else {
                    remove_empty_parents(install_dir, path.parent());
                }
            }
            Ok(path) if path.exists() => {
                errors.push(format!("Unexpected restored path type: {relative}"));
            }
            Ok(_) => {}
            Err(error) => errors.push(error),
        }
    }
    if let Some(path) = recovery_backup
        && let Err(error) = restore_archive_files(path, install_dir, transaction_id)
    {
        errors.push(error);
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

pub(crate) fn restore_archive_files(
    path: &Path,
    install_dir: &Path,
    transaction_id: &str,
) -> Result<(), String> {
    let metadata = load_metadata(path, None, None)?;
    let temporary_root = path.with_file_name(format!(".restore-{transaction_id}"));
    fs::create_dir_all(&temporary_root)
        .map_err(|error| format!("Could not create rollback staging: {error}"))?;
    let result = (|| {
        extract_and_verify(path, &temporary_root, &metadata)?;
        for entry in &metadata.files {
            let source = safe_path::safe_join(&temporary_root, &entry.path)?;
            let target = guarded_join(install_dir, &entry.path)?;
            fs::create_dir_all(target.parent().unwrap_or(install_dir))
                .map_err(|error| format!("Could not recreate rollback folder: {error}"))?;
            atomic_copy(&source, &target, transaction_id)?;
        }
        Ok::<(), String>(())
    })();
    fs::remove_dir_all(&temporary_root).ok();
    result
}

fn affected_paths(metadata: &BackupMetadata) -> Vec<String> {
    let mut paths: Vec<_> = metadata
        .files
        .iter()
        .map(|entry| entry.path.clone())
        .collect();
    paths.extend(metadata.remove_on_restore.clone());
    paths.sort_by_key(|path| path.to_ascii_lowercase());
    paths.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    paths
}

pub(crate) fn collect_existing_files(
    install_dir: &Path,
    affected: &[String],
) -> Result<Vec<(String, PathBuf)>, String> {
    let mut collected = HashMap::new();
    for relative in affected {
        let target = guarded_join(install_dir, relative)?;
        if target.is_file() {
            collected.insert(relative.to_ascii_lowercase(), (relative.clone(), target));
        } else if target.is_dir() {
            for entry in walk_files(&target)? {
                let relative = entry
                    .1
                    .strip_prefix(install_dir)
                    .map_err(|_| "A backup input escaped the modpack folder".to_string())?
                    .to_string_lossy()
                    .replace('\\', "/");
                let relative = safe_path::normalize_relative(&relative)?;
                collected.insert(relative.to_ascii_lowercase(), (relative, entry.1));
            }
        } else if target.exists() {
            return Err(format!("Unsupported affected path type: {relative}"));
        }
    }
    let mut files: Vec<_> = collected.into_values().collect();
    files.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(files)
}

fn walk_files(root: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    let mut files = Vec::new();
    if !root.exists() {
        return Ok(files);
    }
    for item in walkdir::WalkDir::new(root)
        .follow_links(false)
        .sort_by_file_name()
    {
        let item = item.map_err(|error| format!("Could not inspect backup content: {error}"))?;
        if item.file_type().is_symlink() {
            return Err(format!(
                "Symbolic links are not supported in backup content: {}",
                item.path().display()
            ));
        }
        if item.file_type().is_file() {
            let relative = item
                .path()
                .strip_prefix(root)
                .map_err(|_| "Backup content escaped its root".to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            files.push((safe_path::normalize_relative(&relative)?, item.into_path()));
        }
    }
    Ok(files)
}

fn remove_live_path(
    install_dir: &Path,
    relative: &str,
    removed: &mut Vec<String>,
) -> Result<(), String> {
    let target = guarded_join(install_dir, relative)?;
    if !target.exists() {
        return Ok(());
    }
    if target.is_file() {
        fs::remove_file(&target)
            .map_err(|error| format!("Could not remove restored-new file {relative}: {error}"))?;
    } else if target.is_dir() {
        fs::remove_dir_all(&target)
            .map_err(|error| format!("Could not remove restored-new folder {relative}: {error}"))?;
    } else {
        return Err(format!("Unsupported restore removal path type: {relative}"));
    }
    removed.push(relative.into());
    Ok(())
}

fn backup_path(backups_dir: &Path, backup_id: &str) -> Result<PathBuf, String> {
    validate_component(backup_id, "Backup id")?;
    let path = backups_dir.join(format!("{backup_id}.zip"));
    if !path.is_file() {
        return Err("Restore point was not found".into());
    }
    Ok(path)
}

fn prune(backups_dir: &Path, keep: usize, protected: Option<&Path>) -> Result<(), String> {
    let mut backups = Vec::new();
    for entry in fs::read_dir(backups_dir)
        .map_err(|error| format!("Could not inspect backup retention: {error}"))?
    {
        let entry =
            entry.map_err(|error| format!("Could not inspect backup retention: {error}"))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("Could not inspect backup retention type: {error}"))?;
        if file_type.is_file()
            && !file_type.is_symlink()
            && entry.path().extension().and_then(|value| value.to_str()) == Some("zip")
        {
            let modified = entry
                .metadata()
                .ok()
                .and_then(|value| value.modified().ok())
                .unwrap_or(UNIX_EPOCH);
            backups.push((modified, entry.path()));
        }
    }
    backups.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| right.1.cmp(&left.1)));
    let protected = protected.map(|path| path.to_path_buf());
    let mut retained = 0;
    for (_, path) in backups {
        if protected.as_ref().is_some_and(|value| value == &path) || retained < keep {
            retained += 1;
            continue;
        }
        fs::remove_file(&path)
            .map_err(|error| format!("Could not prune old restore point: {error}"))?;
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
        return Err(
            "Launcher restore staging and backups must be outside the live modpack folder".into(),
        );
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
                "Symbolic links are not allowed in restored paths: {}",
                current.display()
            ));
        }
        let resolved = fs::canonicalize(&current)
            .map_err(|error| format!("Could not resolve {}: {error}", current.display()))?;
        if !resolved.starts_with(&canonical_root) {
            return Err(format!(
                "Restored path escapes the configured folder: {relative}"
            ));
        }
    }
    Ok(target)
}

fn atomic_copy(source: &Path, target: &Path, transaction_id: &str) -> Result<(), String> {
    let file_name = target
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "Restore destination file name is not valid Unicode".to_string())?;
    let temporary = target.with_file_name(format!(".{file_name}.{transaction_id}.partial"));
    fs::copy(source, &temporary)
        .map_err(|error| format!("Could not stage restored {}: {error}", target.display()))?;
    OpenOptions::new()
        .write(true)
        .open(&temporary)
        .and_then(|file| file.sync_all())
        .map_err(|error| format!("Could not sync restored {}: {error}", target.display()))?;
    if target.exists() {
        fs::remove_file(target)
            .map_err(|error| format!("Could not replace {}: {error}", target.display()))?;
    }
    fs::rename(&temporary, target)
        .map_err(|error| format!("Could not activate {}: {error}", target.display()))
}

fn ensure_space(path: &Path, required: u64) -> Result<(), String> {
    fs::create_dir_all(path)
        .map_err(|error| format!("Could not create disk-space check folder: {error}"))?;
    let available = available_space(path)
        .map_err(|error| format!("Could not check available disk space: {error}"))?;
    let needed = required
        .checked_add(FREE_SPACE_BUFFER)
        .ok_or_else(|| "Required disk space overflowed".to_string())?;
    if available < needed {
        return Err(format!(
            "Not enough disk space: need approximately {} MiB plus working room",
            required.div_ceil(1024 * 1024)
        ));
    }
    Ok(())
}

fn staged_bytes(files: &[BackupFile]) -> Result<u64, String> {
    files
        .iter()
        .try_fold(0_u64, |total, entry| total.checked_add(entry.size))
        .ok_or_else(|| "Restore size overflowed".to_string())
}

fn reject_link(path: &Path, label: &str) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("Could not inspect {label}: {error}"))?;
    if metadata.file_type().is_symlink() {
        return Err(format!("{label} cannot be a symbolic link"));
    }
    Ok(())
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

fn zip_options(size: u64) -> SimpleFileOptions {
    SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .compression_level(Some(6))
        .last_modified_time(DateTime::default())
        .unix_permissions(0o644)
        .large_file(size > u64::from(u32::MAX))
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

fn restore_plans() -> &'static Mutex<HashMap<String, RestorePlan>> {
    RESTORE_PLANS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn profile(install: &Path) -> GameProfile {
        let mut profile = crate::models::LauncherConfig::default().profiles.remove(0);
        profile.id = "fixture".into();
        profile.install_dir = install.display().to_string();
        profile.local_modpack_version = "1.0.0".into();
        profile
    }

    #[test]
    fn restore_is_previewed_and_removes_files_created_by_update() {
        let temp = TempDir::new().unwrap();
        let install = temp.path().join("install");
        let data = temp.path().join("data");
        let backups = data.join("backups/fixture");
        fs::create_dir_all(&install).unwrap();
        fs::write(install.join("existing.txt"), b"old").unwrap();
        let files = vec![("existing.txt".into(), install.join("existing.txt"))];
        create_archive(
            &backups,
            "point-one",
            "fixture",
            "pre_update",
            "1.0.0",
            &files,
            &["new.txt".into()],
        )
        .unwrap();
        fs::write(install.join("existing.txt"), b"new").unwrap();
        fs::write(install.join("new.txt"), b"created").unwrap();

        let preview = prepare_at(&profile(&install), "point-one", &data, true).unwrap();
        assert_eq!(preview.staged_files, 1);
        assert_eq!(preview.files_to_remove, 1);
        let plan = restore_plans()
            .lock()
            .unwrap()
            .remove(&preview.preview_id)
            .unwrap();
        let outcome = execute_plan(&plan, None, || Ok(())).unwrap();

        assert!(outcome.success);
        assert_eq!(fs::read(install.join("existing.txt")).unwrap(), b"old");
        assert!(!install.join("new.txt").exists());
        assert!(!outcome.recovery_backup_path.is_empty());
    }

    #[test]
    fn failed_restore_rolls_back_current_files_and_removals() {
        let temp = TempDir::new().unwrap();
        let install = temp.path().join("install");
        let data = temp.path().join("data");
        let backups = data.join("backups/fixture");
        fs::create_dir_all(&install).unwrap();
        fs::write(install.join("a.txt"), b"backup-a").unwrap();
        fs::write(install.join("b.txt"), b"backup-b").unwrap();
        let files = vec![
            ("a.txt".into(), install.join("a.txt")),
            ("b.txt".into(), install.join("b.txt")),
        ];
        create_archive(
            &backups,
            "point-two",
            "fixture",
            "pre_update",
            "1.0.0",
            &files,
            &["new.txt".into()],
        )
        .unwrap();
        fs::write(install.join("a.txt"), b"current-a").unwrap();
        fs::write(install.join("b.txt"), b"current-b").unwrap();
        fs::write(install.join("new.txt"), b"current-new").unwrap();

        let preview = prepare_at(&profile(&install), "point-two", &data, true).unwrap();
        let plan = restore_plans()
            .lock()
            .unwrap()
            .remove(&preview.preview_id)
            .unwrap();
        let outcome = execute_plan(&plan, Some(1), || Ok(())).unwrap();

        assert!(!outcome.success);
        assert!(outcome.rolled_back);
        assert_eq!(fs::read(install.join("a.txt")).unwrap(), b"current-a");
        assert_eq!(fs::read(install.join("b.txt")).unwrap(), b"current-b");
        assert_eq!(fs::read(install.join("new.txt")).unwrap(), b"current-new");
    }

    #[test]
    fn unsafe_or_legacy_archive_never_reaches_live_install() {
        let temp = TempDir::new().unwrap();
        let install = temp.path().join("install");
        let data = temp.path().join("data");
        let backups = data.join("backups/fixture");
        fs::create_dir_all(&install).unwrap();
        fs::create_dir_all(&backups).unwrap();
        let output = File::create(backups.join("unsafe.zip")).unwrap();
        let mut archive = ZipWriter::new(output);
        archive.start_file("../escape.txt", zip_options(3)).unwrap();
        archive.write_all(b"bad").unwrap();
        archive.finish().unwrap();

        assert!(prepare_at(&profile(&install), "unsafe", &data, true).is_err());
        assert!(!temp.path().join("escape.txt").exists());
        assert!(fs::read_dir(&install).unwrap().next().is_none());
    }

    #[test]
    fn retention_keeps_the_newest_five_restore_points() {
        let temp = TempDir::new().unwrap();
        let install = temp.path().join("install");
        let backups = temp.path().join("backups");
        fs::create_dir_all(&install).unwrap();
        fs::write(install.join("file.txt"), b"data").unwrap();
        let files = vec![("file.txt".into(), install.join("file.txt"))];
        for index in 0..7 {
            create_archive(
                &backups,
                &format!("point-{index}"),
                "fixture",
                "manual",
                "1.0.0",
                &files,
                &[],
            )
            .unwrap();
        }
        assert_eq!(list_at(&backups, "fixture").unwrap().len(), 5);
    }
}
