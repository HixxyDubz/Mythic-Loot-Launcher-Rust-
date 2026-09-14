use std::{
    collections::HashSet,
    env,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use tauri::{AppHandle, Manager};

use crate::models::LauncherConfig;

const CONFIG_FILE: &str = "launcher-config.json";

// Separate OS file handles serialize readers and the entire read/modify/write
// transaction, including other processes using this edition's data directory.
fn config_lock(directory: &Path) -> Result<File, String> {
    crate::safe_path::reject_link_path(directory)?;
    fs::create_dir_all(directory)
        .map_err(|e| format!("Could not create settings directory: {e}"))?;
    let path = directory.join("launcher-config.lock");
    crate::safe_path::reject_link_path(&path)?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|e| format!("Could not open settings lock: {e}"))?;
    fs2::FileExt::lock_exclusive(&file).map_err(|e| format!("Could not lock settings: {e}"))?;
    Ok(file)
}

pub fn update<T>(
    app: &AppHandle,
    change: impl FnOnce(&mut LauncherConfig) -> Result<T, String>,
) -> Result<T, String> {
    update_at(&data_dir(app)?, change)
}

pub fn update_at<T>(
    directory: &Path,
    change: impl FnOnce(&mut LauncherConfig) -> Result<T, String>,
) -> Result<T, String> {
    let _lock = config_lock(directory)?;
    let mut config = load_locked(directory)?;
    let before = config.clone();
    let result = change(&mut config)?;
    if config != before {
        save_locked(directory, &config)?;
    }
    Ok(result)
}

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

pub fn load_or_create_at(directory: &Path) -> Result<LauncherConfig, String> {
    let _lock = config_lock(directory)?;
    load_locked(directory)
}

fn load_locked(directory: &Path) -> Result<LauncherConfig, String> {
    let path = directory.join(CONFIG_FILE);
    if path.exists() {
        match read_config(&path) {
            Ok((config, migrated)) => {
                if migrated {
                    save_locked(directory, &config)?;
                }
                return Ok(config);
            }
            // A future schema must not be silently replaced with an old backup.
            Err(error) if !recoverable_config_error(&error) => {
                return Err(error);
            }
            Err(_) => preserve_invalid(&path)?,
        }
    }
    // The backup is the last committed state. A temporary file is only used
    // when no valid committed copy remains (e.g. first-ever save interrupted).
    for recovery in [
        directory.join(format!("{CONFIG_FILE}.bak")),
        directory.join(format!("{CONFIG_FILE}.tmp")),
    ] {
        if !recovery.exists() {
            continue;
        }
        match read_config(&recovery) {
            Ok((config, _)) => {
                // Do not rotate/delete the recovery source while recovering it.
                let recovered = directory.join("launcher-config.recovering");
                crate::safe_path::reject_link_path(&recovered)?;
                let bytes = serde_json::to_vec_pretty(&config).map_err(|e| e.to_string())?;
                write_synced(&recovered, &bytes)
                    .map_err(|e| format!("Could not stage recovered settings: {e}"))?;
                fs::rename(&recovered, &path)
                    .map_err(|e| format!("Could not activate recovered settings: {e}"))?;
                return Ok(config);
            }
            Err(error) if !recoverable_config_error(&error) => {
                return Err(error);
            }
            Err(_) => preserve_invalid(&recovery)?,
        }
    }
    let config = LauncherConfig::default();
    save_locked(directory, &config)?;
    Ok(config)
}

fn read_config(path: &Path) -> Result<(LauncherConfig, bool), String> {
    crate::safe_path::reject_link_path(path)?;
    let bytes = fs::read(path).map_err(|e| format!("Could not read settings: {e}"))?;
    let mut config: LauncherConfig =
        serde_json::from_slice(&bytes).map_err(|e| format!("Invalid settings JSON: {e}"))?;
    let migrated = migrate(&mut config)?;
    validate(&config).map_err(|e| format!("Invalid settings content: {e}"))?;
    Ok((config, migrated))
}

fn recoverable_config_error(error: &str) -> bool {
    error.starts_with("Invalid settings JSON:") || error.starts_with("Invalid settings content:")
}

fn preserve_invalid(path: &Path) -> Result<(), String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let preserved = path.with_file_name(format!("launcher-config.corrupt-{stamp}-{name}"));
    fs::rename(path, preserved).map_err(|e| format!("Could not preserve invalid settings: {e}"))
}

#[cfg(test)]
pub fn save_at(directory: &Path, config: &LauncherConfig) -> Result<(), String> {
    let _lock = config_lock(directory)?;
    save_locked(directory, config)
}

fn save_locked(directory: &Path, config: &LauncherConfig) -> Result<(), String> {
    validate(config)?;
    fs::create_dir_all(directory)
        .map_err(|error| format!("Could not create {}: {error}", directory.display()))?;
    let destination = directory.join(CONFIG_FILE);
    let temporary = directory.join(format!("{CONFIG_FILE}.tmp"));
    let backup = directory.join(format!("{CONFIG_FILE}.bak"));
    for path in [&destination, &temporary, &backup] {
        crate::safe_path::reject_link_path(path)?;
    }
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
    if config.schema_version != 2 {
        return Err(format!(
            "Unsupported launcher configuration schema {}",
            config.schema_version
        ));
    }
    if config.profiles.is_empty() {
        return Err("At least one modpack profile is required".into());
    }
    let mut ids = HashSet::new();
    for profile in &config.profiles {
        if profile.id.is_empty()
            || profile.id.len() > 64
            || !profile
                .id
                .bytes()
                .next()
                .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
            || !profile.id.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
            })
        {
            return Err(format!("Invalid modpack profile id: {}", profile.id));
        }
        if !ids.insert(profile.id.as_str()) {
            return Err(format!("Duplicate modpack profile id: {}", profile.id));
        }
    }
    if !ids.contains(config.selected_profile_id.as_str()) {
        return Err("The selected modpack profile does not exist".into());
    }
    Ok(())
}

fn migrate(config: &mut LauncherConfig) -> Result<bool, String> {
    match config.schema_version {
        2 => Ok(false),
        1 => {
            config.schema_version = 2;
            for profile in &mut config.profiles {
                if profile.display_name == "Minecraft - Mythic Loot Server" {
                    profile.display_name = "Mythic Loot Minecraft".into();
                } else if profile.display_name == "7 Days To Die - Mythic Loot Server" {
                    profile.display_name = "Mythic Loot 7 Days".into();
                }
            }
            Ok(true)
        }
        version => Err(format!(
            "Unsupported launcher configuration schema {version}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interrupted_rotation_recovers_last_committed_settings_without_losing_backup() {
        let root = tempfile::tempdir().unwrap();
        let mut committed = LauncherConfig::default();
        committed.profiles[0].install_dir = "C:\\Real\\Mods".into();
        committed.profiles[0].local_modpack_version = "9.8.7".into();
        save_at(root.path(), &committed).unwrap();
        let active = root.path().join(CONFIG_FILE);
        let backup = root.path().join(format!("{CONFIG_FILE}.bak"));
        fs::rename(&active, &backup).unwrap();
        let staged = root.path().join(format!("{CONFIG_FILE}.tmp"));
        fs::write(
            &staged,
            serde_json::to_vec(&LauncherConfig::default()).unwrap(),
        )
        .unwrap();
        assert_eq!(load_or_create_at(root.path()).unwrap(), committed);
        assert_eq!(read_config(&backup).unwrap().0, committed);
        assert_eq!(read_config(&active).unwrap().0, committed);
    }

    #[test]
    fn first_save_interruption_uses_valid_staged_settings() {
        let root = tempfile::tempdir().unwrap();
        let config = LauncherConfig {
            selected_profile_id: "seven_days_main".into(),
            ..LauncherConfig::default()
        };
        fs::write(
            root.path().join(format!("{CONFIG_FILE}.tmp")),
            serde_json::to_vec(&config).unwrap(),
        )
        .unwrap();
        assert_eq!(load_or_create_at(root.path()).unwrap(), config);
    }

    #[test]
    fn invalid_active_recovers_backup_but_future_schema_is_never_downgraded() {
        let root = tempfile::tempdir().unwrap();
        let config = LauncherConfig {
            selected_profile_id: "seven_days_main".into(),
            ..LauncherConfig::default()
        };
        let backup = root.path().join(format!("{CONFIG_FILE}.bak"));
        let active = root.path().join(CONFIG_FILE);
        fs::write(&backup, serde_json::to_vec(&config).unwrap()).unwrap();
        fs::write(&active, b"{broken").unwrap();
        assert_eq!(load_or_create_at(root.path()).unwrap(), config);
        let future = LauncherConfig {
            schema_version: 99,
            ..config.clone()
        };
        fs::write(&active, serde_json::to_vec(&future).unwrap()).unwrap();
        assert!(
            load_or_create_at(root.path())
                .unwrap_err()
                .contains("Unsupported")
        );
        assert_eq!(read_config(&backup).unwrap().0, config);
        assert!(fs::read_to_string(&active).unwrap().contains("99"));
    }

    #[test]
    fn concurrent_updates_do_not_lose_other_fields() {
        let root = tempfile::tempdir().unwrap();
        load_or_create_at(root.path()).unwrap();
        std::thread::scope(|scope| {
            for index in 0..2 {
                let directory = root.path();
                scope.spawn(move || {
                    for value in 0..20 {
                        update_at(directory, |config| {
                            config.profiles[index].local_modpack_version = value.to_string();
                            std::thread::sleep(std::time::Duration::from_millis(1));
                            Ok(())
                        })
                        .unwrap();
                    }
                });
            }
        });
        assert!(
            load_or_create_at(root.path())
                .unwrap()
                .profiles
                .iter()
                .all(|p| p.local_modpack_version == "19")
        );
    }

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
        assert_eq!(loaded.schema_version, 2);
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

    #[test]
    fn upgrades_schema_one_without_preserving_server_concepts() {
        let root = tempfile::tempdir().expect("temporary directory");
        let mut legacy = LauncherConfig {
            schema_version: 1,
            ..LauncherConfig::default()
        };
        legacy.profiles[0].display_name = "Minecraft - Mythic Loot Server".into();
        let mut legacy_json = serde_json::to_value(&legacy).unwrap();
        let first_profile = legacy_json["profiles"][0].as_object_mut().unwrap();
        first_profile.insert("serverName".into(), "Legacy server".into());
        first_profile.insert("serverIp".into(), "203.0.113.10".into());
        first_profile.insert("serverPort".into(), 25565.into());
        let bytes = serde_json::to_vec_pretty(&legacy_json).unwrap();
        fs::create_dir_all(root.path()).unwrap();
        fs::write(root.path().join(CONFIG_FILE), bytes).unwrap();
        let migrated = load_or_create_at(root.path()).unwrap();
        assert_eq!(migrated.schema_version, 2);
        assert_eq!(migrated.profiles[0].display_name, "Mythic Loot Minecraft");
        let rewritten = fs::read_to_string(root.path().join(CONFIG_FILE)).unwrap();
        assert!(!rewritten.contains("serverName"));
        assert!(!rewritten.contains("serverIp"));
        assert!(!rewritten.contains("serverPort"));
    }

    #[test]
    fn existing_schema_two_profiles_gain_safe_defaults_for_new_local_fields() {
        let root = tempfile::tempdir().expect("temporary directory");
        let config = LauncherConfig::default();
        let mut json = serde_json::to_value(&config).unwrap();
        json.as_object_mut().unwrap().remove("optionalSelections");
        for profile in json["profiles"].as_array_mut().unwrap() {
            profile.as_object_mut().unwrap().remove("minecraftLauncher");
            profile.as_object_mut().unwrap().remove("catalogVisible");
        }
        fs::write(
            root.path().join(CONFIG_FILE),
            serde_json::to_vec_pretty(&json).unwrap(),
        )
        .unwrap();

        let loaded = load_or_create_at(root.path()).expect("load older schema two config");
        assert!(loaded.optional_selections.is_empty());
        assert!(
            loaded
                .profiles
                .iter()
                .all(|profile| profile.minecraft_launcher.is_empty())
        );
        assert!(
            loaded
                .profiles
                .iter()
                .all(|profile| profile.catalog_visible)
        );
    }
}
