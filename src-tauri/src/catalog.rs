use std::{collections::HashSet, env, path::PathBuf};

#[cfg(not(feature = "developer"))]
use std::fs;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::{manifest, remote, safe_path, storage};

#[cfg(any(not(feature = "developer"), test))]
use crate::models::{GameProfile, LauncherConfig};

#[cfg(feature = "developer")]
pub const CATALOG_REPOSITORY: &str = "HixxyDubz/Mythic-Loot-Launcher-Rust-";
#[cfg(feature = "developer")]
pub const CATALOG_BRANCH: &str = "main";
#[cfg(feature = "developer")]
pub const CATALOG_FILE_PATH: &str = "launcher-catalog.json";
pub const DEFAULT_CATALOG_URL: &str = "https://raw.githubusercontent.com/HixxyDubz/Mythic-Loot-Launcher-Rust-/main/launcher-catalog.json";
const MAX_CATALOG_BYTES: usize = 4 * 1024 * 1024;
const CATALOG_PATH: &str = "catalog/launcher-catalog.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PublicCatalog {
    pub catalog_version: String,
    pub generated_at: String,
    pub profiles: Vec<CatalogProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogProfile {
    pub id: String,
    pub game: String,
    pub display_name: String,
    pub required_game_version: String,
    pub required_modpack_version: String,
    pub manifest_url: String,
    pub deployment_subdir: String,
    pub logo_url: String,
    pub discord_invite: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshSummary {
    pub catalog_changed: bool,
    pub manifests_changed: usize,
    pub manifests_checked: usize,
    pub online: bool,
    pub message: String,
}

#[cfg(not(feature = "developer"))]
pub fn apply_cached(app: &AppHandle, config: &mut LauncherConfig) -> Result<bool, String> {
    let path = cache_path(app)?;
    if !path.is_file() {
        return Ok(false);
    }
    let bytes = fs::read(&path)
        .map_err(|error| format!("Could not read cached public catalogue: {error}"))?;
    let catalog = match parse(&bytes) {
        Ok(catalog) => catalog,
        Err(_) => {
            let rejected = path.with_extension(format!(
                "rejected-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|value| value.as_secs())
                    .unwrap_or_default()
            ));
            fs::rename(&path, rejected).ok();
            return Ok(false);
        }
    };
    Ok(merge(config, &catalog) > 0)
}

pub fn refresh(app: &AppHandle) -> Result<RefreshSummary, String> {
    let mut notes = Vec::new();
    let mut online = false;
    let mut catalog_changed = false;
    let url = env::var("MYTHIC_LOOT_CATALOG_URL").unwrap_or_else(|_| DEFAULT_CATALOG_URL.into());

    match remote::fetch_https(&url, MAX_CATALOG_BYTES).and_then(|bytes| {
        let catalog = parse(&bytes)?;
        let cache_changed = remote::write_atomic(&cache_path(app)?, &bytes)?;
        #[cfg(not(feature = "developer"))]
        let merged = {
            let mut config = storage::load_or_create(app)?;
            let merged = merge(&mut config, &catalog);
            if merged > 0 {
                storage::save(app, &config)?;
            }
            merged
        };
        #[cfg(feature = "developer")]
        let merged = {
            drop(catalog);
            0
        };
        Ok(cache_changed || merged > 0)
    }) {
        Ok(changed) => {
            online = true;
            catalog_changed = changed;
            notes.push(
                if changed {
                    "Public catalogue refreshed"
                } else {
                    "Public catalogue is current"
                }
                .to_string(),
            );
        }
        Err(error) => notes.push(format!(
            "Catalogue unavailable; using verified local data ({error})"
        )),
    }

    let config = storage::load_or_create(app)?;
    let mut manifests_checked = 0;
    let mut manifests_changed = 0;
    let mut manifest_failures = 0;
    for profile in &config.profiles {
        if profile.manifest_url.trim().is_empty() {
            continue;
        }
        manifests_checked += 1;
        match manifest::refresh_remote(app, profile) {
            Ok(changed) => {
                online = true;
                manifests_changed += usize::from(changed);
            }
            Err(_) => manifest_failures += 1,
        }
    }
    if manifests_checked > 0 {
        notes.push(format!(
            "{} manifest{} checked, {} changed{}",
            manifests_checked,
            if manifests_checked == 1 { "" } else { "s" },
            manifests_changed,
            if manifest_failures > 0 {
                format!(", {manifest_failures} unavailable")
            } else {
                String::new()
            },
        ));
    }
    Ok(RefreshSummary {
        catalog_changed,
        manifests_changed,
        manifests_checked,
        online,
        message: notes.join(". "),
    })
}

fn cache_path(app: &AppHandle) -> Result<PathBuf, String> {
    safe_path::safe_join(&storage::data_dir(app)?, CATALOG_PATH)
}

pub fn parse(bytes: &[u8]) -> Result<PublicCatalog, String> {
    let catalog: PublicCatalog = serde_json::from_slice(bytes)
        .map_err(|error| format!("Public catalogue JSON is invalid: {error}"))?;
    validate(&catalog)?;
    Ok(catalog)
}

pub fn validate(catalog: &PublicCatalog) -> Result<(), String> {
    if catalog.catalog_version.split('.').next() != Some("1") {
        return Err(format!(
            "Unsupported public catalogue version {}",
            catalog.catalog_version
        ));
    }
    if catalog.generated_at.trim().is_empty() {
        return Err("Public catalogue requires generatedAt".into());
    }
    let mut ids = HashSet::new();
    for profile in &catalog.profiles {
        if !valid_id(&profile.id) || !valid_id(&profile.game) {
            return Err(format!(
                "Public catalogue contains an unsafe profile or game id: {}",
                profile.id
            ));
        }
        if !ids.insert(profile.id.to_ascii_lowercase()) {
            return Err(format!(
                "Public catalogue contains duplicate profile id: {}",
                profile.id
            ));
        }
        if profile.display_name.trim().is_empty() || profile.display_name.len() > 120 {
            return Err(format!(
                "Public catalogue profile {} has an invalid display name",
                profile.id
            ));
        }
        if profile.required_modpack_version.trim().is_empty() {
            return Err(format!(
                "Public catalogue profile {} has no required modpack version",
                profile.id
            ));
        }
        validate_https(&profile.manifest_url, "manifestUrl", &profile.id, false)?;
        if !profile.logo_url.is_empty() && !profile.logo_url.starts_with("/assets/") {
            validate_https(&profile.logo_url, "logoUrl", &profile.id, false)?;
        }
        if !profile.discord_invite.is_empty() {
            validate_https(&profile.discord_invite, "discordInvite", &profile.id, true)?;
        }
        if !profile.deployment_subdir.is_empty() {
            safe_path::normalize_relative(&profile.deployment_subdir).map_err(|error| {
                format!(
                    "Profile {} has unsafe deploymentSubdir: {error}",
                    profile.id
                )
            })?;
        }
    }
    Ok(())
}

#[cfg(any(not(feature = "developer"), test))]
pub fn merge(config: &mut LauncherConfig, catalog: &PublicCatalog) -> usize {
    let mut changed = 0;
    for published in &catalog.profiles {
        if let Some(existing) = config
            .profiles
            .iter_mut()
            .find(|profile| profile.id == published.id)
        {
            let before = existing.clone();
            apply_public(existing, published);
            changed += usize::from(*existing != before);
        } else {
            config.profiles.push(new_profile(published));
            changed += 1;
        }
    }
    changed
}

#[cfg(any(not(feature = "developer"), test))]
fn apply_public(profile: &mut GameProfile, published: &CatalogProfile) {
    profile.game.clone_from(&published.game);
    profile.display_name.clone_from(&published.display_name);
    profile
        .required_game_version
        .clone_from(&published.required_game_version);
    profile
        .required_modpack_version
        .clone_from(&published.required_modpack_version);
    profile.manifest_url.clone_from(&published.manifest_url);
    profile
        .deployment_subdir
        .clone_from(&published.deployment_subdir);
    profile.logo_path.clone_from(&published.logo_url);
    profile.discord_invite.clone_from(&published.discord_invite);
    profile.update_source.clear();
}

#[cfg(any(not(feature = "developer"), test))]
fn new_profile(published: &CatalogProfile) -> GameProfile {
    let mut profile = GameProfile {
        id: published.id.clone(),
        game: String::new(),
        display_name: String::new(),
        required_game_version: String::new(),
        required_modpack_version: String::new(),
        local_modpack_version: String::new(),
        manifest_path: format!("manifests/{}.json", published.id),
        install_dir: String::new(),
        game_dir: String::new(),
        game_exe_path: String::new(),
        launch_args: String::new(),
        minecraft_launcher: String::new(),
        discord_invite: String::new(),
        update_source: String::new(),
        manifest_url: String::new(),
        deployment_subdir: String::new(),
        logo_path: String::new(),
        catalog_visible: true,
    };
    apply_public(&mut profile, published);
    profile
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}

fn validate_https(value: &str, field: &str, profile: &str, discord: bool) -> Result<(), String> {
    if !value.starts_with("https://") || value.chars().any(char::is_whitespace) {
        return Err(format!("Profile {profile} has invalid HTTPS {field}"));
    }
    if discord && !manifest::is_discord_invite(value) {
        return Err(format!("Profile {profile} has a non-Discord discordInvite"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> PublicCatalog {
        PublicCatalog {
            catalog_version: "1.0".into(),
            generated_at: "2026-08-29T20:00:00Z".into(),
            profiles: vec![CatalogProfile {
                id: "new_pack".into(),
                game: "minecraft".into(),
                display_name: "New Pack".into(),
                required_game_version: "1.21.1".into(),
                required_modpack_version: "1.0.0".into(),
                manifest_url: "https://github.com/example/new-pack/releases/latest/download/new_pack-manifest.json".into(),
                deployment_subdir: String::new(),
                logo_url: "/assets/minecraft.png".into(),
                discord_invite: String::new(),
            }],
        }
    }

    #[test]
    fn catalogue_merge_adds_public_data_without_machine_paths() {
        let mut config = LauncherConfig::default();
        assert_eq!(merge(&mut config, &catalog()), 1);
        let profile = config
            .profiles
            .iter()
            .find(|profile| profile.id == "new_pack")
            .unwrap();
        assert_eq!(profile.display_name, "New Pack");
        assert!(profile.install_dir.is_empty());
        assert!(profile.game_exe_path.is_empty());
        assert!(profile.local_modpack_version.is_empty());
    }

    #[test]
    fn catalogue_merge_preserves_player_local_state() {
        let mut config = LauncherConfig::default();
        let mut published = catalog();
        published.profiles[0].id = "minecraft_main".into();
        published.profiles[0].display_name = "Published Minecraft".into();
        config.profiles[0].install_dir = "C:\\Player\\Minecraft".into();
        config.profiles[0].game_exe_path = "C:\\Launcher.exe".into();
        config.profiles[0].local_modpack_version = "0.9.0".into();
        assert_eq!(merge(&mut config, &published), 1);
        assert_eq!(config.profiles[0].display_name, "Published Minecraft");
        assert_eq!(config.profiles[0].install_dir, "C:\\Player\\Minecraft");
        assert_eq!(config.profiles[0].game_exe_path, "C:\\Launcher.exe");
        assert_eq!(config.profiles[0].local_modpack_version, "0.9.0");
    }

    #[test]
    fn catalogue_rejects_duplicate_ids_unsafe_paths_and_non_https_urls() {
        let mut value = catalog();
        value.profiles.push(value.profiles[0].clone());
        assert!(validate(&value).unwrap_err().contains("duplicate"));
        let mut value = catalog();
        value.profiles[0].deployment_subdir = "../escape".into();
        assert!(validate(&value).unwrap_err().contains("deploymentSubdir"));
        let mut value = catalog();
        value.profiles[0].manifest_url = "http://example.invalid/manifest.json".into();
        assert!(validate(&value).unwrap_err().contains("HTTPS"));
    }

    #[test]
    fn checked_in_player_catalogue_passes_the_public_contract() {
        let catalog = parse(include_bytes!("../../launcher-catalog.json")).unwrap();
        assert_eq!(catalog.profiles.len(), 2);
        assert_eq!(catalog.profiles[0].id, "minecraft_main");
        assert_eq!(catalog.profiles[1].id, "seven_days_main");
    }
}
