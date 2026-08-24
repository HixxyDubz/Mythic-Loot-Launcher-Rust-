use std::{path::Path, process::Command};

use crate::models::{GameProfile, LaunchOutcome};

pub fn launch(profile: &GameProfile) -> Result<LaunchOutcome, String> {
    let executable = Path::new(profile.game_exe_path.trim());
    if !executable.is_file() {
        return Err("The configured game executable does not exist".into());
    }
    let mut arguments = split_windows_args(&profile.launch_args)?;
    let mut joins_directly = false;
    if !profile.server_ip.trim().is_empty()
        && let Some(join) = direct_join_args(profile)
    {
        arguments.extend(join);
        joins_directly = true;
    }
    let parent = executable
        .parent()
        .ok_or_else(|| "The game executable has no parent directory".to_string())?;
    let child = Command::new(executable)
        .args(&arguments)
        .current_dir(parent)
        .spawn()
        .map_err(|error| format!("Could not start {}: {error}", profile.display_name))?;

    let join_hint = if profile.server_ip.trim().is_empty() || joins_directly {
        String::new()
    } else {
        format!(
            "Join {}:{} from the in-game multiplayer menu.",
            profile.server_ip, profile.server_port
        )
    };
    Ok(LaunchOutcome {
        pid: child.id(),
        message: format!("Started {}", profile.display_name),
        join_hint,
    })
}

fn direct_join_args(profile: &GameProfile) -> Option<Vec<String>> {
    let address = format!("{}:{}", profile.server_ip.trim(), profile.server_port);
    match profile.game.as_str() {
        "seven_days" => Some(vec![
            format!("-connect={}", profile.server_ip.trim()),
            format!("-port={}", profile.server_port),
        ]),
        "factorio" => Some(vec!["--mp-connect".into(), address]),
        _ => None,
    }
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
            split_windows_args(r#"--profile "My Server" --flag"#).unwrap(),
            vec!["--profile", "My Server", "--flag"]
        );
    }

    #[test]
    fn rejects_unmatched_quotes() {
        assert!(split_windows_args("--profile \"broken").is_err());
    }
}
