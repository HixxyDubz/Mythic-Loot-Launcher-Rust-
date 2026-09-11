//! Developer-only machine-local authoring choices. Never part of the public catalogue.
use crate::{models::GameProfile, packager::PackageRequest, remote, safe_path, storage};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tauri::AppHandle;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishingChoices {
    pub request: PackageRequest,
    pub game_versions: Vec<String>,
}

fn path(root: &Path, profile_id: &str) -> Result<PathBuf, String> {
    crate::validate_profile_id(profile_id)?;
    let path = safe_path::safe_join(root, &format!("publishing-choices/{profile_id}.json"))?;
    safe_path::reject_link_path(&path)?;
    Ok(path)
}

fn read(root: &Path, profile_id: &str) -> Result<Option<PublishingChoices>, String> {
    let path = path(root, profile_id)?;
    match fs::metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("Could not inspect publishing choices: {error}")),
        Ok(meta) if !meta.is_file() || meta.len() > 128 * 1024 => {
            return Err("Saved publishing choices are not a valid small local file".into());
        }
        Ok(_) => {}
    }
    let choices: PublishingChoices = serde_json::from_slice(
        &fs::read(&path).map_err(|error| error.to_string())?,
    )
    .map_err(|error| {
        format!("Saved publishing choices are damaged and were left unchanged: {error}")
    })?;
    if choices.request.profile_id != profile_id {
        return Err("Saved publishing choices belong to a different profile".into());
    }
    Ok(Some(choices))
}

#[tauri::command]
pub fn load_publishing_choices(
    app: AppHandle,
    profile_id: String,
) -> Result<PublishingChoices, String> {
    let _work = crate::operations::WorkGuard::begin()?;
    let config = storage::load_or_create(&app)?;
    let profile = config
        .profiles
        .iter()
        .find(|p| p.id == profile_id)
        .ok_or("That modpack profile does not exist")?;
    if let Some(saved) = read(&storage::data_dir(&app)?, &profile_id)? {
        return Ok(saved);
    }
    let loaded = crate::content_editor::load_authoring(&app, profile);
    let repository = profile
        .manifest_url
        .strip_prefix("https://github.com/")
        .map(|tail| tail.split('/').take(2).collect::<Vec<_>>().join("/"))
        .unwrap_or_default();
    Ok(PublishingChoices {
        request: PackageRequest {
            profile_id,
            source_dir: profile.install_dir.clone(),
            version: profile.required_modpack_version.clone(),
            game_version: profile.required_game_version.clone(),
            minecraft_mod_loader: loaded
                .manifest
                .minecraft_base_mod_loader
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .into(),
            release_date: time::OffsetDateTime::now_utc().date().to_string(),
            repository,
            release_notes: format!("Release {}", profile.required_modpack_version),
        },
        game_versions: vec![profile.required_game_version.clone()]
            .into_iter()
            .filter(|v| !v.is_empty())
            .collect(),
    })
}

#[tauri::command]
pub fn save_publishing_choices(
    app: AppHandle,
    request: PackageRequest,
) -> Result<PublishingChoices, String> {
    let _operation = crate::operations::MaintenanceGuard::acquire()?;
    save(&app, request)
}

pub(crate) fn save(app: &AppHandle, request: PackageRequest) -> Result<PublishingChoices, String> {
    let config = storage::load_or_create(app)?;
    let profile = config
        .profiles
        .iter()
        .find(|p| p.id == request.profile_id)
        .ok_or("That modpack profile does not exist")?;
    save_at(&storage::data_dir(app)?, profile, request)
}

fn save_at(
    root: &Path,
    profile: &GameProfile,
    request: PackageRequest,
) -> Result<PublishingChoices, String> {
    crate::packager::validate_request(profile, &request)?;
    let previous = read(root, &request.profile_id)?;
    let mut game_versions = vec![request.game_version.trim().to_string()];
    for version in previous
        .into_iter()
        .flat_map(|p| p.game_versions)
        .chain(std::iter::once(profile.required_game_version.clone()))
    {
        if !version.is_empty() && !game_versions.contains(&version) && game_versions.len() < 24 {
            game_versions.push(version);
        }
    }
    let choices = PublishingChoices {
        request,
        game_versions,
    };
    let bytes = serde_json::to_vec_pretty(&choices).map_err(|error| error.to_string())?;
    remote::write_atomic(&path(root, &profile.id)?, &bytes)?;
    Ok(choices)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn choices_are_per_profile_local_and_keep_version_history() {
        let root = tempfile::TempDir::new().unwrap();
        let mut profile = crate::models::LauncherConfig::default().profiles.remove(1);
        let request = PackageRequest {
            profile_id: profile.id.clone(),
            source_dir: "C:\\Private\\Mods".into(),
            version: "2.0".into(),
            game_version: "3.1 (b8)".into(),
            minecraft_mod_loader: String::new(),
            release_date: "2026-09-10".into(),
            repository: "owner/pack".into(),
            release_notes: "Balance changes".into(),
        };
        save_at(root.path(), &profile, request.clone()).unwrap();
        let mut next = request;
        next.game_version = "3.2".into();
        next.source_dir = "D:\\New Mods".into();
        save_at(root.path(), &profile, next).unwrap();
        let loaded = read(root.path(), &profile.id).unwrap().unwrap();
        assert_eq!(loaded.request.source_dir, "D:\\New Mods");
        assert_eq!(&loaded.game_versions[..2], &["3.2", "3.1 (b8)"]);
        assert!(!root.path().join(&profile.manifest_path).exists());
        profile.id = "different_pack".into();
        assert!(read(root.path(), &profile.id).unwrap().is_none());
    }

    #[test]
    fn malformed_choices_fail_closed_and_traversal_is_refused() {
        let root = tempfile::TempDir::new().unwrap();
        assert!(path(root.path(), "../escape").is_err());
        let path = path(root.path(), "fixture").unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"broken").unwrap();
        assert!(
            read(root.path(), "fixture")
                .unwrap_err()
                .contains("left unchanged")
        );
        assert_eq!(fs::read(path).unwrap(), b"broken");
    }
}
