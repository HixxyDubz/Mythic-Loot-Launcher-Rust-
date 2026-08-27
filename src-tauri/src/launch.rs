use std::{
    path::Path,
    process::{Child, Command},
};

use crate::models::{GameProfile, LaunchOutcome};

pub fn launch(profile: &GameProfile) -> Result<LaunchOutcome, String> {
    let child = spawn(profile)?;
    Ok(LaunchOutcome {
        pid: child.id(),
        message: format!("Started {}", profile.display_name),
    })
}

pub(crate) fn spawn(profile: &GameProfile) -> Result<Child, String> {
    let executable = Path::new(profile.game_exe_path.trim());
    if !executable.is_file() {
        return Err("The configured game executable does not exist".into());
    }
    let arguments = split_windows_args(&profile.launch_args)?;
    let parent = executable
        .parent()
        .ok_or_else(|| "The game executable has no parent directory".to_string())?;
    Command::new(executable)
        .args(&arguments)
        .current_dir(parent)
        .spawn()
        .map_err(|error| format!("Could not start {}: {error}", profile.display_name))
}

pub fn split_windows_args(input: &str) -> Result<Vec<String>, String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut chars = input.chars().peekable();
    let mut in_quotes = false;

    while let Some(character) = chars.next() {
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
                if !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                }
            }
            value => current.push(value),
        }
    }
    if in_quotes {
        return Err("Launch arguments contain an unmatched quote".into());
    }
    if !current.is_empty() {
        args.push(current);
    }
    Ok(args)
}

#[cfg(test)]
mod tests {
    use super::*;

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
