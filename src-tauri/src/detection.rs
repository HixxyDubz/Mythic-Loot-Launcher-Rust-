use std::{
    collections::HashSet,
    env, fs,
    path::{Path, PathBuf},
};

use crate::models::{DetectedInstall, GameProfile};

pub fn detect(profile: &GameProfile) -> Vec<DetectedInstall> {
    let mut installs = match profile.game.as_str() {
        "minecraft" => detect_minecraft(),
        game => detect_steam_game(game),
    };
    if let Some(configured) = configured_install(profile) {
        installs.insert(0, configured);
    }
    deduplicate(installs)
}

fn configured_install(profile: &GameProfile) -> Option<DetectedInstall> {
    let exe = nonempty_path(&profile.game_exe_path);
    let install = nonempty_path(&profile.install_dir).or_else(|| {
        exe.as_ref()
            .and_then(|path| path.parent().map(Path::to_path_buf))
    })?;
    if !install.exists() && exe.as_ref().is_none_or(|path| !path.exists()) {
        return None;
    }
    Some(DetectedInstall {
        label: "Current configuration".into(),
        exe_path: exe.map(|path| path.display().to_string()),
        install_dir: install.display().to_string(),
        source: "configured".into(),
    })
}

fn detect_minecraft() -> Vec<DetectedInstall> {
    let mut found = Vec::new();
    let appdata = env_path("APPDATA");
    let local = env_path("LOCALAPPDATA");
    let home = env_path("USERPROFILE");
    let program_files_x86 = env_path("ProgramFiles(x86)");

    let official_exe = first_file([
        program_files_x86
            .as_ref()
            .map(|path| path.join("Minecraft Launcher/MinecraftLauncher.exe")),
        local
            .as_ref()
            .map(|path| path.join("Programs/Minecraft Launcher/MinecraftLauncher.exe")),
    ]);
    if let Some(root) = appdata.as_ref().map(|path| path.join(".minecraft"))
        && root.is_dir()
    {
        found.push(candidate(
            "Minecraft (official)",
            official_exe,
            root,
            "official",
        ));
    }

    let curseforge_exe = first_file([
        local
            .as_ref()
            .map(|path| path.join("Programs/CurseForge Windows/CurseForge.exe")),
        program_files_x86
            .as_ref()
            .map(|path| path.join("Overwolf/OverwolfLauncher.exe")),
    ]);
    if let Some(instances) = home
        .as_ref()
        .map(|path| path.join("curseforge/minecraft/Instances"))
        && instances.is_dir()
    {
        add_child_directories(
            &mut found,
            &instances,
            "CurseForge",
            curseforge_exe.clone(),
            "curseforge",
        );
    }

    let modrinth_exe = first_file([
        local
            .as_ref()
            .map(|path| path.join("Programs/Modrinth App/Modrinth App.exe")),
        local
            .as_ref()
            .map(|path| path.join("Programs/Modrinth App/modrinth-app.exe")),
    ]);
    if let Some(profiles) = appdata
        .as_ref()
        .map(|path| path.join("ModrinthApp/profiles"))
        && profiles.is_dir()
    {
        add_child_directories(&mut found, &profiles, "Modrinth", modrinth_exe, "modrinth");
    }

    let prism_exe = first_file([
        local
            .as_ref()
            .map(|path| path.join("Programs/PrismLauncher/prismlauncher.exe")),
        program_files_x86
            .as_ref()
            .map(|path| path.join("PrismLauncher/prismlauncher.exe")),
    ]);
    if let Some(instances) = appdata
        .as_ref()
        .map(|path| path.join("PrismLauncher/instances"))
        && instances.is_dir()
    {
        add_child_directories(&mut found, &instances, "Prism", prism_exe, "prism");
    }
    found
}

fn add_child_directories(
    output: &mut Vec<DetectedInstall>,
    parent: &Path,
    prefix: &str,
    exe: Option<PathBuf>,
    source: &str,
) {
    let Ok(entries) = fs::read_dir(parent) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        output.push(candidate(
            &format!("{prefix} · {}", entry.file_name().to_string_lossy()),
            exe.clone(),
            path,
            source,
        ));
    }
}

struct SteamSpec {
    folder: &'static str,
    executables: &'static [&'static str],
}

fn steam_spec(game: &str) -> Option<SteamSpec> {
    match game {
        "seven_days" => Some(SteamSpec {
            folder: "7 Days To Die",
            executables: &["7DaysToDie.exe", "7dLauncher.exe"],
        }),
        "palworld" => Some(SteamSpec {
            folder: "Palworld",
            executables: &[
                "Palworld.exe",
                "Pal/Binaries/Win64/Palworld-Win64-Shipping.exe",
            ],
        }),
        "core_keeper" => Some(SteamSpec {
            folder: "Core Keeper",
            executables: &["CoreKeeper.exe"],
        }),
        "marvel_heroes" => Some(SteamSpec {
            folder: "Marvel Heroes",
            executables: &[
                "UnrealEngine3/Binaries/Win64/MarvelGame.exe",
                "MarvelGame.exe",
            ],
        }),
        "valheim" => Some(SteamSpec {
            folder: "Valheim",
            executables: &["valheim.exe"],
        }),
        "factorio" => Some(SteamSpec {
            folder: "Factorio",
            executables: &["bin/x64/factorio.exe"],
        }),
        "stardew_valley" => Some(SteamSpec {
            folder: "Stardew Valley",
            executables: &["Stardew Valley.exe"],
        }),
        _ => None,
    }
}

fn detect_steam_game(game: &str) -> Vec<DetectedInstall> {
    let Some(spec) = steam_spec(game) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for root in steam_library_roots() {
        let install = root.join("steamapps/common").join(spec.folder);
        if !install.is_dir() {
            continue;
        }
        let exe = spec
            .executables
            .iter()
            .map(|relative| install.join(relative))
            .find(|path| path.is_file());
        found.push(candidate(
            &format!("Steam · {}", spec.folder),
            exe,
            install,
            "steam",
        ));
    }
    found
}

fn steam_library_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(program_files_x86) = env_path("ProgramFiles(x86)") {
        roots.push(program_files_x86.join("Steam"));
    }
    if let Some(program_files) = env_path("ProgramFiles") {
        roots.push(program_files.join("Steam"));
    }
    let mut extra = Vec::new();
    for root in &roots {
        let vdf = root.join("steamapps/libraryfolders.vdf");
        if let Ok(text) = fs::read_to_string(vdf) {
            extra.extend(parse_steam_library_paths(&text));
        }
    }
    roots.extend(extra);
    let mut seen = HashSet::new();
    roots
        .into_iter()
        .filter(|path| seen.insert(normalized(path)))
        .collect()
}

fn parse_steam_library_paths(text: &str) -> Vec<PathBuf> {
    text.lines()
        .filter_map(|line| {
            let quoted: Vec<&str> = line.split('"').skip(1).step_by(2).collect();
            quoted.windows(2).find_map(|pair| {
                pair[0]
                    .trim()
                    .eq_ignore_ascii_case("path")
                    .then(|| PathBuf::from(pair[1].replace("\\\\", "\\")))
            })
        })
        .collect()
}

fn candidate(label: &str, exe: Option<PathBuf>, install: PathBuf, source: &str) -> DetectedInstall {
    DetectedInstall {
        label: label.into(),
        exe_path: exe
            .filter(|path| path.is_file())
            .map(|path| path.display().to_string()),
        install_dir: install.display().to_string(),
        source: source.into(),
    }
}

fn deduplicate(installs: Vec<DetectedInstall>) -> Vec<DetectedInstall> {
    let mut seen = HashSet::new();
    installs
        .into_iter()
        .filter(|install| seen.insert(install.install_dir.to_lowercase()))
        .collect()
}

fn env_path(name: &str) -> Option<PathBuf> {
    env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn nonempty_path(value: &str) -> Option<PathBuf> {
    (!value.trim().is_empty()).then(|| PathBuf::from(value.trim()))
}

fn first_file<const N: usize>(candidates: [Option<PathBuf>; N]) -> Option<PathBuf> {
    candidates.into_iter().flatten().find(|path| path.is_file())
}

fn normalized(path: &Path) -> String {
    path.to_string_lossy().replace('/', "\\").to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_modern_steam_vdf_paths() {
        let fixture = r#"
            "0" { "path" "C:\\Program Files (x86)\\Steam" }
            "1" { "path" "D:\\Games\\SteamLibrary" }
        "#;
        let paths = parse_steam_library_paths(fixture);
        assert_eq!(paths.len(), 2);
        assert!(paths[1].to_string_lossy().contains("SteamLibrary"));
    }
}
