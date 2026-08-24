use std::path::Path;

use crate::models::{GameProfile, ProfileHealth, ReadinessStatus};

pub fn assess(profile: &GameProfile) -> ProfileHealth {
    let mut details = Vec::new();
    let exe = profile.game_exe_path.trim();
    if exe.is_empty() {
        return health(
            profile,
            ReadinessStatus::SetupRequired,
            "Choose or detect the game client",
            vec!["No game executable is configured.".into()],
        );
    }
    if !Path::new(exe).is_file() {
        return health(
            profile,
            ReadinessStatus::GamePathMissing,
            "The configured game client was not found",
            vec![exe.into()],
        );
    }
    details.push("Game client found".into());

    let install = profile.install_dir.trim();
    if install.is_empty() {
        return health(
            profile,
            ReadinessStatus::SetupRequired,
            "Choose the modpack folder",
            details,
        );
    }
    if !Path::new(install).is_dir() {
        details.push(install.into());
        return health(
            profile,
            ReadinessStatus::SetupRequired,
            "The configured modpack folder was not found",
            details,
        );
    }
    details.push("Modpack folder found".into());

    if !profile.required_modpack_version.trim().is_empty()
        && profile.local_modpack_version.trim() != profile.required_modpack_version.trim()
    {
        details.push(format!(
            "Installed: {} · Required: {}",
            value_or_unknown(&profile.local_modpack_version),
            profile.required_modpack_version
        ));
        return health(
            profile,
            ReadinessStatus::UpdateRequired,
            "The modpack version needs attention",
            details,
        );
    }

    if profile.server_ip.trim().is_empty() {
        details.push("Server address is not configured yet".into());
    }
    health(profile, ReadinessStatus::Ready, "Ready to play", details)
}

fn health(
    profile: &GameProfile,
    status: ReadinessStatus,
    headline: &str,
    details: Vec<String>,
) -> ProfileHealth {
    ProfileHealth {
        profile_id: profile.id.clone(),
        status,
        headline: headline.into(),
        details,
    }
}

fn value_or_unknown(value: &str) -> &str {
    if value.trim().is_empty() {
        "Not verified"
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::LauncherConfig;

    #[test]
    fn defaults_are_honestly_setup_required() {
        for profile in LauncherConfig::default().profiles {
            assert_eq!(assess(&profile).status, ReadinessStatus::SetupRequired);
        }
    }

    #[test]
    fn never_marks_an_unknown_modpack_current() {
        let root = tempfile::tempdir().expect("temporary directory");
        let exe = root.path().join("game.exe");
        std::fs::write(&exe, b"fixture").expect("fixture exe");
        let mut profile = LauncherConfig::default().profiles.remove(0);
        profile.game_exe_path = exe.display().to_string();
        profile.install_dir = root.path().display().to_string();
        assert_eq!(assess(&profile).status, ReadinessStatus::UpdateRequired);
    }
}
