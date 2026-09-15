use crate::{models::LauncherPreferences, operations::WorkGuard, storage};
use tauri::AppHandle;

#[tauri::command]
pub fn save_preferences(
    app: AppHandle,
    preferences: LauncherPreferences,
) -> Result<LauncherPreferences, String> {
    let _guard = WorkGuard::begin()?;
    storage::update(&app, |config| {
        config.preferences = preferences.clone();
        Ok(preferences)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{LauncherConfig, LauncherFont, LauncherTheme};

    #[test]
    fn older_preferences_keep_their_values_and_gain_appearance_defaults() {
        let preferences: LauncherPreferences = serde_json::from_str(
            r#"{"reduceMotion":true,"autoCheckUpdates":false,"closeAfterLaunch":true}"#,
        )
        .unwrap();
        assert!(preferences.reduce_motion);
        assert!(!preferences.auto_check_updates);
        assert!(preferences.close_after_launch);
        assert_eq!(preferences.theme, LauncherTheme::Amethyst);
        assert_eq!(preferences.font, LauncherFont::System);
        assert!(preferences.decorative_background);
    }

    #[test]
    fn invalid_appearance_values_cannot_be_saved() {
        assert!(serde_json::from_str::<LauncherPreferences>(r#"{"theme":"remote-css"}"#).is_err());
        assert!(serde_json::from_str::<LauncherPreferences>(r#"{"font":"url(file)"}"#).is_err());
    }

    #[test]
    fn preferences_persist_without_replacing_profiles_or_optional_choices() {
        let root = tempfile::tempdir().unwrap();
        let mut original = LauncherConfig::default();
        original.profiles[0].install_dir = "C:\\Player\\Minecraft".into();
        original.profiles[0].local_modpack_version = "9.0".into();
        original.optional_selections.insert(
            "minecraft_main".into(),
            crate::models::OptionalSelection {
                install_dir: original.profiles[0].install_dir.clone(),
                enabled: vec!["mods/extra.jar".into()],
            },
        );
        storage::update_at(root.path(), |config| {
            *config = original.clone();
            Ok(())
        })
        .unwrap();
        storage::update_at(root.path(), |config| {
            config.preferences.theme = LauncherTheme::Slate;
            config.preferences.auto_check_updates = false;
            Ok(())
        })
        .unwrap();
        let saved = storage::load_or_create_at(root.path()).unwrap();
        assert_eq!(saved.profiles, original.profiles);
        assert_eq!(saved.optional_selections, original.optional_selections);
        assert_eq!(saved.preferences.theme, LauncherTheme::Slate);
        assert!(!saved.preferences.auto_check_updates);
    }
}
