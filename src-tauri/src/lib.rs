mod detection;
mod launch;
mod manifest;
mod models;
mod readiness;
mod safe_path;
mod server_status;
mod storage;

use manifest::FileVerification;
use models::{BootstrapPayload, DetectedInstall, GameProfile, LaunchOutcome, ReadinessStatus};
use server_status::ServerStatus;
use tauri::{AppHandle, Manager};

fn payload(app: &AppHandle) -> Result<BootstrapPayload, String> {
    let config = storage::load_or_create(app)?;
    let loaded: Vec<_> = config
        .profiles
        .iter()
        .map(|profile| manifest::load_for_profile(app, profile))
        .collect();
    let manifests: Vec<_> = loaded.iter().map(|loaded| loaded.summary.clone()).collect();
    let servers: Vec<_> = config
        .profiles
        .iter()
        .map(ServerStatus::not_checked)
        .collect();
    let health = config
        .profiles
        .iter()
        .zip(manifests.iter())
        .zip(servers.iter())
        .map(|((profile, manifest), server)| {
            readiness::assess(profile, Some(manifest), Some(server))
        })
        .collect();
    Ok(BootstrapPayload {
        config,
        games: models::built_in_games(),
        health,
        manifests,
        servers,
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
        return Err("That server profile does not exist".into());
    }
    config.selected_profile_id = profile_id;
    storage::save(&app, &config)?;
    payload(&app)
}

#[tauri::command]
fn save_profile(app: AppHandle, profile: GameProfile) -> Result<BootstrapPayload, String> {
    if profile.id.trim().is_empty() || profile.display_name.trim().is_empty() {
        return Err("A server profile requires an id and display name".into());
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
async fn refresh_server_status(
    profile: GameProfile,
    use_cache: bool,
) -> Result<ServerStatus, String> {
    tauri::async_runtime::spawn_blocking(move || server_status::query(&profile, use_cache))
        .await
        .map_err(|error| format!("server status task failed: {error}"))
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
        .ok_or_else(|| "That server profile does not exist".to_string())?;
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
        .ok_or_else(|| "That server profile does not exist".to_string())?;
    let loaded = manifest::load_for_profile(&app, profile);
    let health = readiness::assess(profile, Some(&loaded.summary), None);
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
            refresh_server_status,
            verify_profile_files,
            launch_profile
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
