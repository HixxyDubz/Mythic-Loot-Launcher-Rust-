use std::{
    collections::HashSet,
    fs::{self, Metadata},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[cfg(windows)]
use std::os::windows::fs::MetadataExt;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::{models::GameProfile, storage};

const BACKUPS_TO_KEEP: usize = 5;
const TEMPORARY_RETENTION: Duration = Duration::from_secs(24 * 60 * 60);
const MAX_SCANNED_ENTRIES: usize = 1_000_000;
const MAX_REPORTED_ISSUES: usize = 20;

const TEMPORARY_ROOTS: [(&str, &str, &str); 7] = [
    ("update-staging", "Update staging", "update-staging"),
    ("restore-staging", "Restore staging", "restore-staging"),
    (
        "publish-previews",
        "Developer package previews",
        "publish-previews",
    ),
    (
        "catalog-previews",
        "Developer catalogue previews",
        "catalog-previews",
    ),
    (
        "content-release-previews",
        "Developer content previews",
        "content-release-previews",
    ),
    (
        "app-update-staging",
        "Application update staging",
        "app-update-staging",
    ),
    (
        "app-update-release-previews",
        "Developer application release previews",
        "app-update-release-previews",
    ),
];

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum StorageCleanupKind {
    OldBackups,
    MetadataCache,
    TemporaryWork,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageBucket {
    pub key: String,
    pub label: String,
    pub category: String,
    pub path: String,
    pub bytes_used: u64,
    pub file_count: usize,
    pub directory_count: usize,
    pub exists: bool,
    pub truncated: bool,
    pub cleanup_kind: Option<StorageCleanupKind>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageReport {
    pub data_dir: String,
    pub launcher_bytes: u64,
    pub profile_bytes: u64,
    pub measured_at: i64,
    pub temporary_retention_hours: u32,
    pub backups_kept_per_profile: usize,
    pub buckets: Vec<StorageBucket>,
    pub issues: Vec<String>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageCleanupOutcome {
    pub kind: StorageCleanupKind,
    pub deleted_entries: usize,
    pub reclaimed_bytes: u64,
    pub skipped_entries: usize,
    pub complete: bool,
    pub message: String,
    pub report: StorageReport,
}

#[derive(Default)]
struct ScanResult {
    bytes: u64,
    files: usize,
    directories: usize,
    exists: bool,
    truncated: bool,
    issues: Vec<String>,
}

#[derive(Default)]
struct CleanupCount {
    deleted: usize,
    bytes: u64,
    skipped: usize,
}

pub fn report(app: &AppHandle) -> Result<StorageReport, String> {
    let data_dir = storage::data_dir(app)?;
    let config = storage::load_or_create(app)?;
    report_at(&data_dir, &config.profiles)
}

pub fn clean(
    app: &AppHandle,
    kind: StorageCleanupKind,
    confirmed: bool,
) -> Result<StorageCleanupOutcome, String> {
    require_confirmation(confirmed)?;
    let data_dir = storage::data_dir(app)?;
    let config = storage::load_or_create(app)?;
    clean_at(&data_dir, &config.profiles, kind, SystemTime::now())
}

fn require_confirmation(confirmed: bool) -> Result<(), String> {
    if confirmed {
        Ok(())
    } else {
        Err("Storage cleanup requires explicit confirmation".into())
    }
}

fn report_at(data_dir: &Path, profiles: &[GameProfile]) -> Result<StorageReport, String> {
    let launcher_scan = scan_path(data_dir);
    let mut issues = launcher_scan.issues.clone();
    let mut buckets = Vec::new();
    let mut profile_bytes = 0_u64;
    let mut truncated = launcher_scan.truncated;

    for profile in profiles {
        if profile.install_dir.trim().is_empty() {
            continue;
        }
        let path = PathBuf::from(profile.install_dir.trim());
        let scan = scan_path(&path);
        profile_bytes = profile_bytes.saturating_add(scan.bytes);
        truncated |= scan.truncated;
        extend_issues(&mut issues, scan.issues.iter().cloned());
        buckets.push(bucket(
            format!("profile:{}", profile.id),
            profile.display_name.clone(),
            "Modpack",
            &path,
            scan,
            None,
        ));
    }

    let managed = [
        (
            "backups",
            "Restore points / backups",
            "Recovery",
            "backups",
            Some(StorageCleanupKind::OldBackups),
        ),
        (
            "catalog",
            "Verified catalogue cache",
            "Cache",
            "catalog",
            Some(StorageCleanupKind::MetadataCache),
        ),
        (
            "manifests",
            "Trusted manifests",
            "Launcher state",
            "manifests",
            None,
        ),
        (
            "minecraft-bootstrap",
            "Minecraft launcher imports",
            "Generated files",
            "minecraft-bootstrap",
            None,
        ),
        (
            "safe-launch",
            "Safe Launch recovery",
            "Recovery",
            "safe-launch",
            None,
        ),
        (
            "support-bundles",
            "Privacy-redacted support bundles",
            "Generated files",
            "support-bundles",
            None,
        ),
    ];
    for (key, label, category, relative, cleanup_kind) in managed {
        let path = data_dir.join(relative);
        let scan = scan_path(&path);
        truncated |= scan.truncated;
        extend_issues(&mut issues, scan.issues.iter().cloned());
        buckets.push(bucket(
            key.into(),
            label.into(),
            category,
            &path,
            scan,
            cleanup_kind,
        ));
    }

    for (key, label, relative) in TEMPORARY_ROOTS {
        let path = data_dir.join(relative);
        let scan = scan_path(&path);
        truncated |= scan.truncated;
        extend_issues(&mut issues, scan.issues.iter().cloned());
        buckets.push(bucket(
            key.into(),
            label.into(),
            "Temporary work",
            &path,
            scan,
            Some(StorageCleanupKind::TemporaryWork),
        ));
    }

    buckets.sort_by(|left, right| {
        left.category
            .cmp(&right.category)
            .then_with(|| left.label.cmp(&right.label))
    });
    issues.truncate(MAX_REPORTED_ISSUES);
    Ok(StorageReport {
        data_dir: data_dir.display().to_string(),
        launcher_bytes: launcher_scan.bytes,
        profile_bytes,
        measured_at: unix_millis(SystemTime::now()),
        temporary_retention_hours: 24,
        backups_kept_per_profile: BACKUPS_TO_KEEP,
        buckets,
        issues,
        truncated,
    })
}

fn clean_at(
    data_dir: &Path,
    profiles: &[GameProfile],
    kind: StorageCleanupKind,
    now: SystemTime,
) -> Result<StorageCleanupOutcome, String> {
    fs::create_dir_all(data_dir)
        .map_err(|error| format!("Could not create launcher data storage: {error}"))?;
    reject_link_root(data_dir)?;
    let mut count = CleanupCount::default();
    match kind {
        StorageCleanupKind::OldBackups => {
            clean_old_backups(data_dir, profiles, &mut count)?;
        }
        StorageCleanupKind::MetadataCache => {
            clean_directory_contents(&data_dir.join("catalog"), None, now, &mut count)?;
        }
        StorageCleanupKind::TemporaryWork => {
            for (_, _, relative) in TEMPORARY_ROOTS {
                clean_directory_contents(
                    &data_dir.join(relative),
                    Some(TEMPORARY_RETENTION),
                    now,
                    &mut count,
                )?;
            }
        }
    }
    let complete = count.skipped == 0;
    let message = cleanup_message(kind, &count);
    let report = report_at(data_dir, profiles)?;
    Ok(StorageCleanupOutcome {
        kind,
        deleted_entries: count.deleted,
        reclaimed_bytes: count.bytes,
        skipped_entries: count.skipped,
        complete,
        message,
        report,
    })
}

fn clean_old_backups(
    data_dir: &Path,
    profiles: &[GameProfile],
    count: &mut CleanupCount,
) -> Result<(), String> {
    let backups_root = data_dir.join("backups");
    if !backups_root.exists() {
        return Ok(());
    }
    reject_link_root(&backups_root)?;
    let mut seen = HashSet::new();
    for profile in profiles {
        if !safe_component(&profile.id) || !seen.insert(profile.id.as_str()) {
            continue;
        }
        let profile_root = backups_root.join(&profile.id);
        if !profile_root.exists() {
            continue;
        }
        let metadata = fs::symlink_metadata(&profile_root)
            .map_err(|error| format!("Could not inspect backup folder: {error}"))?;
        if is_link_like(&metadata) || !metadata.is_dir() {
            count.skipped += 1;
            continue;
        }
        let mut archives = Vec::new();
        for entry in fs::read_dir(&profile_root)
            .map_err(|error| format!("Could not inspect backup folder: {error}"))?
        {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => {
                    count.skipped += 1;
                    continue;
                }
            };
            let metadata = match fs::symlink_metadata(entry.path()) {
                Ok(metadata) => metadata,
                Err(_) => {
                    count.skipped += 1;
                    continue;
                }
            };
            if is_link_like(&metadata)
                || !metadata.is_file()
                || entry.path().extension().and_then(|value| value.to_str()) != Some("zip")
            {
                continue;
            }
            archives.push((
                metadata.modified().unwrap_or(UNIX_EPOCH),
                entry.file_name(),
                entry.path(),
                metadata.len(),
            ));
        }
        archives.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| right.1.cmp(&left.1)));
        for (_, _, path, bytes) in archives.into_iter().skip(BACKUPS_TO_KEEP) {
            match fs::remove_file(path) {
                Ok(()) => {
                    count.deleted += 1;
                    count.bytes = count.bytes.saturating_add(bytes);
                }
                Err(_) => count.skipped += 1,
            }
        }
    }
    Ok(())
}

fn clean_directory_contents(
    root: &Path,
    older_than: Option<Duration>,
    now: SystemTime,
    count: &mut CleanupCount,
) -> Result<(), String> {
    if !root.exists() {
        return Ok(());
    }
    reject_link_root(root)?;
    for entry in fs::read_dir(root).map_err(|error| {
        format!(
            "Could not inspect cleanup folder {}: {error}",
            root.display()
        )
    })? {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                count.skipped += 1;
                continue;
            }
        };
        let path = entry.path();
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(_) => {
                count.skipped += 1;
                continue;
            }
        };
        if is_link_like(&metadata) {
            count.skipped += 1;
            continue;
        }
        if older_than.is_some_and(|age| {
            metadata
                .modified()
                .ok()
                .and_then(|modified| now.duration_since(modified).ok())
                .is_none_or(|elapsed| elapsed < age)
        }) {
            continue;
        }
        remove_regular_tree(&path, count);
    }
    Ok(())
}

fn remove_regular_tree(path: &Path, count: &mut CleanupCount) {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => {
            count.skipped += 1;
            return;
        }
    };
    if is_link_like(&metadata) {
        count.skipped += 1;
        return;
    }
    if metadata.is_file() {
        match fs::remove_file(path) {
            Ok(()) => {
                count.deleted += 1;
                count.bytes = count.bytes.saturating_add(metadata.len());
            }
            Err(_) => count.skipped += 1,
        }
        return;
    }
    if !metadata.is_dir() {
        count.skipped += 1;
        return;
    }
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(_) => {
            count.skipped += 1;
            return;
        }
    };
    for entry in entries {
        match entry {
            Ok(entry) => remove_regular_tree(&entry.path(), count),
            Err(_) => count.skipped += 1,
        }
    }
    match fs::remove_dir(path) {
        Ok(()) => count.deleted += 1,
        Err(_) => count.skipped += 1,
    }
}

fn scan_path(path: &Path) -> ScanResult {
    if !path.exists() {
        return ScanResult::default();
    }
    let root_metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) => {
            return ScanResult {
                exists: true,
                issues: vec![format!("Could not inspect {}: {error}", path.display())],
                ..ScanResult::default()
            };
        }
    };
    if is_link_like(&root_metadata) {
        return ScanResult {
            exists: true,
            issues: vec![format!(
                "Skipped linked or redirected storage root {}",
                path.display()
            )],
            ..ScanResult::default()
        };
    }
    if root_metadata.is_file() {
        return ScanResult {
            bytes: root_metadata.len(),
            files: 1,
            exists: true,
            ..ScanResult::default()
        };
    }
    let mut result = ScanResult {
        exists: true,
        ..ScanResult::default()
    };
    let mut pending = vec![path.to_path_buf()];
    let mut inspected = 0_usize;
    while let Some(directory) = pending.pop() {
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) => {
                push_issue(
                    &mut result.issues,
                    format!("Could not inspect {}: {error}", directory.display()),
                );
                continue;
            }
        };
        for entry in entries {
            if inspected >= MAX_SCANNED_ENTRIES {
                result.truncated = true;
                push_issue(
                    &mut result.issues,
                    format!("Storage scan stopped after {MAX_SCANNED_ENTRIES} entries"),
                );
                pending.clear();
                break;
            }
            inspected += 1;
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    push_issue(&mut result.issues, format!("Storage scan warning: {error}"));
                    continue;
                }
            };
            let entry_path = entry.path();
            let metadata = match fs::symlink_metadata(&entry_path) {
                Ok(metadata) => metadata,
                Err(error) => {
                    push_issue(
                        &mut result.issues,
                        format!("Could not inspect {}: {error}", entry_path.display()),
                    );
                    continue;
                }
            };
            if is_link_like(&metadata) {
                push_issue(
                    &mut result.issues,
                    format!("Skipped linked item {}", entry_path.display()),
                );
            } else if metadata.is_file() {
                result.files += 1;
                result.bytes = result.bytes.saturating_add(metadata.len());
            } else if metadata.is_dir() {
                result.directories += 1;
                pending.push(entry_path);
            }
        }
    }
    result
}

fn bucket(
    key: String,
    label: String,
    category: &str,
    path: &Path,
    scan: ScanResult,
    cleanup_kind: Option<StorageCleanupKind>,
) -> StorageBucket {
    StorageBucket {
        key,
        label,
        category: category.into(),
        path: path.display().to_string(),
        bytes_used: scan.bytes,
        file_count: scan.files,
        directory_count: scan.directories,
        exists: scan.exists,
        truncated: scan.truncated,
        cleanup_kind,
    }
}

fn cleanup_message(kind: StorageCleanupKind, count: &CleanupCount) -> String {
    let label = match kind {
        StorageCleanupKind::OldBackups => "Old restore points",
        StorageCleanupKind::MetadataCache => "Verified catalogue cache",
        StorageCleanupKind::TemporaryWork => "Temporary work older than 24 hours",
    };
    if count.skipped == 0 {
        format!(
            "{label}: removed {} entr{} and reclaimed {} bytes.",
            count.deleted,
            if count.deleted == 1 { "y" } else { "ies" },
            count.bytes
        )
    } else {
        format!(
            "{label}: removed {} entr{}, reclaimed {} bytes and safely skipped {} entr{}.",
            count.deleted,
            if count.deleted == 1 { "y" } else { "ies" },
            count.bytes,
            count.skipped,
            if count.skipped == 1 { "y" } else { "ies" }
        )
    }
}

fn reject_link_root(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("Could not inspect cleanup root {}: {error}", path.display()))?;
    if is_link_like(&metadata) || !metadata.is_dir() {
        return Err(format!(
            "Refusing to clean linked, redirected or non-directory storage root {}",
            path.display()
        ));
    }
    Ok(())
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

fn safe_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn push_issue(issues: &mut Vec<String>, issue: String) {
    if issues.len() < MAX_REPORTED_ISSUES {
        issues.push(issue);
    }
}

fn extend_issues(issues: &mut Vec<String>, additions: impl Iterator<Item = String>) {
    for issue in additions {
        push_issue(issues, issue);
    }
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

    fn profiles_with_install(path: &Path) -> Vec<GameProfile> {
        let mut profiles = crate::models::LauncherConfig::default().profiles;
        profiles[0].install_dir = path.display().to_string();
        profiles
    }

    #[test]
    fn report_measures_launcher_and_configured_modpack_storage() {
        let root = tempfile::tempdir().unwrap();
        let install = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("catalog")).unwrap();
        fs::write(root.path().join("catalog/catalog.json"), vec![1_u8; 20]).unwrap();
        fs::write(install.path().join("mod.jar"), vec![2_u8; 30]).unwrap();

        let report = report_at(root.path(), &profiles_with_install(install.path())).unwrap();
        assert!(report.launcher_bytes >= 20);
        assert_eq!(report.profile_bytes, 30);
        assert!(report.buckets.iter().any(|bucket| {
            bucket.key == "catalog"
                && bucket.bytes_used == 20
                && bucket.cleanup_kind == Some(StorageCleanupKind::MetadataCache)
        }));
        assert!(
            report
                .buckets
                .iter()
                .any(|bucket| bucket.key == "profile:minecraft_main" && bucket.bytes_used == 30)
        );
    }

    #[test]
    fn cleanup_is_fail_closed_and_uses_only_fixed_launcher_targets() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("catalog/nested")).unwrap();
        fs::write(root.path().join("catalog/nested/cache.json"), b"cache").unwrap();
        fs::write(outside.path().join("keep.txt"), b"keep").unwrap();
        let profiles = crate::models::LauncherConfig::default().profiles;

        assert!(require_confirmation(false).is_err());
        let outcome = clean_at(
            root.path(),
            &profiles,
            StorageCleanupKind::MetadataCache,
            SystemTime::now(),
        )
        .unwrap();
        assert!(!root.path().join("catalog/nested/cache.json").exists());
        assert_eq!(fs::read(outside.path().join("keep.txt")).unwrap(), b"keep");
        assert!(outcome.reclaimed_bytes >= 5);
    }

    #[test]
    fn temporary_cleanup_preserves_recent_work_and_removes_old_work() {
        let root = tempfile::tempdir().unwrap();
        let recent = root.path().join("update-staging/recent");
        fs::create_dir_all(&recent).unwrap();
        fs::write(recent.join("recent.bin"), b"recent").unwrap();
        let now = SystemTime::now();
        let first = clean_at(
            root.path(),
            &crate::models::LauncherConfig::default().profiles,
            StorageCleanupKind::TemporaryWork,
            now,
        )
        .unwrap();
        assert!(recent.exists());
        assert_eq!(first.reclaimed_bytes, 0);

        let second = clean_at(
            root.path(),
            &crate::models::LauncherConfig::default().profiles,
            StorageCleanupKind::TemporaryWork,
            now + Duration::from_secs(25 * 60 * 60),
        )
        .unwrap();
        assert!(!recent.exists());
        assert!(second.reclaimed_bytes >= 6);
    }

    #[test]
    fn old_backup_cleanup_keeps_the_newest_five_for_each_known_profile() {
        let root = tempfile::tempdir().unwrap();
        let profile_root = root.path().join("backups/minecraft_main");
        fs::create_dir_all(&profile_root).unwrap();
        for index in 0..7 {
            fs::write(profile_root.join(format!("backup-{index}.zip")), [index]).unwrap();
            std::thread::sleep(Duration::from_millis(2));
        }
        let outcome = clean_at(
            root.path(),
            &crate::models::LauncherConfig::default().profiles,
            StorageCleanupKind::OldBackups,
            SystemTime::now(),
        )
        .unwrap();
        let remaining = fs::read_dir(profile_root).unwrap().count();
        assert_eq!(remaining, BACKUPS_TO_KEEP);
        assert_eq!(outcome.reclaimed_bytes, 2);
    }
}
