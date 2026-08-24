use std::{
    collections::HashSet,
    env,
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use tauri::{AppHandle, Manager};

use crate::models::LauncherConfig;

const CONFIG_FILE: &str = "launcher-config.json";

pub fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    if let Some(override_dir) = env::var_os("MYTHIC_LOOT_DATA_DIR") {
        let path = PathBuf::from(override_dir);
        if path.as_os_str().is_empty() {
            return Err("MYTHIC_LOOT_DATA_DIR is empty".into());
        }
        return Ok(path);
    }
    app.path()
        .app_data_dir()
        .map_err(|error| format!("Could not resolve the launcher data directory: {error}"))
}

pub fn load_or_create(app: &AppHandle) -> Result<LauncherConfig, String> {
    let directory = data_dir(app)?;
    load_or_create_at(&directory)
}

pub fn save(app: &AppHandle, config: &LauncherConfig) -> Result<(), String> {
    let directory = data_dir(app)?;
    save_at(&directory, config)
}

pub fn load_or_create_at(directory: &Path) -> Result<LauncherConfig, String> {
    fs::create_dir_all(directory)
        .map_err(|error| format!("Could not create {}: {error}", directory.display()))?;
    let path = directory.join(CONFIG_FILE);
    if !path.exists() {
        let config = LauncherConfig::default();
        save_at(directory, &config)?;
        return Ok(config);
    }

    let bytes =
        fs::read(&path).map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    match serde_json::from_slice::<LauncherConfig>(&bytes) {
        Ok(config) => {
            validate(&config)?;
            Ok(config)
        }
        Err(error) => {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let corrupt = directory.join(format!("launcher-config.corrupt-{stamp}.json"));
            fs::rename(&path, &corrupt).map_err(|move_error| {
                format!(
                    "Configuration is invalid ({error}) and could not be preserved as {}: {move_error}",
                    corrupt.display()
                )
            })?;
            let config = LauncherConfig::default();
            save_at(directory, &config)?;
            Ok(config)
        }
    }
}

pub fn save_at(directory: &Path, config: &LauncherConfig) -> Result<(), String> {
    validate(config)?;
    fs::create_dir_all(directory)
        .map_err(|error| format!("Could not create {}: {error}", directory.display()))?;
    let destination = directory.join(CONFIG_FILE);
    let temporary = directory.join(format!("{CONFIG_FILE}.tmp"));
    let backup = directory.join(format!("{CONFIG_FILE}.bak"));
    let bytes = serde_json::to_vec_pretty(config)
        .map_err(|error| format!("Could not encode launcher settings: {error}"))?;

    write_synced(&temporary, &bytes)
        .map_err(|error| format!("Could not stage {}: {error}", temporary.display()))?;

    if backup.exists() {
        fs::remove_file(&backup)
            .map_err(|error| format!("Could not rotate {}: {error}", backup.display()))?;
    }
    if destination.exists() {
        fs::rename(&destination, &backup).map_err(|error| {
            format!(
                "Could not preserve the previous configuration as {}: {error}",
                backup.display()
            )
        })?;
    }
    if let Err(error) = fs::rename(&temporary, &destination) {
        if backup.exists() {
            let _ = fs::rename(&backup, &destination);
        }
        return Err(format!(
            "Could not activate {}: {error}",
            destination.display()
        ));
    }
    Ok(())
}

fn write_synced(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn validate(config: &LauncherConfig) -> Result<(), String> {
    if config.schema_version != 1 {
        return Err(format!(
            "Unsupported launcher configuration schema {}",
            config.schema_version
        ));
    }
    if config.profiles.is_empty() {
        return Err("At least one server profile is required".into());
    }
    let mut ids = HashSet::new();
    for profile in &config.profiles {
        if profile.id.trim().is_empty() {
            return Err("A server profile has an empty id".into());
        }
        if !ids.insert(profile.id.as_str()) {
            return Err(format!("Duplicate server profile id: {}", profile.id));
        }
    }
    if !ids.contains(config.selected_profile_id.as_str()) {
        return Err("The selected server profile does not exist".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_and_round_trips_defaults() {
        let root = tempfile::tempdir().expect("temporary directory");
        let first = load_or_create_at(root.path()).expect("create defaults");
        assert_eq!(first.profiles.len(), 2);
        let mut changed = first;
        changed.selected_profile_id = "seven_days_main".into();
        save_at(root.path(), &changed).expect("save config");
        let loaded = load_or_create_at(root.path()).expect("reload config");
        assert_eq!(loaded.selected_profile_id, "seven_days_main");
    }

    #[test]
    fn preserves_invalid_json_before_recovery() {
        let root = tempfile::tempdir().expect("temporary directory");
        fs::write(root.path().join(CONFIG_FILE), b"{not-json").expect("fixture");
        let loaded = load_or_create_at(root.path()).expect("recover defaults");
        assert_eq!(loaded.schema_version, 1);
        let preserved = fs::read_dir(root.path())
            .expect("directory")
            .filter_map(Result::ok)
            .any(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("launcher-config.corrupt-")
            });
        assert!(preserved);
    }
}
