use std::{
    path::Path,
    process::{Child, Command},
};

use crate::models::{GameProfile, LaunchOutcome};

pub fn launch(profile: &GameProfile, close_after_launch: bool) -> Result<LaunchOutcome, String> {
    let child = spawn(profile)?;
    let message = match profile.minecraft_launcher.as_str() {
        "curseforge" => format!("Opened CurseForge for {}", profile.display_name),
        "modrinth" => format!("Opened Modrinth for {}", profile.display_name),
        _ => format!("Started {}", profile.display_name),
    };
    Ok(LaunchOutcome {
        pid: child.id(),
        message,
        close_after_launch,
    })
}

pub(crate) fn spawn(profile: &GameProfile) -> Result<Child, String> {
    let executable = Path::new(profile.game_exe_path.trim());
    if !executable.is_file() {
        return Err("The configured game executable does not exist".into());
    }
    let arguments = split_windows_args(&profile.launch_args)?;
    let working_directory = working_directory(profile)?;
    Command::new(executable)
        .args(&arguments)
        .current_dir(working_directory)
        .spawn()
        .map_err(|error| format!("Could not start {}: {error}", profile.display_name))
}

fn working_directory(profile: &GameProfile) -> Result<&Path, String> {
    let executable = Path::new(profile.game_exe_path.trim());
    let parent = executable
        .parent()
        .ok_or_else(|| "The game executable has no parent directory".to_string())?;
    let working_directory = if profile.game == "minecraft"
        && crate::java_runtime::is_java_executable(executable)
    {
        let directory = if profile.game_dir.trim().is_empty() {
            &profile.install_dir
        } else {
            &profile.game_dir
        };
        let directory = Path::new(directory.trim());
        if !directory.is_absolute() || !directory.is_dir() {
            return Err("A direct Java launch needs an existing absolute game directory".into());
        }
        directory
    } else {
        parent
    };
    Ok(working_directory)
}

pub fn split_windows_args(input: &str) -> Result<Vec<String>, String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut chars = input.chars().peekable();
    let mut in_quotes = false;
    let mut started = false;

    while let Some(character) = chars.next() {
        if !character.is_whitespace() || in_quotes {
            started = true;
        }
        match character {
            '"' => in_quotes = !in_quotes,
            '\\' => {
                let mut slashes = 1;
                while chars.peek() == Some(&'\\') {
                    chars.next();
                    slashes += 1;
                }
                if chars.peek() == Some(&'"') {
                    for _ in 0..(slashes / 2) {
                        current.push('\\');
                    }
                    if slashes % 2 == 1 {
                        chars.next();
                        current.push('"');
                    }
                } else {
                    for _ in 0..slashes {
                        current.push('\\');
                    }
                }
            }
            value if value.is_whitespace() && !in_quotes => {
                if started {
                    args.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            value => current.push(value),
        }
    }
    if in_quotes {
        return Err("Launch arguments contain an unmatched quote".into());
    }
    if started {
        args.push(current);
    }
    Ok(args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_explicit_empty_arguments() {
        assert_eq!(
            split_windows_args(r#"--name "" "C:\Game Dir\\""#).unwrap(),
            vec!["--name", "", "C:\\Game Dir\\"]
        );
    }

    #[test]
    fn direct_java_uses_game_data_directory_not_java_bin() {
        let root = tempfile::tempdir().unwrap();
        let mut profile = crate::models::LauncherConfig::default().profiles.remove(0);
        profile.game_exe_path = root.path().join("bin/java.exe").display().to_string();
        profile.install_dir = root.path().display().to_string();
        assert_eq!(working_directory(&profile).unwrap(), root.path());
        profile.game_dir = root.path().join("missing-game").display().to_string();
        assert!(working_directory(&profile).is_err());
        profile.game_exe_path = root
            .path()
            .join("launcher/CurseForge.exe")
            .display()
            .to_string();
        assert_eq!(
            working_directory(&profile).unwrap(),
            root.path().join("launcher")
        );
    }

    #[test]
    fn preserves_quoted_windows_arguments() {
        assert_eq!(
            split_windows_args(r#"--profile "My Modpack" --flag"#).unwrap(),
            vec!["--profile", "My Modpack", "--flag"]
        );
    }

    #[test]
    fn rejects_unmatched_quotes() {
        assert!(split_windows_args("--profile \"broken").is_err());
    }
}
