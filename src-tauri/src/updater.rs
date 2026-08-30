use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{self, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use fs2::available_space;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use walkdir::WalkDir;
use zip::ZipArchive;
#[cfg(test)]
use zip::{ZipWriter, write::SimpleFileOptions};

use crate::{
    manifest::{self, FileEntry, Manifest},
    models::GameProfile,
    restore_points, safe_path, storage,
};

const FREE_SPACE_BUFFER: u64 = 64 * 1024 * 1024;
const DOWNLOAD_ATTEMPTS: usize = 3;
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TransactionKind {
    Update,
    Repair,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionRequest {
    pub profile_id: String,
    pub kind: TransactionKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionPreview {
    pub preview_id: String,
    pub profile_id: String,
    pub kind: TransactionKind,
    pub version: String,
    pub source: String,
    pub staged_files: usize,
    pub staged_bytes: u64,
    pub existing_files_to_backup: usize,
    pub new_files: usize,
    pub obsolete_paths: usize,
    pub issues: Vec<String>,
    pub ready: bool,
    pub nothing_to_do: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionOutcome {
    pub profile_id: String,
    pub kind: TransactionKind,
    pub success: bool,
    pub applied: Vec<String>,
    pub removed: Vec<String>,
    pub backup_path: String,
    pub rolled_back: bool,
    pub rollback_error: String,
    pub message: String,
    pub error: String,
}

#[derive(Debug, Clone)]
struct StageEntry {
    relative: String,
    size: u64,
    hash: String,
}

#[derive(Debug, Clone)]
struct TransactionPlan {
    preview_id: String,
    profile_id: String,
    kind: TransactionKind,
    version: String,
    previous_version: String,
    stage_root: PathBuf,
    content_dir: PathBuf,
    install_dir: PathBuf,
    backups_dir: PathBuf,
    manifest: Manifest,
    inventory: Vec<StageEntry>,
    obsolete_paths: Vec<String>,
}

static TRANSACTION_PLANS: OnceLock<Mutex<HashMap<String, TransactionPlan>>> = OnceLock::new();

pub fn prepare(
    app: &AppHandle,
    request: &TransactionRequest,
) -> Result<TransactionPreview, String> {
    let config = storage::load_or_create(app)?;
    let profile = config
        .profiles
        .iter()
        .find(|profile| profile.id == request.profile_id)
        .ok_or_else(|| "That modpack profile does not exist".to_string())?;
    let loaded = manifest::load_for_profile(app, profile);
    if !loaded.summary.valid {
        return Err(format!(
            "The trusted manifest cannot be used: {}",
            loaded.summary.errors.join("; ")
        ));
    }
    let data_dir = storage::data_dir(app)?;
    prepare_at(profile, &loaded.manifest, request.kind, &data_dir, true)
}

pub fn apply(
    app: &AppHandle,
    preview_id: &str,
    confirmed: bool,
) -> Result<TransactionOutcome, String> {
    require_confirmation(confirmed)?;
    let plan = transaction_plans()
        .lock()
        .map_err(|_| "Update preview cache is unavailable".to_string())?
        .remove(preview_id)
        .ok_or_else(|| "Prepare a fresh update or repair preview before applying".to_string())?;

    let mut config = storage::load_or_create(app)?;
    let profile = config
        .profiles
        .iter_mut()
        .find(|profile| profile.id == plan.profile_id)
        .ok_or_else(|| "The previewed modpack profile no longer exists".to_string())?;
    let expected_install = PathBuf::from(profile.install_dir.trim());
    if expected_install != plan.install_dir {
        return Err("The modpack folder changed after preview; prepare again".into());
    }
    profile.local_modpack_version = plan.version.clone();
    execute_plan(&plan, None, || storage::save(app, &config))
}

fn require_confirmation(confirmed: bool) -> Result<(), String> {
    if confirmed {
        Ok(())
    } else {
        Err("Applying an update or repair requires explicit confirmation".into())
    }
}

fn prepare_at(
    profile: &GameProfile,
    manifest: &Manifest,
    kind: TransactionKind,
    data_dir: &Path,
    remember_plan: bool,
) -> Result<TransactionPreview, String> {
    let manifest_issues = manifest::validate(manifest, Some(profile));
    if !manifest_issues.is_empty() {
        return Err(format!(
            "The trusted manifest cannot be used: {}",
            manifest_issues.join("; ")
        ));
    }
    validate_profile_component(&profile.id)?;
    let install_dir = PathBuf::from(profile.install_dir.trim());
    if !install_dir.is_dir() {
        return Err(
            "Choose an existing modpack folder before preparing an update or repair".into(),
        );
    }
    reject_root_link(&install_dir)?;

    let data_existed = data_dir.exists();
    fs::create_dir_all(data_dir)
        .map_err(|error| format!("Could not create launcher data folder: {error}"))?;
    let canonical_install = fs::canonicalize(&install_dir)
        .map_err(|error| format!("Could not resolve modpack folder: {error}"))?;
    let canonical_data = fs::canonicalize(data_dir)
        .map_err(|error| format!("Could not resolve launcher data folder: {error}"))?;
    if canonical_data.starts_with(&canonical_install) {
        if !data_existed {
            fs::remove_dir(&canonical_data).ok();
        }
        return Err("Launcher staging and backups must be outside the live modpack folder".into());
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let preview_id = format!("{}-{}-{stamp:x}", profile.id, kind_label(kind));
    let stage_root = data_dir.join("update-staging").join(&preview_id);
    let content_dir = stage_root.join("content");
    let backups_dir = data_dir.join("backups").join(&profile.id);
    fs::create_dir_all(&content_dir)
        .map_err(|error| format!("Could not create isolated update staging: {error}"))?;

    let preparation = (|| {
        let bad_required = mismatched_required(manifest, &install_dir)?;
        let obsolete_existing = existing_obsolete(manifest, &install_dir)?;
        let version_current = version_marker_matches(&install_dir, &manifest.modpack_version);
        let nothing_to_do = kind == TransactionKind::Repair
            && bad_required.is_empty()
            && obsolete_existing.is_empty()
            && version_current;
        if nothing_to_do {
            return Ok((String::new(), Vec::new(), obsolete_existing, true));
        }

        let source = if kind == TransactionKind::Repair && bad_required.is_empty() {
            String::new()
        } else {
            prepare_package(profile, manifest, &stage_root)?
        };
        let wanted = if kind == TransactionKind::Repair {
            Some(
                bad_required
                    .iter()
                    .map(|entry| entry.path.to_ascii_lowercase())
                    .collect::<HashSet<_>>(),
            )
        } else {
            None
        };
        if !source.is_empty() {
            extract_validated_package(
                &stage_root.join("update.zip"),
                &content_dir,
                wanted.as_ref(),
            )?;
        }
        let inventory = inventory(&content_dir)?;
        let issues = verify_candidate(kind, manifest, &install_dir, &content_dir, &bad_required)?;
        if !issues.is_empty() {
            return Err(format!(
                "The staged candidate cannot produce a valid installation: {}",
                issues.join("; ")
            ));
        }
        Ok((source, inventory, obsolete_existing, false))
    })();

    let (source, inventory, obsolete_paths, nothing_to_do) = match preparation {
        Ok(value) => value,
        Err(error) => {
            let _ = fs::remove_dir_all(&stage_root);
            return Err(error);
        }
    };

    let staged_bytes = inventory
        .iter()
        .try_fold(0_u64, |total, entry| total.checked_add(entry.size));
    let staged_bytes = staged_bytes.ok_or_else(|| "Staged file sizes overflowed".to_string())?;
    let mut backup_count = 0;
    let mut new_files = 0;
    for entry in &inventory {
        let target = guarded_join(&install_dir, &entry.relative)?;
        if target.exists() {
            backup_count += 1;
        } else {
            new_files += 1;
        }
    }
    backup_count += obsolete_paths.len();
    if install_dir.join("modpack_version.txt").is_file() {
        backup_count += 1;
    }

    let plan = TransactionPlan {
        preview_id: preview_id.clone(),
        profile_id: profile.id.clone(),
        kind,
        version: manifest.modpack_version.clone(),
        previous_version: profile.local_modpack_version.clone(),
        stage_root: stage_root.clone(),
        content_dir,
        install_dir: install_dir.clone(),
        backups_dir,
        manifest: manifest.clone(),
        inventory: inventory.clone(),
        obsolete_paths: obsolete_paths.clone(),
    };
    if remember_plan && !nothing_to_do {
        transaction_plans()
            .lock()
            .map_err(|_| "Update preview cache is unavailable".to_string())?
            .insert(preview_id.clone(), plan);
    }
    if nothing_to_do {
        let _ = fs::remove_dir_all(&stage_root);
    }

    Ok(TransactionPreview {
        preview_id,
        profile_id: profile.id.clone(),
        kind,
        version: manifest.modpack_version.clone(),
        source,
        staged_files: inventory.len(),
        staged_bytes,
        existing_files_to_backup: backup_count,
        new_files,
        obsolete_paths: obsolete_paths.len(),
        issues: Vec::new(),
        ready: !nothing_to_do,
        nothing_to_do,
        message: if nothing_to_do {
            "All required files and the installed version marker already match the trusted manifest."
                .into()
        } else {
            format!(
                "{} candidate is staged and verified. The live modpack has not been changed.",
                title_kind(kind)
            )
        },
    })
}

fn prepare_package(
    profile: &GameProfile,
    manifest: &Manifest,
    stage_root: &Path,
) -> Result<String, String> {
    let package = stage_root.join("update.zip");
    if !manifest.update_parts.is_empty() {
        let combined_size = manifest.update_parts.iter().try_fold(0_u64, |total, part| {
            u64::try_from(part.size)
                .ok()
                .and_then(|size| total.checked_add(size))
        });
        if let Some(size) = combined_size {
            ensure_space(stage_root, size.saturating_mul(2))?;
        }
        let mut combined = BufWriter::new(
            File::create(&package)
                .map_err(|error| format!("Could not create multipart assembly: {error}"))?,
        );
        for (index, part) in manifest.update_parts.iter().enumerate() {
            let part_path = stage_root.join(format!("update.part{:03}", index + 1));
            fetch_source(&part.url, &part_path, Some(&part.sha256))?;
            let actual_size = part_path
                .metadata()
                .map_err(|error| format!("Could not inspect update part {}: {error}", index + 1))?
                .len();
            if part.size >= 0 && u64::try_from(part.size).ok() != Some(actual_size) {
                return Err(format!(
                    "Update part {} size does not match the manifest",
                    index + 1
                ));
            }
            io::copy(
                &mut BufReader::new(File::open(&part_path).map_err(|error| {
                    format!("Could not reopen update part {}: {error}", index + 1)
                })?),
                &mut combined,
            )
            .map_err(|error| format!("Could not assemble update part {}: {error}", index + 1))?;
            fs::remove_file(&part_path).ok();
        }
        combined
            .flush()
            .map_err(|error| format!("Could not finish multipart assembly: {error}"))?;
        drop(combined);
        if !manifest.update_sha256.trim().is_empty()
            && !manifest::sha256(&package)?.eq_ignore_ascii_case(&manifest.update_sha256)
        {
            fs::remove_file(&package).ok();
            return Err(
                "Reassembled update package SHA-256 does not match the trusted manifest".into(),
            );
        }
        return Ok(format!(
            "{} verified release parts",
            manifest.update_parts.len()
        ));
    }

    let source = if !manifest.update_url.trim().is_empty() {
        manifest.update_url.trim()
    } else {
        profile.update_source.trim()
    };
    if source.is_empty() {
        return Err("No dedicated modpack update package is configured".into());
    }
    let expected_hash = if manifest.update_sha256.trim().is_empty() {
        None
    } else {
        Some(manifest.update_sha256.as_str())
    };
    if source.starts_with("https://") && expected_hash.is_none() {
        return Err("Remote modpack updates require a trusted SHA-256 package checksum".into());
    }
    fetch_source(source, &package, expected_hash)?;
    Ok(source.into())
}

fn fetch_source(
    source: &str,
    destination: &Path,
    expected_hash: Option<&str>,
) -> Result<(), String> {
    let source = source.trim();
    if manifest::is_discord_invite(source) {
        return Err("Discord invitations cannot be used as modpack update sources".into());
    }
    if source.starts_with("https://") {
        download_https(source, destination)?;
    } else if source.contains("://") {
        return Err("Remote modpack updates must use HTTPS".into());
    } else {
        let source_path = PathBuf::from(source);
        if !source_path.is_file() {
            return Err(format!("Local update package was not found: {source}"));
        }
        ensure_space(
            destination.parent().unwrap_or(Path::new(".")),
            source_path
                .metadata()
                .map_err(|error| format!("Could not inspect local update package: {error}"))?
                .len(),
        )?;
        let partial = partial_path(destination);
        fs::copy(&source_path, &partial)
            .map_err(|error| format!("Could not stage local update package: {error}"))?;
        fs::rename(&partial, destination)
            .map_err(|error| format!("Could not activate staged update package: {error}"))?;
    }
    if let Some(expected) = expected_hash {
        let actual = manifest::sha256(destination)?;
        if !actual.eq_ignore_ascii_case(expected) {
            fs::remove_file(destination).ok();
            return Err("Update package SHA-256 does not match the trusted manifest".into());
        }
    }
    Ok(())
}

fn download_https(url: &str, destination: &Path) -> Result<(), String> {
    let partial = partial_path(destination);
    let mut last_error = String::new();
    for _attempt in 1..=DOWNLOAD_ATTEMPTS {
        fs::remove_file(&partial).ok();
        let result = (|| {
            let config = ureq::Agent::config_builder()
                .timeout_global(Some(DOWNLOAD_TIMEOUT))
                .build();
            let agent: ureq::Agent = config.into();
            let mut response = agent
                .get(url)
                .call()
                .map_err(|error| format!("HTTPS request failed: {error}"))?;
            let content_length = response
                .headers()
                .get("content-length")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok());
            if let Some(size) = content_length {
                ensure_space(destination.parent().unwrap_or(Path::new(".")), size)?;
            }
            let mut output = BufWriter::new(
                File::create(&partial)
                    .map_err(|error| format!("Could not create partial download: {error}"))?,
            );
            io::copy(&mut response.body_mut().as_reader(), &mut output)
                .map_err(|error| format!("Download was interrupted: {error}"))?;
            output
                .flush()
                .map_err(|error| format!("Could not flush downloaded package: {error}"))?;
            fs::rename(&partial, destination)
                .map_err(|error| format!("Could not activate downloaded package: {error}"))?;
            Ok::<(), String>(())
        })();
        match result {
            Ok(()) => return Ok(()),
            Err(error) => {
                last_error = error;
                fs::remove_file(&partial).ok();
            }
        }
    }
    Err(format!(
        "Download failed after {DOWNLOAD_ATTEMPTS} attempts: {last_error}"
    ))
}

fn extract_validated_package(
    package: &Path,
    destination: &Path,
    wanted: Option<&HashSet<String>>,
) -> Result<(), String> {
    let file = File::open(package)
        .map_err(|error| format!("Could not open staged update package: {error}"))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| format!("Update package is not a valid ZIP: {error}"))?;
    let mut seen = HashSet::new();
    let mut total_size = 0_u64;
    for index in 0..archive.len() {
        let mut member = archive
            .by_index(index)
            .map_err(|error| format!("Could not inspect ZIP member {index}: {error}"))?;
        let normalized = safe_path::validate_archive_member(member.name(), member.is_dir())?;
        if !seen.insert(normalized.to_ascii_lowercase()) {
            return Err(format!(
                "Duplicate or case-colliding archive member: {normalized}"
            ));
        }
        if member.encrypted() {
            return Err(format!(
                "Encrypted archive member is not supported: {normalized}"
            ));
        }
        if member.is_symlink() || (!member.is_dir() && !member.is_file()) {
            return Err(format!(
                "Archive links and special files are not allowed: {normalized}"
            ));
        }
        if member.is_file() {
            total_size = total_size
                .checked_add(member.size())
                .ok_or_else(|| "Archive uncompressed size overflowed".to_string())?;
            io::copy(&mut member, &mut io::sink())
                .map_err(|error| format!("CRC validation failed for {normalized}: {error}"))?;
        }
    }
    ensure_space(destination, total_size)?;

    for index in 0..archive.len() {
        let mut member = archive
            .by_index(index)
            .map_err(|error| format!("Could not reopen ZIP member {index}: {error}"))?;
        let normalized = safe_path::validate_archive_member(member.name(), member.is_dir())?;
        if member.is_dir() {
            continue;
        }
        if wanted.is_some_and(|paths| !paths.contains(&normalized.to_ascii_lowercase())) {
            continue;
        }
        let target = safe_path::safe_join(destination, &normalized)?;
        fs::create_dir_all(target.parent().unwrap_or(destination))
            .map_err(|error| format!("Could not create staging folder: {error}"))?;
        let mut output = BufWriter::new(
            File::create(&target)
                .map_err(|error| format!("Could not stage {normalized}: {error}"))?,
        );
        io::copy(&mut member, &mut output)
            .map_err(|error| format!("Could not extract {normalized}: {error}"))?;
        output
            .flush()
            .map_err(|error| format!("Could not flush staged file {normalized}: {error}"))?;
    }
    Ok(())
}

fn mismatched_required<'a>(
    manifest: &'a Manifest,
    install_dir: &Path,
) -> Result<Vec<&'a FileEntry>, String> {
    manifest
        .files
        .iter()
        .filter(|entry| entry.required)
        .filter_map(|entry| match entry_matches(entry, install_dir) {
            Ok(true) => None,
            Ok(false) => Some(Ok(entry)),
            Err(error) => Some(Err(error)),
        })
        .collect()
}

fn entry_matches(entry: &FileEntry, root: &Path) -> Result<bool, String> {
    let path = safe_path::safe_join(root, &entry.path)?;
    if !path.is_file() {
        return Ok(false);
    }
    let size = path
        .metadata()
        .map_err(|error| format!("Could not inspect {}: {error}", entry.path))?
        .len();
    Ok(u64::try_from(entry.size).ok() == Some(size)
        && manifest::sha256(&path)?.eq_ignore_ascii_case(&entry.hash))
}

fn verify_candidate(
    kind: TransactionKind,
    manifest: &Manifest,
    install_dir: &Path,
    content_dir: &Path,
    bad_required: &[&FileEntry],
) -> Result<Vec<String>, String> {
    let entries: Vec<&FileEntry> = if kind == TransactionKind::Repair {
        bad_required.to_vec()
    } else {
        manifest
            .files
            .iter()
            .filter(|entry| entry.required)
            .collect()
    };
    let mut issues = Vec::new();
    for entry in entries {
        let staged = safe_path::safe_join(content_dir, &entry.path)?;
        let root = if staged.is_file() {
            content_dir
        } else {
            install_dir
        };
        if !entry_matches(entry, root)? {
            issues.push(entry.path.clone());
        }
    }
    Ok(issues)
}

fn inventory(root: &Path) -> Result<Vec<StageEntry>, String> {
    let mut entries = Vec::new();
    for item in WalkDir::new(root).follow_links(false).sort_by_file_name() {
        let item = item.map_err(|error| format!("Could not inventory staging: {error}"))?;
        if item.depth() == 0 || item.file_type().is_dir() {
            continue;
        }
        if item.file_type().is_symlink() {
            return Err("A symbolic link appeared in isolated staging".into());
        }
        let relative = item
            .path()
            .strip_prefix(root)
            .map_err(|_| "A staged path escaped its root".to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        let relative = safe_path::normalize_relative(&relative)?;
        let size = item
            .metadata()
            .map_err(|error| format!("Could not inspect staged {relative}: {error}"))?
            .len();
        entries.push(StageEntry {
            relative,
            size,
            hash: manifest::sha256(item.path())?,
        });
    }
    entries.sort_by(|left, right| left.relative.cmp(&right.relative));
    Ok(entries)
}

fn execute_plan<F>(
    plan: &TransactionPlan,
    fail_after: Option<usize>,
    finalize: F,
) -> Result<TransactionOutcome, String>
where
    F: FnOnce() -> Result<(), String>,
{
    let mut outcome = TransactionOutcome {
        profile_id: plan.profile_id.clone(),
        kind: plan.kind,
        success: false,
        applied: Vec::new(),
        removed: Vec::new(),
        backup_path: String::new(),
        rolled_back: false,
        rollback_error: String::new(),
        message: String::new(),
        error: String::new(),
    };
    let mut created = HashSet::new();
    let mut backup_path = None;
    let operation = (|| {
        verify_plan(plan)?;
        ensure_space(&plan.install_dir, staged_bytes(&plan.inventory)?)?;
        let affected = affected_paths(plan);
        backup_path = create_backup(plan, &affected)?;
        if let Some(path) = &backup_path {
            outcome.backup_path = path.display().to_string();
        }

        for (index, entry) in plan.inventory.iter().enumerate() {
            apply_one(plan, &entry.relative, &mut outcome.applied, &mut created)?;
            if fail_after == Some(index + 1) {
                return Err("Forced apply failure".into());
            }
        }
        remove_obsolete(plan, &mut outcome.removed)?;
        let post_issues = verify_required_install(&plan.manifest, &plan.install_dir)?;
        if !post_issues.is_empty() {
            return Err(format!(
                "Post-update verification failed for {} required file(s): {}",
                post_issues.len(),
                post_issues
                    .iter()
                    .take(5)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        apply_version_marker(plan, &mut outcome.applied, &mut created)?;
        finalize()?;
        Ok::<(), String>(())
    })();

    match operation {
        Ok(()) => {
            outcome.success = true;
            outcome.message = format!(
                "{} completed successfully and passed post-install verification.",
                title_kind(plan.kind)
            );
        }
        Err(error) => {
            outcome.error = error;
            let mutated = !outcome.applied.is_empty() || !outcome.removed.is_empty();
            if mutated {
                match rollback(plan, backup_path.as_deref(), &created) {
                    Ok(()) => {
                        outcome.rolled_back = true;
                        outcome.message = format!(
                            "{} failed and every recorded live change was rolled back.",
                            title_kind(plan.kind)
                        );
                    }
                    Err(rollback_error) => {
                        outcome.rollback_error = rollback_error;
                        outcome.message = format!(
                            "{} failed and automatic rollback could not complete. Do not launch the game until the backup is restored.",
                            title_kind(plan.kind)
                        );
                    }
                }
            } else {
                outcome.message = format!(
                    "{} stopped before the live modpack was changed.",
                    title_kind(plan.kind)
                );
            }
        }
    }
    fs::remove_dir_all(&plan.stage_root).ok();
    Ok(outcome)
}

fn verify_plan(plan: &TransactionPlan) -> Result<(), String> {
    reject_root_link(&plan.install_dir)?;
    let current = inventory(&plan.content_dir)?;
    if current.len() != plan.inventory.len()
        || current.iter().zip(&plan.inventory).any(|(left, right)| {
            left.relative != right.relative || left.size != right.size || left.hash != right.hash
        })
    {
        return Err("Isolated staging changed after preview; prepare again".into());
    }
    let issues = if plan.kind == TransactionKind::Repair {
        let expected: Vec<_> = plan
            .inventory
            .iter()
            .filter_map(|staged| {
                plan.manifest
                    .files
                    .iter()
                    .find(|entry| entry.path.eq_ignore_ascii_case(&staged.relative))
            })
            .collect();
        verify_candidate(
            plan.kind,
            &plan.manifest,
            &plan.install_dir,
            &plan.content_dir,
            &expected,
        )?
    } else {
        verify_candidate(
            plan.kind,
            &plan.manifest,
            &plan.install_dir,
            &plan.content_dir,
            &[],
        )?
    };
    if issues.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "The preview can no longer produce a valid install: {}",
            issues.join(", ")
        ))
    }
}

fn affected_paths(plan: &TransactionPlan) -> Vec<String> {
    let mut paths: Vec<_> = plan
        .inventory
        .iter()
        .map(|entry| entry.relative.clone())
        .collect();
    paths.extend(plan.obsolete_paths.clone());
    paths.push("modpack_version.txt".into());
    paths.sort_by_key(|path| path.to_ascii_lowercase());
    paths.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    paths
}

fn create_backup(plan: &TransactionPlan, affected: &[String]) -> Result<Option<PathBuf>, String> {
    let files = restore_points::collect_existing_files(&plan.install_dir, affected)?;
    let mut remove_on_restore = Vec::new();
    for entry in &plan.inventory {
        if !guarded_join(&plan.install_dir, &entry.relative)?.exists() {
            remove_on_restore.push(entry.relative.clone());
        }
    }
    if !plan.install_dir.join("modpack_version.txt").exists() {
        remove_on_restore.push("modpack_version.txt".into());
    }
    let backup_id = format!("{}_pre_{}", plan.preview_id, kind_label(plan.kind));
    restore_points::create_archive(
        &plan.backups_dir,
        &backup_id,
        &plan.profile_id,
        &format!("pre_{}", kind_label(plan.kind)),
        &plan.previous_version,
        &files,
        &remove_on_restore,
    )
    .map(Some)
}

fn apply_one(
    plan: &TransactionPlan,
    relative: &str,
    applied: &mut Vec<String>,
    created: &mut HashSet<String>,
) -> Result<(), String> {
    let source = safe_path::safe_join(&plan.content_dir, relative)?;
    let target = guarded_join(&plan.install_dir, relative)?;
    if target.is_dir() {
        return Err(format!(
            "Cannot replace a live folder with a file: {relative}"
        ));
    }
    if !target.exists() {
        created.insert(relative.into());
    }
    fs::create_dir_all(target.parent().unwrap_or(&plan.install_dir))
        .map_err(|error| format!("Could not create destination for {relative}: {error}"))?;
    applied.push(relative.into());
    atomic_copy(&source, &target, &plan.preview_id)
}

fn atomic_copy(source: &Path, target: &Path, transaction_id: &str) -> Result<(), String> {
    let file_name = target
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "Destination file name is not valid Unicode".to_string())?;
    let temporary = target.with_file_name(format!(".{file_name}.{transaction_id}.partial"));
    fs::copy(source, &temporary).map_err(|error| {
        format!(
            "Could not stage live replacement {}: {error}",
            target.display()
        )
    })?;
    OpenOptions::new()
        .write(true)
        .open(&temporary)
        .and_then(|file| file.sync_all())
        .map_err(|error| {
            format!(
                "Could not sync live replacement {}: {error}",
                target.display()
            )
        })?;
    if target.exists() {
        fs::remove_file(target)
            .map_err(|error| format!("Could not replace {}: {error}", target.display()))?;
    }
    fs::rename(&temporary, target)
        .map_err(|error| format!("Could not activate {}: {error}", target.display()))
}

fn remove_obsolete(plan: &TransactionPlan, removed: &mut Vec<String>) -> Result<(), String> {
    for relative in &plan.obsolete_paths {
        let target = guarded_join(&plan.install_dir, relative)?;
        if !target.exists() {
            continue;
        }
        removed.push(relative.clone());
        if target.is_file() {
            fs::remove_file(&target)
                .map_err(|error| format!("Could not remove obsolete {relative}: {error}"))?;
        } else if target.is_dir() {
            fs::remove_dir_all(&target)
                .map_err(|error| format!("Could not remove obsolete {relative}: {error}"))?;
        } else {
            return Err(format!("Unsupported obsolete path type: {relative}"));
        }
    }
    Ok(())
}

fn apply_version_marker(
    plan: &TransactionPlan,
    applied: &mut Vec<String>,
    created: &mut HashSet<String>,
) -> Result<(), String> {
    let relative = "modpack_version.txt";
    let target = guarded_join(&plan.install_dir, relative)?;
    if !target.exists() {
        created.insert(relative.into());
    }
    let temporary_source = plan.stage_root.join("verified-version.txt");
    fs::write(&temporary_source, format!("{}\n", plan.version))
        .map_err(|error| format!("Could not stage installed version marker: {error}"))?;
    applied.push(relative.into());
    atomic_copy(&temporary_source, &target, &plan.preview_id)
}

fn verify_required_install(manifest: &Manifest, install_dir: &Path) -> Result<Vec<String>, String> {
    let mut issues = Vec::new();
    for entry in manifest.files.iter().filter(|entry| entry.required) {
        if !entry_matches(entry, install_dir)? {
            issues.push(entry.path.clone());
        }
    }
    Ok(issues)
}

fn rollback(
    plan: &TransactionPlan,
    backup_path: Option<&Path>,
    created: &HashSet<String>,
) -> Result<(), String> {
    let mut errors = Vec::new();
    let mut created: Vec<_> = created.iter().cloned().collect();
    created.sort_by_key(|path| std::cmp::Reverse(path.matches('/').count()));
    for relative in created {
        match guarded_join(&plan.install_dir, &relative) {
            Ok(target) if target.is_file() => {
                if let Err(error) = fs::remove_file(&target) {
                    errors.push(format!("{relative}: {error}"));
                }
                remove_empty_parents(&plan.install_dir, target.parent());
            }
            Ok(target) if target.is_dir() => {
                if let Err(error) = fs::remove_dir_all(&target) {
                    errors.push(format!("{relative}: {error}"));
                }
            }
            Ok(_) => {}
            Err(error) => errors.push(format!("{relative}: {error}")),
        }
    }
    if let Some(path) = backup_path
        && let Err(error) =
            restore_points::restore_archive_files(path, &plan.install_dir, &plan.preview_id)
    {
        errors.push(error);
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

fn existing_obsolete(manifest: &Manifest, install_dir: &Path) -> Result<Vec<String>, String> {
    let mut existing = Vec::new();
    for relative in &manifest.obsolete_files {
        if guarded_join(install_dir, relative)?.exists() {
            existing.push(relative.clone());
        }
    }
    Ok(existing)
}

fn version_marker_matches(install_dir: &Path, expected: &str) -> bool {
    fs::read_to_string(install_dir.join("modpack_version.txt"))
        .map(|value| value.trim() == expected.trim())
        .unwrap_or(false)
}

fn reject_root_link(root: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(root)
        .map_err(|error| format!("Could not inspect modpack folder: {error}"))?;
    if metadata.file_type().is_symlink() {
        return Err("The configured modpack folder cannot be a symbolic link".into());
    }
    Ok(())
}

fn guarded_join(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let target = safe_path::safe_join(root, relative)?;
    let canonical_root = fs::canonicalize(root)
        .map_err(|error| format!("Could not resolve modpack folder: {error}"))?;
    let mut current = root.to_path_buf();
    let normalized = safe_path::normalize_relative(relative)?;
    for part in normalized.split('/') {
        current.push(part);
        if !current.exists() {
            continue;
        }
        let metadata = fs::symlink_metadata(&current)
            .map_err(|error| format!("Could not inspect {}: {error}", current.display()))?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "Symbolic links are not allowed in affected modpack paths: {}",
                current.display()
            ));
        }
        let resolved = fs::canonicalize(&current)
            .map_err(|error| format!("Could not resolve {}: {error}", current.display()))?;
        if !resolved.starts_with(&canonical_root) {
            return Err(format!(
                "Affected modpack path escapes the configured folder: {relative}"
            ));
        }
    }
    Ok(target)
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

fn staged_bytes(inventory: &[StageEntry]) -> Result<u64, String> {
    inventory
        .iter()
        .try_fold(0_u64, |total, entry| total.checked_add(entry.size))
        .ok_or_else(|| "Staged file sizes overflowed".to_string())
}

fn partial_path(destination: &Path) -> PathBuf {
    destination.with_extension(format!(
        "{}.partial",
        destination
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("download")
    ))
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

fn validate_profile_component(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 100
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte))
    {
        return Err("Profile id cannot be used for isolated update storage".into());
    }
    Ok(())
}

fn transaction_plans() -> &'static Mutex<HashMap<String, TransactionPlan>> {
    TRANSACTION_PLANS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn kind_label(kind: TransactionKind) -> &'static str {
    match kind {
        TransactionKind::Update => "update",
        TransactionKind::Repair => "repair",
    }
}

fn title_kind(kind: TransactionKind) -> &'static str {
    match kind {
        TransactionKind::Update => "Update",
        TransactionKind::Repair => "Repair",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use tempfile::TempDir;

    fn hash(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    fn profile(install: &Path, source: &Path) -> GameProfile {
        let mut profile = crate::models::LauncherConfig::default().profiles.remove(0);
        profile.id = "fixture".into();
        profile.game = "minecraft".into();
        profile.install_dir = install.display().to_string();
        profile.update_source = source.display().to_string();
        profile.local_modpack_version = "1.0.0".into();
        profile
    }

    fn manifest(entries: Vec<(&str, &[u8])>) -> Manifest {
        Manifest {
            manifest_version: "1.0".into(),
            profile_id: "fixture".into(),
            game: "minecraft".into(),
            modpack_version: "2.0.0".into(),
            files: entries
                .into_iter()
                .map(|(path, bytes)| FileEntry {
                    path: path.into(),
                    size: i64::try_from(bytes.len()).unwrap(),
                    hash: hash(bytes),
                    required: true,
                    ..FileEntry::default()
                })
                .collect(),
            ..Manifest::default()
        }
    }

    fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let file = File::create(path).unwrap();
        let mut archive = ZipWriter::new(file);
        for (name, bytes) in entries {
            archive
                .start_file(*name, SimpleFileOptions::default())
                .unwrap();
            archive.write_all(bytes).unwrap();
        }
        archive.finish().unwrap();
    }

    fn multipart_manifest(package: &Path, parts: &[PathBuf]) -> Manifest {
        let mut manifest = manifest(vec![("mods/example.jar", b"multipart")]);
        manifest.update_sha256 = crate::manifest::sha256(package).unwrap();
        manifest.update_parts = parts
            .iter()
            .map(|path| crate::manifest::UpdatePart {
                url: path.display().to_string(),
                sha256: crate::manifest::sha256(path).unwrap(),
                size: i64::try_from(path.metadata().unwrap().len()).unwrap(),
            })
            .collect();
        manifest
    }

    #[test]
    fn multipart_download_reassembles_the_exact_verified_package() {
        let root = TempDir::new().unwrap();
        let install = root.path().join("install");
        fs::create_dir_all(&install).unwrap();
        let package = root.path().join("source.zip");
        write_zip(&package, &[("mods/example.jar", b"multipart")]);
        let bytes = fs::read(&package).unwrap();
        let midpoint = bytes.len() / 2;
        let parts = [root.path().join("part001"), root.path().join("part002")];
        fs::write(&parts[0], &bytes[..midpoint]).unwrap();
        fs::write(&parts[1], &bytes[midpoint..]).unwrap();
        let manifest = multipart_manifest(&package, &parts);
        let stage = root.path().join("stage");
        fs::create_dir_all(&stage).unwrap();

        let source = prepare_package(&profile(&install, &package), &manifest, &stage).unwrap();
        assert_eq!(source, "2 verified release parts");
        assert_eq!(fs::read(stage.join("update.zip")).unwrap(), bytes);
    }

    #[test]
    fn multipart_download_rejects_a_wrong_combined_package_hash() {
        let root = TempDir::new().unwrap();
        let install = root.path().join("install");
        fs::create_dir_all(&install).unwrap();
        let package = root.path().join("source.zip");
        write_zip(&package, &[("mods/example.jar", b"multipart")]);
        let bytes = fs::read(&package).unwrap();
        let midpoint = bytes.len() / 2;
        let parts = [root.path().join("part001"), root.path().join("part002")];
        fs::write(&parts[0], &bytes[..midpoint]).unwrap();
        fs::write(&parts[1], &bytes[midpoint..]).unwrap();
        let mut manifest = multipart_manifest(&package, &parts);
        manifest.update_sha256 = "a".repeat(64);
        let stage = root.path().join("stage");
        fs::create_dir_all(&stage).unwrap();

        let error = prepare_package(&profile(&install, &package), &manifest, &stage).unwrap_err();
        assert!(error.contains("Reassembled update package SHA-256"));
        assert!(!stage.join("update.zip").exists());
    }

    #[test]
    fn update_stages_overlay_then_applies_with_backup() {
        let root = TempDir::new().unwrap();
        let install = root.path().join("install");
        fs::create_dir_all(install.join("mods")).unwrap();
        fs::write(install.join("mods/same.jar"), b"same").unwrap();
        fs::write(install.join("mods/change.jar"), b"old").unwrap();
        let package = root.path().join("update.zip");
        write_zip(
            &package,
            &[("mods/change.jar", b"new"), ("mods/new.jar", b"added")],
        );
        let profile = profile(&install, &package);
        let manifest = manifest(vec![
            ("mods/same.jar", b"same"),
            ("mods/change.jar", b"new"),
            ("mods/new.jar", b"added"),
        ]);
        let data = root.path().join("data");
        let preview =
            prepare_at(&profile, &manifest, TransactionKind::Update, &data, true).unwrap();
        assert!(preview.ready);
        assert_eq!(preview.staged_files, 2);
        assert_eq!(fs::read(install.join("mods/change.jar")).unwrap(), b"old");
        let plan = transaction_plans()
            .lock()
            .unwrap()
            .remove(&preview.preview_id)
            .unwrap();
        let outcome = execute_plan(&plan, None, || Ok(())).unwrap();
        assert!(outcome.success, "{outcome:?}");
        assert!(!outcome.backup_path.is_empty());
        assert_eq!(fs::read(install.join("mods/change.jar")).unwrap(), b"new");
        assert_eq!(fs::read(install.join("mods/new.jar")).unwrap(), b"added");
        assert_eq!(
            fs::read_to_string(install.join("modpack_version.txt"))
                .unwrap()
                .trim(),
            "2.0.0"
        );
    }

    #[test]
    fn unsafe_archive_fails_before_live_mutation() {
        let root = TempDir::new().unwrap();
        let install = root.path().join("install");
        fs::create_dir_all(&install).unwrap();
        fs::write(install.join("sentinel.txt"), b"untouched").unwrap();
        let package = root.path().join("unsafe.zip");
        write_zip(&package, &[("../outside.txt", b"bad")]);
        let profile = profile(&install, &package);
        let manifest = manifest(vec![("mods/a.jar", b"good")]);
        assert!(
            prepare_at(
                &profile,
                &manifest,
                TransactionKind::Update,
                &root.path().join("data"),
                false,
            )
            .is_err()
        );
        assert_eq!(
            fs::read(install.join("sentinel.txt")).unwrap(),
            b"untouched"
        );
        assert!(!root.path().join("outside.txt").exists());
    }

    #[test]
    fn mid_apply_failure_rolls_back_overwrites_and_new_files() {
        let root = TempDir::new().unwrap();
        let install = root.path().join("install");
        fs::create_dir_all(&install).unwrap();
        fs::write(install.join("a.txt"), b"old-a").unwrap();
        let package = root.path().join("update.zip");
        write_zip(&package, &[("a.txt", b"new-a"), ("b.txt", b"new-b")]);
        let profile = profile(&install, &package);
        let manifest = manifest(vec![("a.txt", b"new-a"), ("b.txt", b"new-b")]);
        let preview = prepare_at(
            &profile,
            &manifest,
            TransactionKind::Update,
            &root.path().join("data"),
            true,
        )
        .unwrap();
        let plan = transaction_plans()
            .lock()
            .unwrap()
            .remove(&preview.preview_id)
            .unwrap();
        let outcome = execute_plan(&plan, Some(1), || Ok(())).unwrap();
        assert!(!outcome.success);
        assert!(outcome.rolled_back, "{outcome:?}");
        assert_eq!(fs::read(install.join("a.txt")).unwrap(), b"old-a");
        assert!(!install.join("b.txt").exists());
    }

    #[test]
    fn finalize_failure_restores_obsolete_and_version_marker() {
        let root = TempDir::new().unwrap();
        let install = root.path().join("install");
        fs::create_dir_all(&install).unwrap();
        fs::write(install.join("a.txt"), b"old").unwrap();
        fs::write(install.join("obsolete.txt"), b"restore-me").unwrap();
        fs::write(install.join("modpack_version.txt"), b"1.0.0\n").unwrap();
        let package = root.path().join("update.zip");
        write_zip(&package, &[("a.txt", b"new")]);
        let profile = profile(&install, &package);
        let mut manifest = manifest(vec![("a.txt", b"new")]);
        manifest.obsolete_files.push("obsolete.txt".into());
        let preview = prepare_at(
            &profile,
            &manifest,
            TransactionKind::Update,
            &root.path().join("data"),
            true,
        )
        .unwrap();
        let plan = transaction_plans()
            .lock()
            .unwrap()
            .remove(&preview.preview_id)
            .unwrap();
        let outcome =
            execute_plan(&plan, None, || Err("forced persistence failure".into())).unwrap();
        assert!(!outcome.success);
        assert!(outcome.rolled_back, "{outcome:?}");
        assert_eq!(fs::read(install.join("a.txt")).unwrap(), b"old");
        assert_eq!(
            fs::read(install.join("obsolete.txt")).unwrap(),
            b"restore-me"
        );
        assert_eq!(
            fs::read_to_string(install.join("modpack_version.txt"))
                .unwrap()
                .trim(),
            "1.0.0"
        );
    }

    #[test]
    fn refuses_staging_inside_live_install() {
        let root = TempDir::new().unwrap();
        let install = root.path().join("install");
        fs::create_dir_all(&install).unwrap();
        let package = root.path().join("update.zip");
        write_zip(&package, &[("a.txt", b"new")]);
        let profile = profile(&install, &package);
        let manifest = manifest(vec![("a.txt", b"new")]);
        let error = prepare_at(
            &profile,
            &manifest,
            TransactionKind::Update,
            &install.join("launcher-data"),
            false,
        )
        .unwrap_err();
        assert!(error.contains("outside the live modpack"));
        assert!(!install.join("a.txt").exists());
    }

    #[test]
    fn repair_stages_and_applies_only_mismatched_files() {
        let root = TempDir::new().unwrap();
        let install = root.path().join("install");
        fs::create_dir_all(&install).unwrap();
        fs::write(install.join("good.txt"), b"good").unwrap();
        fs::write(install.join("bad.txt"), b"bad").unwrap();
        let package = root.path().join("update.zip");
        write_zip(&package, &[("good.txt", b"good"), ("bad.txt", b"fixed")]);
        let profile = profile(&install, &package);
        let manifest = manifest(vec![("good.txt", b"good"), ("bad.txt", b"fixed")]);
        let preview = prepare_at(
            &profile,
            &manifest,
            TransactionKind::Repair,
            &root.path().join("data"),
            true,
        )
        .unwrap();
        assert_eq!(preview.staged_files, 1);
        let plan = transaction_plans()
            .lock()
            .unwrap()
            .remove(&preview.preview_id)
            .unwrap();
        let outcome = execute_plan(&plan, None, || Ok(())).unwrap();
        assert!(outcome.success, "{outcome:?}");
        assert_eq!(fs::read(install.join("bad.txt")).unwrap(), b"fixed");
        assert_eq!(fs::read(install.join("good.txt")).unwrap(), b"good");
    }

    #[test]
    fn applying_without_confirmation_is_fail_closed() {
        assert!(require_confirmation(false).is_err());
    }
}
