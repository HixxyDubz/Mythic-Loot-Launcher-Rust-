mod detection;
mod launch;
mod manifest;
mod minecraft_setup;
mod models;
mod packager;
mod publisher;
mod readiness;
mod restore_points;
mod safe_launch;
mod safe_path;
mod storage;
mod updater;

use manifest::FileVerification;
use minecraft_setup::{MinecraftBootstrapArtifact, MinecraftBootstrapRequest};
use models::{BootstrapPayload, DetectedInstall, GameProfile, LaunchOutcome, ReadinessStatus};
use packager::{PackagePreview, PackageRequest, ReleasePublication};
use publisher::{PublisherStatus, RepositoryCreation, RepositoryRequest};
use restore_points::{RestoreOutcome, RestorePointSummary, RestorePreview};
use safe_launch::{SafeLaunchOutcome, SafeLaunchRecovery, SafeLaunchStatus};
use tauri::{AppHandle, Manager};
use updater::{TransactionOutcome, TransactionPreview, TransactionRequest};

fn payload(app: &AppHandle) -> Result<BootstrapPayload, String> {
    let config = storage::load_or_create(app)?;
    let loaded: Vec<_> = config
        .profiles
        .iter()
        .map(|profile| manifest::load_for_profile(app, profile))
        .collect();
    let manifests: Vec<_> = loaded.iter().map(|loaded| loaded.summary.clone()).collect();
    let health = config
        .profiles
        .iter()
        .zip(manifests.iter())
        .map(|(profile, manifest)| readiness::assess(profile, Some(manifest)))
        .collect();
    Ok(BootstrapPayload {
        config,
        games: models::built_in_games(),
        health,
        manifests,
        data_dir: storage::data_dir(app)?.display().to_string(),
    })
}

#[tauri::command]
fn bootstrap(app: AppHandle) -> Result<BootstrapPayload, String> {
    payload(&app)
}

#[tauri::command]
fn select_profile(app: AppHandle, profile_id: String) -> Result<BootstrapPayload, String> {
    let mut config = storage::load_or_create(&app)?;
    if !config
        .profiles
        .iter()
        .any(|profile| profile.id == profile_id)
    {
        return Err("That modpack profile does not exist".into());
    }
    config.selected_profile_id = profile_id;
    storage::save(&app, &config)?;
    payload(&app)
}

#[tauri::command]
fn save_profile(app: AppHandle, profile: GameProfile) -> Result<BootstrapPayload, String> {
    if profile.id.trim().is_empty() || profile.display_name.trim().is_empty() {
        return Err("A modpack profile requires an id and display name".into());
    }
    if manifest::is_discord_invite(&profile.update_source)
        || manifest::is_discord_invite(&profile.manifest_url)
    {
        return Err("Discord invitations cannot be used as update or manifest sources".into());
    }
    let mut config = storage::load_or_create(&app)?;
    match config
        .profiles
        .iter_mut()
        .find(|existing| existing.id == profile.id)
    {
        Some(existing) => *existing = profile,
        None => config.profiles.push(profile),
    }
    storage::save(&app, &config)?;
    payload(&app)
}

#[tauri::command]
fn detect_installations(profile: GameProfile) -> Vec<DetectedInstall> {
    detection::detect(&profile)
}

#[tauri::command]
async fn prepare_minecraft_bootstrap(
    app: AppHandle,
    request: MinecraftBootstrapRequest,
) -> Result<MinecraftBootstrapArtifact, String> {
    tauri::async_runtime::spawn_blocking(move || minecraft_setup::prepare(&app, &request))
        .await
        .map_err(|error| format!("Minecraft bootstrap task failed: {error}"))?
}

#[tauri::command]
async fn github_publisher_status() -> Result<PublisherStatus, String> {
    tauri::async_runtime::spawn_blocking(publisher::status)
        .await
        .map_err(|error| format!("GitHub preflight task failed: {error}"))
}

#[tauri::command]
async fn create_github_repository(
    request: RepositoryRequest,
) -> Result<RepositoryCreation, String> {
    tauri::async_runtime::spawn_blocking(move || publisher::create_repository(&request))
        .await
        .map_err(|error| format!("GitHub repository task failed: {error}"))?
}

#[tauri::command]
async fn prepare_modpack_release(
    app: AppHandle,
    request: PackageRequest,
) -> Result<PackagePreview, String> {
    tauri::async_runtime::spawn_blocking(move || packager::prepare(&app, &request))
        .await
        .map_err(|error| format!("Modpack packaging task failed: {error}"))?
}

#[tauri::command]
async fn publish_modpack_release(
    preview_id: String,
    confirmed: bool,
) -> Result<ReleasePublication, String> {
    tauri::async_runtime::spawn_blocking(move || packager::publish(&preview_id, confirmed))
        .await
        .map_err(|error| format!("GitHub release task failed: {error}"))?
}

#[tauri::command]
async fn prepare_modpack_transaction(
    app: AppHandle,
    request: TransactionRequest,
) -> Result<TransactionPreview, String> {
    tauri::async_runtime::spawn_blocking(move || updater::prepare(&app, &request))
        .await
        .map_err(|error| format!("Modpack staging task failed: {error}"))?
}

#[tauri::command]
async fn apply_modpack_transaction(
    app: AppHandle,
    preview_id: String,
    confirmed: bool,
) -> Result<TransactionOutcome, String> {
    tauri::async_runtime::spawn_blocking(move || updater::apply(&app, &preview_id, confirmed))
        .await
        .map_err(|error| format!("Modpack transaction task failed: {error}"))?
}

#[tauri::command]
fn list_restore_points(
    app: AppHandle,
    profile_id: String,
) -> Result<Vec<RestorePointSummary>, String> {
    restore_points::list(&app, &profile_id)
}

#[tauri::command]
async fn prepare_restore_point(
    app: AppHandle,
    profile_id: String,
    backup_id: String,
) -> Result<RestorePreview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        restore_points::prepare(&app, &profile_id, &backup_id)
    })
    .await
    .map_err(|error| format!("Restore staging task failed: {error}"))?
}

#[tauri::command]
async fn apply_restore_point(
    app: AppHandle,
    preview_id: String,
    confirmed: bool,
) -> Result<RestoreOutcome, String> {
    tauri::async_runtime::spawn_blocking(move || {
        restore_points::apply(&app, &preview_id, confirmed)
    })
    .await
    .map_err(|error| format!("Restore task failed: {error}"))?
}

#[tauri::command]
fn delete_restore_point(
    app: AppHandle,
    profile_id: String,
    backup_id: String,
    confirmed: bool,
) -> Result<String, String> {
    restore_points::delete(&app, &profile_id, &backup_id, confirmed)
}

#[tauri::command]
fn safe_launch_status(app: AppHandle, profile_id: String) -> Result<SafeLaunchStatus, String> {
    safe_launch::status(&app, &profile_id)
}

#[tauri::command]
async fn start_safe_launch(
    app: AppHandle,
    profile_id: String,
    confirmed: bool,
) -> Result<SafeLaunchOutcome, String> {
    tauri::async_runtime::spawn_blocking(move || safe_launch::start(&app, &profile_id, confirmed))
        .await
        .map_err(|error| format!("Safe Launch task failed: {error}"))?
}

#[tauri::command]
async fn recover_safe_launch(
    app: AppHandle,
    profile_id: String,
    confirmed: bool,
) -> Result<SafeLaunchRecovery, String> {
    tauri::async_runtime::spawn_blocking(move || safe_launch::recover(&app, &profile_id, confirmed))
        .await
        .map_err(|error| format!("Safe Launch recovery task failed: {error}"))?
}

#[tauri::command]
async fn verify_profile_files(
    app: AppHandle,
    profile_id: String,
) -> Result<FileVerification, String> {
    let config = storage::load_or_create(&app)?;
    let profile = config
        .profiles
        .into_iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| "That modpack profile does not exist".to_string())?;
    let loaded = manifest::load_for_profile(&app, &profile);
    if !loaded.summary.valid {
        return Err(loaded.summary.errors.join("; "));
    }
    tauri::async_runtime::spawn_blocking(move || {
        manifest::verify_required_files(&profile, &loaded.manifest)
    })
    .await
    .map_err(|error| format!("file verification task failed: {error}"))?
}

#[tauri::command]
fn launch_profile(app: AppHandle, profile_id: String) -> Result<LaunchOutcome, String> {
    let config = storage::load_or_create(&app)?;
    let profile = config
        .profiles
        .iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| "That modpack profile does not exist".to_string())?;
    let loaded = manifest::load_for_profile(&app, profile);
    let health = readiness::assess(profile, Some(&loaded.summary));
    if health.status != ReadinessStatus::Ready {
        return Err(format!(
            "{} is not ready: {}",
            profile.display_name, health.headline
        ));
    }
    launch::launch(profile)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let window = app
                .get_webview_window("main")
                .ok_or("the main launcher window was not created")?;
            window.show()?;
            #[cfg(debug_assertions)]
            eprintln!(
                "Mythic Loot main window initialized (visible={})",
                window.is_visible().unwrap_or(false)
            );
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            select_profile,
            save_profile,
            detect_installations,
            prepare_minecraft_bootstrap,
            github_publisher_status,
            create_github_repository,
            prepare_modpack_release,
            publish_modpack_release,
            prepare_modpack_transaction,
            apply_modpack_transaction,
            list_restore_points,
            prepare_restore_point,
            apply_restore_point,
            delete_restore_point,
            safe_launch_status,
            start_safe_launch,
            recover_safe_launch,
            verify_profile_files,
            launch_profile
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
