//! Optional choices stay in local configuration, committed with the verified transaction.
use crate::{
    manifest::{self, Manifest},
    models::{GameProfile, OptionalSelection},
    safe_path, storage,
};
use serde::Serialize;
use std::{collections::HashSet, fs, path::Path};
use tauri::AppHandle;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionalFileStatus {
    path: String,
    category: String,
    bytes: i64,
    enabled: bool,
    installed: bool,
    current: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionalExtrasStatus {
    profile_id: String,
    version: String,
    files: Vec<OptionalFileStatus>,
}

#[tauri::command]
pub async fn get_optional_extras(
    app: AppHandle,
    profile_id: String,
) -> Result<OptionalExtrasStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _operation = crate::operations::MaintenanceGuard::acquire()?;
        ensure_no_safe_session(&app, &profile_id)?;
        let config = storage::load_or_create(&app)?;
        let profile = config
            .profiles
            .iter()
            .find(|p| p.id == profile_id)
            .ok_or("That modpack profile does not exist")?;
        let loaded = manifest::load_for_profile(&app, profile);
        if !loaded.summary.valid {
            return Err(loaded.summary.errors.join("; "));
        }
        let selection = resolve(
            profile,
            &loaded.manifest,
            config.optional_selections.get(&profile_id),
            None,
        )?;
        let enabled: HashSet<_> = selection
            .enabled
            .iter()
            .map(|p| p.to_ascii_lowercase())
            .collect();
        let root = Path::new(&profile.install_dir);
        let mut files = Vec::new();
        for entry in &loaded.manifest.optional_files {
            let path = optional_path(root, &entry.path)?;
            let installed = path.is_file();
            let current = installed
                && path.metadata().map_err(|e| e.to_string())?.len()
                    == u64::try_from(entry.size).unwrap_or(u64::MAX)
                && manifest::sha256(&path)?.eq_ignore_ascii_case(&entry.hash);
            files.push(OptionalFileStatus {
                path: entry.path.clone(),
                category: entry.category.clone(),
                bytes: entry.size,
                enabled: enabled.contains(&entry.path.to_ascii_lowercase()),
                installed,
                current,
            });
        }
        Ok(OptionalExtrasStatus {
            profile_id,
            version: loaded.manifest.modpack_version,
            files,
        })
    })
    .await
    .map_err(|error| format!("Optional file inspection failed: {error}"))?
}

pub fn ensure_no_safe_session(app: &AppHandle, profile_id: &str) -> Result<(), String> {
    if crate::safe_launch::status(app, profile_id)?.active {
        return Err("Finish or recover the recorded Safe Launch session before changing or checking optional files".into());
    }
    Ok(())
}

fn optional_path(root: &Path, relative: &str) -> Result<std::path::PathBuf, String> {
    let path = safe_path::safe_join(root, relative)?;
    safe_path::reject_link_path(&path)?;
    if path.is_dir() {
        return Err(format!(
            "Optional file path is a directory and was left unchanged: {relative}"
        ));
    }
    Ok(path)
}

pub fn resolve(
    profile: &GameProfile,
    manifest: &Manifest,
    saved: Option<&OptionalSelection>,
    requested: Option<&[String]>,
) -> Result<OptionalSelection, String> {
    let root = Path::new(profile.install_dir.trim());
    if !root.is_dir() {
        return Err("Choose an existing modpack folder before managing optional files".into());
    }
    safe_path::reject_link_path(root)?;
    let canonical = fs::canonicalize(root).map_err(|error| {
        format!("Choose an existing modpack folder before managing optional files: {error}")
    })?;
    let same_install = saved.filter(|choice| Path::new(&choice.install_dir) == canonical);
    let available: HashSet<_> = manifest
        .optional_files
        .iter()
        .map(|e| e.path.to_ascii_lowercase())
        .collect();
    let mut enabled = HashSet::new();
    if let Some(requested) = requested {
        for path in requested {
            let normalized = safe_path::normalize_relative(path)?.to_ascii_lowercase();
            if !available.contains(&normalized) {
                return Err(format!(
                    "The trusted manifest does not declare this optional file: {path}"
                ));
            }
            if !enabled.insert(normalized) {
                return Err(format!("Optional file was selected more than once: {path}"));
            }
        }
    } else if let Some(saved) = same_install {
        enabled.extend(
            saved
                .enabled
                .iter()
                .map(|p| p.to_ascii_lowercase())
                .filter(|p| available.contains(p)),
        );
    } else {
        // Existing installations keep their currently present extras; fresh installations opt in.
        for entry in &manifest.optional_files {
            if optional_path(root, &entry.path)?.is_file() {
                enabled.insert(entry.path.to_ascii_lowercase());
            }
        }
    }
    let mut enabled: Vec<_> = manifest
        .optional_files
        .iter()
        .filter(|e| enabled.contains(&e.path.to_ascii_lowercase()))
        .map(|e| e.path.clone())
        .collect();
    enabled.sort();
    Ok(OptionalSelection {
        install_dir: canonical.display().to_string(),
        enabled,
    })
}

pub fn effective_manifest(
    profile: &GameProfile,
    manifest: &Manifest,
    selection: &OptionalSelection,
    explicit_change: bool,
) -> Result<Manifest, String> {
    let enabled: HashSet<_> = selection
        .enabled
        .iter()
        .map(|p| p.to_ascii_lowercase())
        .collect();
    let mut effective = manifest.clone();
    effective.optional_files.clear();
    for entry in &manifest.optional_files {
        if enabled.contains(&entry.path.to_ascii_lowercase()) {
            let mut selected = entry.clone();
            selected.required = true;
            effective.files.push(selected);
        } else if explicit_change {
            optional_path(Path::new(&profile.install_dir), &entry.path)?;
            effective.obsolete_files.push(entry.path.clone());
        }
    }
    Ok(effective)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::FileEntry;
    fn fixture(root: &Path) -> (GameProfile, Manifest) {
        let mut profile = crate::models::LauncherConfig::default().profiles.remove(0);
        profile.install_dir = root.display().to_string();
        let manifest = Manifest {
            optional_files: vec![FileEntry {
                path: "extra.jar".into(),
                ..FileEntry::default()
            }],
            ..Manifest::default()
        };
        (profile, manifest)
    }
    #[test]
    fn fresh_installs_opt_in_existing_files_are_adopted_and_missing_selected_files_stay_selected() {
        let root = tempfile::tempdir().unwrap();
        let (profile, manifest) = fixture(root.path());
        assert!(
            resolve(&profile, &manifest, None, None)
                .unwrap()
                .enabled
                .is_empty()
        );
        fs::write(root.path().join("extra.jar"), b"existing").unwrap();
        let selected = resolve(&profile, &manifest, None, None).unwrap();
        assert_eq!(selected.enabled, ["extra.jar"]);
        fs::remove_file(root.path().join("extra.jar")).unwrap();
        assert_eq!(
            resolve(&profile, &manifest, Some(&selected), None)
                .unwrap()
                .enabled,
            ["extra.jar"]
        );
        let other_root = tempfile::tempdir().unwrap();
        let (other, _) = fixture(other_root.path());
        assert!(
            resolve(&other, &manifest, Some(&selected), None)
                .unwrap()
                .enabled
                .is_empty()
        );
    }
    #[test]
    fn selection_accepts_only_known_optional_files_and_never_deletes_directories() {
        let root = tempfile::tempdir().unwrap();
        let (profile, manifest) = fixture(root.path());
        for paths in [
            vec!["../escape".into()],
            vec!["required.jar".into()],
            vec!["extra.jar".into(), "EXTRA.jar".into()],
        ] {
            assert!(resolve(&profile, &manifest, None, Some(&paths)).is_err());
        }
        fs::create_dir(root.path().join("extra.jar")).unwrap();
        let selection = resolve(&profile, &manifest, None, Some(&[])).unwrap();
        assert!(
            effective_manifest(&profile, &manifest, &selection, true)
                .unwrap_err()
                .contains("directory")
        );
    }
}
