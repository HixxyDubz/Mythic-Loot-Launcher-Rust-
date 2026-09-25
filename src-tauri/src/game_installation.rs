//! Read-only local evidence, separate from installed-modpack state and launch readiness.
use crate::{detection, manifest, models::GameProfile, operations::WorkGuard, safe_path, storage};
use serde::Serialize;
use std::{collections::HashMap, fs::File, io::Read, path::Path};
use tauri::AppHandle;

const MAX_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathCheck {
    label: String,
    path: String,
    status: &'static str,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameInstallationCheck {
    profile_id: String,
    game: String,
    paths: Vec<PathCheck>,
    client_found: Option<bool>,
    game_version: Option<String>,
    required_game_version: Option<String>,
    comparison: &'static str,
    steam_build_id: Option<String>,
    notes: Vec<String>,
}

#[tauri::command]
pub async fn inspect_game_installation(
    app: AppHandle,
    profile_id: String,
    game: String,
    game_directory: String,
    install_directory: String,
    executable: String,
) -> Result<GameInstallationCheck, String> {
    let guard = WorkGuard::begin()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        let config = storage::load_or_create(&app)?;
        let saved = config
            .profiles
            .iter()
            .find(|p| p.id == profile_id)
            .ok_or("Choose a saved modpack profile first")?;
        if saved.game != game {
            return Err("Save the selected game type before checking its installation".into());
        }
        let published = manifest::load_for_profile(&app, saved);
        let expected = published
            .summary
            .valid
            .then_some(published.manifest.required_game_version);
        let mut local = saved.clone();
        local.game_dir = game_directory;
        local.install_dir = install_directory;
        local.game_exe_path = executable;
        Ok(inspect_at(
            &local,
            expected.filter(|v| !v.trim().is_empty()),
        ))
    })
    .await
    .map_err(|_| "Local installation check could not finish".to_string())?
}

fn check_path(label: &str, raw: &str, directory: bool) -> PathCheck {
    let path = Path::new(raw.trim());
    let status = if raw.trim().is_empty() {
        "unconfigured"
    } else if !path.is_absolute() || safe_path::reject_link_path(path).is_err() {
        "unsafe"
    } else if if directory {
        path.is_dir()
    } else {
        path.is_file()
    } {
        "present"
    } else {
        "missing"
    };
    PathCheck {
        label: label.into(),
        path: raw.trim().into(),
        status,
    }
}

fn inspect_at(profile: &GameProfile, expected: Option<String>) -> GameInstallationCheck {
    let paths = vec![
        check_path("Game directory", &profile.game_dir, true),
        check_path("Modpack base folder", &profile.install_dir, true),
        check_path("Game or launcher executable", &profile.game_exe_path, false),
    ];
    let root_ok = paths[0].status == "present";
    let root = Path::new(profile.game_dir.trim());
    let mut result = GameInstallationCheck {
        profile_id: profile.id.clone(),
        game: profile.game.clone(),
        paths,
        client_found: None,
        game_version: None,
        required_game_version: expected,
        comparison: "unknown",
        steam_build_id: None,
        notes: vec![],
    };
    if let Some(spec) = detection::steam_spec(&profile.game) {
        let found = root_ok
            && spec.executables.iter().any(|relative| {
                let path = root.join(relative);
                path.is_file() && safe_path::reject_link_path(&path).is_ok()
            });
        result.client_found = Some(found);
        if found && result.paths[2].status == "present" {
            let selected = std::fs::canonicalize(profile.game_exe_path.trim()).ok();
            if !spec.executables.iter().any(|relative| {
                std::fs::canonicalize(root.join(relative))
                    .ok()
                    .is_some_and(|path| Some(path) == selected)
            }) {
                result.notes.push("The selected launch executable differs from the recognised client under this game root. Confirm the custom launcher or select the correct client before launching.".into());
            }
        }
        if !found {
            result.notes.push("No recognised client executable was found under this game directory. Check that this is the game root, not its Mods folder.".into());
        }
        if found && let Some(app_id) = spec.app_id {
            match steam_build(root, app_id) {
                Ok(build) => result.steam_build_id = build,
                Err(error) => result.notes.push(error),
            }
        }
        if found && profile.game == "factorio" {
            match factorio_version(root) {
                Ok(version) => result.game_version = Some(version),
                Err(error) => result.notes.push(error),
            }
        }
    } else {
        result.notes.push("This client uses manual folder selection; no known executable layout or reliable version reader is assumed.".into());
    }
    if profile.game == "seven_days" {
        result.notes.push("7DTD executable resource versions can describe Unity, not the game. Steam build IDs are shown separately and are never compared with a game version such as 3.1.".into());
    }
    if profile.game == "factorio" {
        result.notes.push("Factorio's standard Steam user-data folder may be separate from the game root. Portable/custom write-data paths require manual confirmation; no configuration is changed.".into());
    }
    if result.paths[1].status == "missing" {
        result.notes.push("The modpack folder does not exist yet. This check does not create it; review the target before installing a pack.".into());
    }
    if let (Some(local), Some(required)) = (&result.game_version, &result.required_game_version)
        && exact_version(required).is_some()
    {
        result.comparison = if local == required {
            "matches"
        } else {
            "mismatch"
        };
    }
    if result.game_version.is_none() {
        result.notes.push("Installed game version cannot be determined from the supported local metadata. Confirm it in the game or its launcher; no version was guessed.".into());
    }
    result.notes.push("Advisory metadata/path check only. It does not verify game binaries, change launch readiness, save settings, install files or contact a server.".into());
    result
}

pub(crate) fn read_text(path: &Path) -> Result<String, String> {
    safe_path::reject_link_path(path).map_err(|_| "Linked metadata paths are not inspected")?;
    let file = File::open(path).map_err(|_| "Supported local metadata is unavailable")?;
    let metadata = file
        .metadata()
        .map_err(|_| "Cannot inspect local metadata")?;
    if !metadata.is_file() || metadata.len() > MAX_BYTES {
        return Err("Local metadata must be a regular file no larger than 1 MiB".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read local metadata")?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("Local metadata exceeds the 1 MiB limit".into());
    }
    String::from_utf8(bytes)
        .map(|s| s.trim_start_matches('\u{feff}').to_string())
        .map_err(|_| "Unsupported local metadata encoding".into())
}
fn exact_version(value: &str) -> Option<&str> {
    let parts: Vec<_> = value.split('.').collect();
    (parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.len() <= 5 && p.bytes().all(|b| b.is_ascii_digit())))
    .then_some(value)
}
fn factorio_version(root: &Path) -> Result<String, String> {
    let text = read_text(&root.join("data/base/info.json"))?;
    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|_| "Factorio base metadata is malformed")?;
    if json.get("name").and_then(|v| v.as_str()) != Some("base") {
        return Err("Factorio metadata does not identify the base game".into());
    }
    json.get("version")
        .and_then(|v| v.as_str())
        .and_then(exact_version)
        .map(str::to_string)
        .ok_or_else(|| "Factorio base version is missing or unsupported".into())
}
fn steam_build(root: &Path, app_id: &str) -> Result<Option<String>, String> {
    let Some(common) = root.parent().filter(|p| {
        p.file_name()
            .is_some_and(|n| n.eq_ignore_ascii_case("common"))
    }) else {
        return Ok(None);
    };
    let Some(steamapps) = common.parent().filter(|p| {
        p.file_name()
            .is_some_and(|n| n.eq_ignore_ascii_case("steamapps"))
    }) else {
        return Ok(None);
    };
    let text = read_text(&steamapps.join(format!("appmanifest_{app_id}.acf")))?;
    let fields = acf_fields(&text)?;
    if fields.get("appid").map(String::as_str) != Some(app_id)
        || !fields.get("installdir").is_some_and(|name| {
            root.file_name()
                .is_some_and(|part| part.eq_ignore_ascii_case(name))
        })
    {
        return Err("Steam metadata does not identify this game and folder".into());
    }
    if fields.get("stateflags").map(String::as_str) != Some("4") {
        return Err("Steam does not report a fully installed, idle build; finish its installation/update first".into());
    }
    fields
        .get("buildid")
        .filter(|id| {
            !id.is_empty()
                && id.len() <= 20
                && id.bytes().all(|b| b.is_ascii_digit())
                && id.bytes().any(|b| b != b'0')
        })
        .cloned()
        .map(Some)
        .ok_or_else(|| "Steam installed build ID is unavailable".into())
}

// Strict bounded Valve KeyValues subset used by local app manifests. Only the
// four allowlisted AppState fields leave this parser; nested/owner data is ignored.
fn acf_fields(text: &str) -> Result<HashMap<String, String>, String> {
    #[derive(PartialEq)]
    enum Token {
        Text(String),
        Open,
        Close,
    }
    let error = || "Steam metadata is malformed or ambiguous".to_string();
    let mut chars = text.chars().peekable();
    let mut tokens = Vec::new();
    while let Some(c) = chars.next() {
        match c {
            c if c.is_whitespace() => {}
            '/' if chars.next_if_eq(&'/').is_some() => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            '{' => tokens.push(Token::Open),
            '}' => tokens.push(Token::Close),
            '"' => {
                let mut value = String::new();
                let mut closed = false;
                while let Some(c) = chars.next() {
                    if c == '"' {
                        closed = true;
                        break;
                    }
                    if c == '\\' {
                        let next = chars.next().ok_or_else(error)?;
                        if !matches!(next, '\\' | '"') {
                            return Err(error());
                        }
                        value.push(next);
                    } else {
                        value.push(c);
                    }
                }
                if !closed {
                    return Err(error());
                }
                tokens.push(Token::Text(value));
            }
            _ => return Err(error()),
        }
        if tokens.len() > 65536 {
            return Err(error());
        }
    }
    if tokens.first() != Some(&Token::Text("AppState".into()))
        || tokens.get(1) != Some(&Token::Open)
    {
        return Err(error());
    }
    let mut fields = HashMap::new();
    let mut depth = 1;
    let mut index = 2;
    while index < tokens.len() {
        match &tokens[index] {
            Token::Close => {
                depth -= 1;
                index += 1;
                if depth == 0 {
                    return if index == tokens.len() {
                        Ok(fields)
                    } else {
                        Err(error())
                    };
                }
            }
            Token::Text(key) => {
                match tokens.get(index + 1) {
                    Some(Token::Open) => {
                        depth += 1;
                        if depth > 16 {
                            return Err(error());
                        }
                    }
                    Some(Token::Text(value)) => {
                        let key = key.to_ascii_lowercase();
                        if depth == 1
                            && matches!(
                                key.as_str(),
                                "appid" | "installdir" | "buildid" | "stateflags"
                            )
                            && fields.insert(key, value.clone()).is_some()
                        {
                            return Err(error());
                        }
                    }
                    _ => return Err(error()),
                }
                index += 2;
            }
            Token::Open => return Err(error()),
        }
    }
    Err(error())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    fn fixture(root: &Path, game: &str) -> GameProfile {
        let mut p = crate::models::LauncherConfig::default().profiles.remove(1);
        p.game = game.into();
        p.game_dir = root.display().to_string();
        p.install_dir = root.join("Mods").display().to_string();
        p.game_exe_path = root
            .join(detection::steam_spec(game).unwrap().executables[0])
            .display()
            .to_string();
        p
    }
    #[test]
    fn never_treats_steam_build_as_a_game_version_or_exposes_owner_data() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("steamapps/common/7 Days To Die");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("7DaysToDie.exe"), b"not executed").unwrap();
        let acf = temp.path().join("steamapps/appmanifest_251570.acf");
        let bytes = br#""AppState" { "appid" "251570" "installdir" "7 Days To Die" "buildid" "123456" "StateFlags" "4" "LastOwner" "private-owner" "UserConfig" { "buildid" "wrong" } }"#;
        fs::write(&acf, bytes).unwrap();
        let result = inspect_at(&fixture(&root, "seven_days"), Some("3.1".into()));
        assert_eq!(result.steam_build_id.as_deref(), Some("123456"));
        assert!(result.game_version.is_none());
        assert_eq!(result.comparison, "unknown");
        assert!(
            !serde_json::to_string(&result)
                .unwrap()
                .contains("private-owner")
        );
        assert_eq!(fs::read(acf).unwrap(), bytes);
        assert!(!root.join("Mods").exists());
    }
    #[test]
    fn factorio_declared_version_compares_only_exact_supported_requirements() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("bin/x64")).unwrap();
        fs::create_dir_all(root.join("data/base")).unwrap();
        fs::write(root.join("bin/x64/factorio.exe"), b"not executed").unwrap();
        fs::write(
            root.join("data/base/info.json"),
            br#"{"name":"base","version":"2.0.72","account":"private"}"#,
        )
        .unwrap();
        let p = fixture(root, "factorio");
        for (required, comparison) in [
            (Some("2.0.72"), "matches"),
            (Some("2.0.73"), "mismatch"),
            (Some("2.0"), "unknown"),
            (None, "unknown"),
        ] {
            let result = inspect_at(&p, required.map(str::to_string));
            assert_eq!(result.comparison, comparison);
            assert_eq!(result.game_version.as_deref(), Some("2.0.72"));
        }
    }
    #[test]
    fn malformed_duplicate_nested_and_wrong_steam_metadata_fail_closed() {
        for text in [
            r#""AppState" { "buildid" "1" "buildid" "2" }"#,
            r#""AppState" { "buildid" "1""#,
            r#""AppState" { } extra"#,
            r#""Other" {}"#,
        ] {
            assert!(acf_fields(text).is_err());
        }
        let fields = acf_fields(
            r#""AppState" { // ignored
            "Nested" { "buildid" "999" } "buildid" "123" "appid" "1" }"#,
        )
        .unwrap();
        assert_eq!(fields.get("buildid").unwrap(), "123");
        assert_eq!(fields.len(), 2);
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("steamapps/common/7 Days To Die");
        fs::create_dir_all(&root).unwrap();
        for (app, folder, state) in [
            ("999", "7 Days To Die", "4"),
            ("251570", "Another game", "4"),
            ("251570", "7 Days To Die", "6"),
        ] {
            fs::write(temp.path().join("steamapps/appmanifest_251570.acf"),format!(r#""AppState" {{ "appid" "{app}" "installdir" "{folder}" "StateFlags" "{state}" "buildid" "123" }}"#)).unwrap();
            assert!(steam_build(&root, "251570").is_err());
        }
    }
    #[test]
    fn missing_wrong_and_oversized_metadata_never_imply_compatibility() {
        let temp = tempfile::tempdir().unwrap();
        let p = fixture(temp.path(), "factorio");
        assert_eq!(
            inspect_at(&p, Some("2.0.72".into())).client_found,
            Some(false)
        );
        fs::create_dir_all(temp.path().join("data/base")).unwrap();
        let path = temp.path().join("data/base/info.json");
        for bytes in [
            b"broken private-data".as_slice(),
            br#"{"name":"another","version":"2.0.72"}"#,
        ] {
            fs::write(&path, bytes).unwrap();
            assert!(factorio_version(temp.path()).is_err());
        }
        File::create(&path).unwrap().set_len(MAX_BYTES + 1).unwrap();
        assert!(factorio_version(temp.path()).is_err());
        assert_eq!(check_path("test", "relative/path", true).status, "unsafe");
        assert_eq!(check_path("test", "", true).status, "unconfigured");
    }
    #[test]
    #[ignore = "Explicit read-only acceptance of the user-selected 7DTD installation"]
    fn inspect_real_seven_days_read_only() {
        let root = std::env::var_os("MYTHIC_LOOT_GAME_CHECK_DIR")
            .expect("Set the selected game directory");
        let result = inspect_at(&fixture(Path::new(&root), "seven_days"), None);
        assert_eq!(result.client_found, Some(true));
        assert!(result.steam_build_id.is_some());
        assert!(result.game_version.is_none());
        println!(
            "7DTD client found; Steam build {}; game version unknown",
            result.steam_build_id.unwrap()
        );
    }
}
