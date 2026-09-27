//! Read-only target discovery. Unknown configuration must never fall back to the game root.
use std::{
    collections::HashMap,
    fs,
    path::{Component, Path, PathBuf},
};

use crate::{game_installation::read_text, models::DetectedInstall, safe_path};

pub fn factorio_mods(
    root: &Path,
    exe: Option<&Path>,
    appdata: Option<&Path>,
    args: &str,
) -> Result<PathBuf, String> {
    let exe = exe.ok_or("No recognised Factorio executable was found")?;
    if !safe_file(exe)
        || !exe
            .file_name()
            .is_some_and(|n| n.eq_ignore_ascii_case("factorio.exe"))
    {
        return Err("The Factorio executable is missing, unrecognised or redirected".into());
    }
    let overrides = factorio_overrides(args)?;
    let executable_dir = exe.parent().ok_or("The executable has no parent folder")?;
    let system = appdata.map(|p| p.join("Factorio"));
    if let Some(value) = overrides.get("mods") {
        // Command-line values are literal paths, not config-file macro expressions.
        return checked_directory(literal_target(value)?);
    }
    let config = if let Some(value) = overrides.get("config") {
        literal_target(value)?
    } else {
        let values = config_values(
            &read_text(&root.join("config-path.cfg"))?,
            "",
            &["config-path"],
        )?;
        let directory = values
            .get("config-path")
            .ok_or("config-path.cfg does not declare its configuration location")?;
        resolve_config_path(directory, executable_dir, system.as_deref())?.join("config.ini")
    };
    let paths = config_values(&read_text(&config)?, "path", &["write-data"])?;
    // Bootstrap defaults describe config generation, not authoritative active paths.
    let value = paths
        .get("write-data")
        .ok_or("The active configuration does not declare write-data")?;
    let data = resolve_config_path(value, executable_dir, system.as_deref())?;
    checked_directory(checked_path(data.join("mods"))?)
}

fn factorio_overrides(args: &str) -> Result<HashMap<&'static str, String>, String> {
    let args = crate::launch::split_windows_args(args)?;
    let mut values = HashMap::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let (key, inline) = arg
            .split_once('=')
            .map_or((arg.as_str(), None), |(k, v)| (k, Some(v)));
        let name = match key {
            "--mod-directory" => "mods",
            "--config" | "-c" => "config",
            "--executable-path" => {
                return Err("Executable-path overrides require manual target selection".into());
            }
            _ if key.starts_with("-c") && key.len() > 2 => {
                return Err("Use a separate absolute path after -c".into());
            }
            _ => continue,
        };
        let value = inline
            .or_else(|| iter.next().map(String::as_str))
            .ok_or("A Factorio path override is missing its value")?;
        if value.is_empty() || values.insert(name, value.to_string()).is_some() {
            return Err("Empty or duplicate Factorio path override".into());
        }
    }
    Ok(values)
}

fn config_values(
    text: &str,
    section: &str,
    keys: &[&str],
) -> Result<HashMap<String, String>, String> {
    let mut current = "";
    let mut result = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(['#', ';']) {
            continue;
        }
        if line.starts_with('[') {
            current = line
                .strip_prefix('[')
                .and_then(|s| s.strip_suffix(']'))
                .ok_or("Malformed configuration section")?
                .trim();
            continue;
        }
        if current != section {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            if keys.iter().any(|key| line.starts_with(key)) {
                return Err("Malformed path configuration".into());
            }
            continue;
        };
        let key = key.trim();
        if keys.contains(&key) {
            let value = value.trim();
            if value.is_empty() || result.insert(key.to_string(), value.to_string()).is_some() {
                return Err("Empty or duplicate path configuration".into());
            }
        }
    }
    Ok(result)
}

fn literal_target(value: &str) -> Result<PathBuf, String> {
    if value.contains("__PATH__") {
        return Err("Unsupported macro in command-line path".into());
    }
    checked_path(PathBuf::from(value))
}

fn resolve_config_path(
    value: &str,
    exe_dir: &Path,
    system: Option<&Path>,
) -> Result<PathBuf, String> {
    if value.contains([';', '#']) {
        return Err("Paths with ambiguous inline comment markers require manual selection".into());
    }
    let mut value = value;
    if value.starts_with('"') {
        value = value
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .ok_or("Unclosed path quotes")?;
    }
    let path = if let Some(suffix) = value.strip_prefix("__PATH__executable__") {
        append_macro(exe_dir, suffix)?
    } else if let Some(suffix) = value.strip_prefix("__PATH__system-write-data__") {
        append_macro(system.ok_or("APPDATA is unavailable")?, suffix)?
    } else {
        if value.contains("__PATH__") {
            return Err("Unsupported Factorio path macro".into());
        }
        PathBuf::from(value)
    };
    checked_path(path)
}

fn append_macro(base: &Path, suffix: &str) -> Result<PathBuf, String> {
    if suffix.is_empty() {
        return Ok(base.to_path_buf());
    }
    let suffix = suffix
        .strip_prefix(['/', '\\'])
        .ok_or("Invalid macro suffix")?;
    if suffix.starts_with(['/', '\\']) {
        return Err("Invalid rooted macro suffix".into());
    }
    Ok(base.join(suffix.replace('\\', "/")))
}

fn checked_path(path: PathBuf) -> Result<PathBuf, String> {
    let text = path.to_string_lossy();
    if !path.is_absolute()
        || text.starts_with("\\\\")
        || text.starts_with("//")
        || text.chars().any(char::is_control)
    {
        return Err("Only absolute local paths are supported for automatic discovery".into());
    }
    safe_path::reject_link_path(&path)?;
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err("Path escapes its volume".into());
                }
            }
            Component::CurDir => (),
            Component::Normal(part) => {
                safe_path::normalize_relative(&part.to_string_lossy())?;
                normalized.push(part);
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    if normalized.parent().is_none() {
        return Err("A volume root cannot be a modpack target".into());
    }
    safe_path::reject_link_path(&normalized)?;
    Ok(normalized)
}

fn checked_directory(path: PathBuf) -> Result<PathBuf, String> {
    if path.exists() && !path.is_dir() {
        return Err("The target is not a folder".into());
    }
    Ok(path)
}

fn safe_file(path: &Path) -> bool {
    path.is_file() && safe_path::reject_link_path(path).is_ok()
}
fn safe_dir(path: &Path) -> bool {
    path.is_dir() && safe_path::reject_link_path(path).is_ok()
}

pub fn detect_hytale(appdata: Option<&Path>, local: Option<&Path>) -> Vec<DetectedInstall> {
    let Some(appdata) = appdata else {
        return vec![];
    };
    let root = appdata.join("Hytale");
    if !safe_dir(&root) {
        return vec![];
    }
    let exe = [
        Some(root.join("hytale-launcher.exe")),
        Some(root.join("HytaleLauncher.exe")),
        local.map(|p| p.join("Programs/Hytale Launcher/Hytale Launcher.exe")),
    ]
    .into_iter()
    .flatten()
    .find(|p| safe_file(p));
    let mut targets = vec![("release".to_string(), root.join("UserData"))];
    let branches = root.join("data");
    if safe_dir(&branches)
        && let Ok(entries) = fs::read_dir(&branches)
    {
        for entry in entries.take(32).flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name != "release"
                && safe_path::normalize_relative(&name).is_ok()
                && safe_dir(&entry.path())
            {
                targets.push((name, entry.path()));
            }
        }
    }
    targets.sort_by(|a, b| a.0.cmp(&b.0));
    targets.into_iter().filter_map(|(branch, data)| {
        if !safe_dir(&data) { return None; }
        let mods = checked_directory(checked_path(data.join("Mods")).ok()?).ok()?;
        let game = root.join("install").join(&branch).join("package/game/latest");
        // Never pair one patchline's data with another's installation.
        if !safe_dir(&game) { return None; }
        Some(DetectedInstall {
            label: format!("Hytale · {branch}"),
            exe_path: exe.as_ref().map(|p| p.display().to_string()),
            install_dir: game.display().to_string(), source: "hytale-launcher".into(),
            modpack_dir: Some(mods.display().to_string()),
            target_note: format!("Existing {branch} data folder. Select the same patchline in Hytale Launcher; Mythic Loot does not switch patchlines. Custom installations remain manual."),
        })
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf) {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("Factorio");
        let exe = root.join("bin/x64/factorio.exe");
        fs::create_dir_all(exe.parent().unwrap()).unwrap();
        fs::write(&exe, b"test executable; never run").unwrap();
        let appdata = temp.path().join("roaming");
        fs::create_dir_all(appdata.join("Factorio/config")).unwrap();
        (temp, root, exe, appdata)
    }

    #[test]
    fn system_config_resolves_mods_without_creating_them() {
        let (_temp, root, exe, appdata) = fixture();
        fs::write(root.join("config-path.cfg"), "config-path=__PATH__system-write-data__/config\nuse-system-read-write-data-directories=true").unwrap();
        let config = appdata.join("Factorio/config/config.ini");
        let bytes = "[path]\nwrite-data=__PATH__system-write-data__\n[other]\nwrite-data=ignored";
        fs::write(&config, bytes).unwrap();
        let target = factorio_mods(&root, Some(&exe), Some(&appdata), "").unwrap();
        assert_eq!(target, appdata.join("Factorio/mods"));
        assert!(!target.exists());
        assert_eq!(fs::read_to_string(config).unwrap(), bytes);
    }

    #[test]
    fn portable_and_custom_config_paths_are_respected() {
        let (temp, root, exe, appdata) = fixture();
        fs::create_dir_all(root.join("config")).unwrap();
        fs::write(root.join("config-path.cfg"), "config-path=__PATH__executable__/../../config\nuse-system-read-write-data-directories=false").unwrap();
        fs::write(
            root.join("config/config.ini"),
            "[path]\nwrite-data=__PATH__executable__/../..",
        )
        .unwrap();
        assert_eq!(
            factorio_mods(&root, Some(&exe), Some(&appdata), "").unwrap(),
            root.join("mods")
        );
        let custom = temp.path().join("Custom data");
        fs::write(
            root.join("config/config.ini"),
            format!("[path]\nwrite-data=\"{}\"", custom.display()),
        )
        .unwrap();
        assert_eq!(
            factorio_mods(&root, Some(&exe), Some(&appdata), "").unwrap(),
            custom.join("mods")
        );
    }

    #[test]
    fn command_line_overrides_take_precedence_without_writes() {
        let (temp, root, exe, appdata) = fixture();
        let target = temp.path().join("Custom Mods");
        let args = format!("--mod-directory=\"{}\"", target.display());
        assert_eq!(
            factorio_mods(&root, Some(&exe), Some(&appdata), &args).unwrap(),
            target
        );
        let config = temp.path().join("custom.ini");
        fs::write(
            &config,
            format!("[path]\nwrite-data={}", temp.path().display()),
        )
        .unwrap();
        let args = format!("-c \"{}\"", config.display());
        assert_eq!(
            factorio_mods(&root, Some(&exe), Some(&appdata), &args).unwrap(),
            temp.path().join("mods")
        );
        assert!(!target.exists());
        for args in [
            "--mod-directory=",
            "--mod-directory relative",
            "--config",
            "-c one -c two",
            "--mod-directory x --mod-directory y",
        ] {
            assert!(
                factorio_mods(&root, Some(&exe), Some(&appdata), args).is_err(),
                "{args}"
            );
        }
    }

    #[test]
    fn missing_ambiguous_and_oversized_metadata_stays_unknown() {
        let (_temp, root, exe, appdata) = fixture();
        assert!(factorio_mods(&root, Some(&exe), Some(&appdata), "").is_err());
        for config in [
            "config-path=one\nconfig-path=two",
            "config-path=relative",
            "config-path=__PATH__unknown__/config",
            "config-path=__PATH__system-write-data__/config ; comment",
            "config-path=__PATH__executable__//outside",
            "config-path=__PATH__executable__/../../missing",
        ] {
            fs::write(root.join("config-path.cfg"), config).unwrap();
            assert!(factorio_mods(&root, Some(&exe), Some(&appdata), "").is_err());
        }
        fs::write(root.join("config-path.cfg"), vec![b'x'; 1024 * 1024 + 1]).unwrap();
        assert!(factorio_mods(&root, Some(&exe), Some(&appdata), "").is_err());
        assert!(
            config_values(
                "[path]\nwrite-data=one\n[path]\nwrite-data=two",
                "path",
                &["write-data"]
            )
            .is_err()
        );
        assert!(config_values("[path\nwrite-data=one", "path", &["write-data"]).is_err());
    }

    #[test]
    fn bootstrap_defaults_do_not_guess_missing_active_paths() {
        let (_temp, root, exe, appdata) = fixture();
        let config = appdata.join("Factorio/config/config.ini");
        fs::write(&config, "[path]\n; write-data not set").unwrap();
        for mode in ["true", "false", "maybe"] {
            fs::write(root.join("config-path.cfg"), format!("config-path=__PATH__system-write-data__/config\nuse-system-read-write-data-directories={mode}")).unwrap();
            assert!(factorio_mods(&root, Some(&exe), Some(&appdata), "").is_err());
        }
    }

    #[test]
    fn unsafe_paths_and_files_are_not_suggested_as_directories() {
        let (temp, root, exe, appdata) = fixture();
        for value in [
            r"\\server\share\mods",
            r"\\?\C:\mods",
            "relative",
            "C:/",
            "C:/mods:stream",
            "C:/CON",
            "C:/trailing.",
        ] {
            assert!(literal_target(value).is_err(), "{value}");
        }
        let file = temp.path().join("config.ini");
        fs::write(&file, "not a directory").unwrap();
        assert!(
            factorio_mods(
                &root,
                Some(&exe),
                Some(&appdata),
                &format!("--mod-directory \"{}\"", file.display())
            )
            .is_err()
        );
    }

    #[test]
    fn hytale_patchlines_keep_distinct_data_targets_and_never_create_mods() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("Hytale");
        for path in [
            "UserData",
            "data/pre-release",
            "data/orphan",
            "install/release/package/game/latest",
            "install/pre-release/package/game/latest",
        ] {
            fs::create_dir_all(root.join(path)).unwrap();
        }
        fs::write(root.join("hytale-launcher.exe"), "not executed").unwrap();
        let found = detect_hytale(Some(temp.path()), None);
        assert_eq!(found.len(), 2);
        let release = found
            .iter()
            .find(|p| p.label == "Hytale · release")
            .unwrap();
        let preview = found
            .iter()
            .find(|p| p.label == "Hytale · pre-release")
            .unwrap();
        assert_eq!(
            release.modpack_dir.as_deref().map(Path::new),
            Some(root.join("UserData/Mods").as_path())
        );
        assert_eq!(
            preview.modpack_dir.as_deref().map(Path::new),
            Some(root.join("data/pre-release/Mods").as_path())
        );
        assert_ne!(preview.install_dir, release.install_dir);
        assert!(!root.join("UserData/Mods").exists());
        assert!(detect_hytale(None, None).is_empty());
    }

    #[test]
    fn macro_overrides_are_not_silently_ignored() {
        assert!(factorio_overrides("--executable-path C:/other").is_err());
        assert!(factorio_overrides("--executable-path=C:/other").is_err());
        assert!(factorio_overrides("-cC:/other/config.ini").is_err());
    }
}
