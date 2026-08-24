mod detection;
mod launch;
mod models;
mod readiness;
mod storage;

use models::{BootstrapPayload, DetectedInstall, GameProfile, LaunchOutcome, ReadinessStatus};
use tauri::{AppHandle, Manager};

fn payload(app: &AppHandle) -> Result<BootstrapPayload, String> {
    let config = storage::load_or_create(app)?;
    let health = config.profiles.iter().map(readiness::assess).collect();
    Ok(BootstrapPayload {
        config,
        games: models::built_in_games(),
        health,
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
fn launch_profile(app: AppHandle, profile_id: String) -> Result<LaunchOutcome, String> {
    let config = storage::load_or_create(&app)?;
    let profile = config
        .profiles
        .iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| "That server profile does not exist".to_string())?;
    let health = readiness::assess(profile);
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
            launch_profile
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
