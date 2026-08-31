mod activity;
mod catalog;
#[cfg(feature = "developer")]
mod catalog_publisher;
#[cfg(feature = "developer")]
mod content_editor;
#[cfg(feature = "developer")]
mod content_publisher;
mod detection;
mod launch;
mod manifest;
mod minecraft_setup;
mod models;
#[cfg(feature = "developer")]
mod packager;
#[cfg(feature = "developer")]
mod publisher;
mod readiness;
mod remote;
mod restore_points;
mod safe_launch;
mod safe_path;
mod storage;
mod storage_maintenance;
mod updater;

use activity::{ActivityItem, ActivityKind};
use manifest::FileVerification;
use minecraft_setup::{MinecraftBootstrapArtifact, MinecraftBootstrapRequest};
use models::{BootstrapPayload, DetectedInstall, GameProfile, LaunchOutcome, ReadinessStatus};
#[cfg(feature = "developer")]
use packager::{PackagePreview, PackageRequest, ReleasePublication};
#[cfg(feature = "developer")]
use publisher::{PublisherStatus, RepositoryCreation, RepositoryRequest};
use restore_points::{RestoreOutcome, RestorePointSummary, RestorePreview};
use safe_launch::{SafeLaunchOutcome, SafeLaunchRecovery, SafeLaunchStatus};
use storage_maintenance::{StorageCleanupKind, StorageCleanupOutcome, StorageReport};
use tauri::{AppHandle, Manager};
use updater::{TransactionOutcome, TransactionPreview, TransactionRequest};

#[cfg(feature = "developer")]
use catalog_publisher::{CatalogPreview, CatalogPublication};
#[cfg(feature = "developer")]
use content_editor::ManifestContentInput;
#[cfg(feature = "developer")]
use content_publisher::{ContentReleasePreview, ContentReleasePublication};

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogRefreshOutcome {
    payload: BootstrapPayload,
    summary: catalog::RefreshSummary,
}

#[cfg(feature = "developer")]
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ModpackPublicationOutcome {
    publication: ReleasePublication,
    payload: BootstrapPayload,
}

#[cfg(feature = "developer")]
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ManifestContentSaveOutcome {
    changed: bool,
    payload: BootstrapPayload,
}

#[cfg(feature = "developer")]
fn apply_release_publication(
    config: &mut models::LauncherConfig,
    publication: &ReleasePublication,
) -> Result<(), String> {
    let profile = config
        .profiles
        .iter_mut()
        .find(|profile| profile.id == publication.profile_id)
        .ok_or_else(|| {
            format!(
                "Release {} was published, but its local modpack profile is no longer available",
                publication.tag
            )
        })?;
    profile.required_modpack_version = publication.version.clone();
    profile.manifest_url = publication.manifest_url.clone();
    profile.update_source.clear();
    profile.catalog_visible = true;
    Ok(())
}

fn payload(app: &AppHandle) -> Result<BootstrapPayload, String> {
    let config = storage::load_or_create(app)?;
    #[cfg(not(feature = "developer"))]
    let config = {
        let mut config = config;
        if catalog::apply_cached(app, &mut config)? {
            storage::save(app, &config)?;
        }
        config
    };
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
async fn refresh_public_catalog(app: AppHandle) -> Result<CatalogRefreshOutcome, String> {
    tauri::async_runtime::spawn_blocking(move || {
        activity::track(
            &app,
            "Public modpack catalogue",
            ActivityKind::Catalogue,
            "Checking for catalogue and manifest changes",
            || {
                let summary = catalog::refresh(&app)?;
                Ok(CatalogRefreshOutcome {
                    payload: payload(&app)?,
                    summary,
                })
            },
            |outcome| (true, outcome.summary.message.clone()),
        )
    })
    .await
    .map_err(|error| format!("Public catalogue refresh task failed: {error}"))?
}

#[tauri::command]
fn bootstrap(app: AppHandle) -> Result<BootstrapPayload, String> {
    payload(&app)
}

#[tauri::command]
fn list_activity(app: AppHandle) -> Result<Vec<ActivityItem>, String> {
    activity::recent(&app)
}

#[tauri::command]
fn clear_finished_activity(app: AppHandle) -> Result<Vec<ActivityItem>, String> {
    activity::clear_finished(&app)
}

#[tauri::command]
async fn get_storage_report(app: AppHandle) -> Result<StorageReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        activity::track(
            &app,
            "Launcher storage report",
            ActivityKind::Storage,
            "Measuring launcher-owned and configured modpack storage",
            || storage_maintenance::report(&app),
            |report| {
                (
                    true,
                    format!(
                        "Storage report ready: {} launcher bytes and {} configured modpack bytes",
                        report.launcher_bytes, report.profile_bytes
                    ),
                )
            },
        )
    })
    .await
    .map_err(|error| format!("Storage report task failed: {error}"))?
}

#[tauri::command]
async fn clean_storage(
    app: AppHandle,
    kind: StorageCleanupKind,
    confirmed: bool,
) -> Result<StorageCleanupOutcome, String> {
    if activity::has_active(&app)? {
        return Err("Wait for current launcher activity to finish before cleaning storage".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        activity::track(
            &app,
            "Launcher storage cleanup",
            ActivityKind::Storage,
            "Cleaning the confirmed launcher-owned storage category",
            || storage_maintenance::clean(&app, kind, confirmed),
            |outcome| (outcome.complete, outcome.message.clone()),
        )
    })
    .await
    .map_err(|error| format!("Storage cleanup task failed: {error}"))?
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
    if profile.display_name.trim().is_empty() {
        return Err("A modpack profile requires an id and display name".into());
    }
    validate_profile_id(&profile.id)?;
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
        None => {
            config.selected_profile_id = profile.id.clone();
            config.profiles.push(profile);
        }
    }
    storage::save(&app, &config)?;
    payload(&app)
}

#[cfg(feature = "developer")]
#[tauri::command]
fn save_manifest_content(
    app: AppHandle,
    profile_id: String,
    content: ManifestContentInput,
) -> Result<ManifestContentSaveOutcome, String> {
    let config = storage::load_or_create(&app)?;
    let profile = config
        .profiles
        .iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| "That modpack profile does not exist".to_string())?;
    let changed = content_editor::save_for_profile(&app, profile, content)?;
    Ok(ManifestContentSaveOutcome {
        changed,
        payload: payload(&app)?,
    })
}

fn validate_profile_id(value: &str) -> Result<(), String> {
    let valid = !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        });
    if valid {
        Ok(())
    } else {
        Err("Modpack id must be 1-64 lowercase letters, numbers, underscores or hyphens and start with a letter or number".into())
    }
}

#[cfg(test)]
mod profile_command_tests {
    use super::validate_profile_id;

    #[test]
    fn profile_ids_are_safe_stable_catalogue_keys() {
        assert!(validate_profile_id("minecraft_main").is_ok());
        assert!(validate_profile_id("7-days-pack").is_ok());
        for invalid in ["", "Uppercase", "has spaces", "../escape", "_leading"] {
            assert!(
                validate_profile_id(invalid).is_err(),
                "accepted {invalid:?}"
            );
        }
    }
}

#[cfg(all(test, feature = "developer"))]
mod publication_command_tests {
    use super::*;

    #[test]
    fn successful_release_updates_the_profile_for_catalogue_publication() {
        let mut config = models::LauncherConfig::default();
        config.profiles[0].catalog_visible = false;
        let publication = ReleasePublication {
            profile_id: "minecraft_main".into(),
            version: "2.0.0".into(),
            repository: "owner/repository".into(),
            tag: "v2.0.0".into(),
            manifest_url: "https://github.com/owner/repository/releases/latest/download/minecraft_main-manifest.json".into(),
            url: "https://github.com/owner/repository/releases/tag/v2.0.0".into(),
            message: "published".into(),
        };
        apply_release_publication(&mut config, &publication).unwrap();
        assert_eq!(config.profiles[0].required_modpack_version, "2.0.0");
        assert_eq!(config.profiles[0].manifest_url, publication.manifest_url);
        assert!(config.profiles[0].update_source.is_empty());
        assert!(config.profiles[0].catalog_visible);
    }
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
    let title = format!("{} launcher profile", request.profile_id);
    tauri::async_runtime::spawn_blocking(move || {
        activity::track(
            &app,
            title,
            ActivityKind::Setup,
            "Preparing launcher import",
            || minecraft_setup::prepare(&app, &request),
            |artifact| (true, artifact.message.clone()),
        )
    })
    .await
    .map_err(|error| format!("Minecraft bootstrap task failed: {error}"))?
}

#[tauri::command]
#[cfg(feature = "developer")]
async fn github_publisher_status() -> Result<PublisherStatus, String> {
    tauri::async_runtime::spawn_blocking(publisher::status)
        .await
        .map_err(|error| format!("GitHub preflight task failed: {error}"))
}

#[tauri::command]
#[cfg(feature = "developer")]
async fn create_github_repository(
    app: AppHandle,
    request: RepositoryRequest,
) -> Result<RepositoryCreation, String> {
    let title = format!("GitHub repository {}", request.repository);
    tauri::async_runtime::spawn_blocking(move || {
        activity::track(
            &app,
            title,
            ActivityKind::Publishing,
            "Creating repository",
            || publisher::create_repository(&request),
            |creation| (true, creation.message.clone()),
        )
    })
    .await
    .map_err(|error| format!("GitHub repository task failed: {error}"))?
}

#[tauri::command]
#[cfg(feature = "developer")]
async fn prepare_modpack_release(
    app: AppHandle,
    request: PackageRequest,
) -> Result<PackagePreview, String> {
    let title = format!("{} release", request.profile_id);
    tauri::async_runtime::spawn_blocking(move || {
        activity::track(
            &app,
            title,
            ActivityKind::Verifying,
            "Scanning and packaging the modpack source",
            || packager::prepare(&app, &request),
            |preview| {
                if preview.ready {
                    (
                        true,
                        format!(
                            "Release preview ready with {} files and {} package asset(s)",
                            preview.file_count,
                            preview.assets.len()
                        ),
                    )
                } else {
                    (
                        false,
                        format!(
                            "Release preview blocked by {} safety issue(s)",
                            preview.issues.len()
                        ),
                    )
                }
            },
        )
    })
    .await
    .map_err(|error| format!("Modpack packaging task failed: {error}"))?
}

#[tauri::command]
#[cfg(feature = "developer")]
async fn publish_modpack_release(
    app: AppHandle,
    preview_id: String,
    confirmed: bool,
) -> Result<ModpackPublicationOutcome, String> {
    tauri::async_runtime::spawn_blocking(move || {
        activity::track(
            &app,
            "Modpack GitHub release",
            ActivityKind::Publishing,
            "Publishing reviewed package assets and manifest",
            || {
                let published = packager::publish(&preview_id, confirmed)?;
                let publication = published.publication;
                let mut config = storage::load_or_create(&app)?;
                apply_release_publication(&mut config, &publication)?;
                let profile = config
                    .profiles
                    .iter()
                    .find(|profile| profile.id == publication.profile_id)
                    .cloned()
                    .ok_or_else(|| {
                        format!(
                            "Release {} was published, but its local modpack profile is no longer available",
                            publication.tag
                        )
                    })?;
                manifest::store_published(&app, &profile, &published.manifest_bytes).map_err(
                    |error| {
                        format!(
                            "Release {} was published, but its trusted manifest could not be activated locally: {error}",
                            publication.tag
                        )
                    },
                )?;
                storage::save(&app, &config).map_err(|error| {
                    format!(
                        "Release {} was published, but the local profile could not be updated: {error}",
                        publication.tag
                    )
                })?;
                Ok(ModpackPublicationOutcome {
                    publication,
                    payload: payload(&app)?,
                })
            },
            |outcome| (true, outcome.publication.message.clone()),
        )
    })
    .await
    .map_err(|error| format!("GitHub release task failed: {error}"))?
}

#[tauri::command]
#[cfg(feature = "developer")]
async fn prepare_public_catalog(app: AppHandle) -> Result<CatalogPreview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        activity::track(
            &app,
            "Player public catalogue",
            ActivityKind::Verifying,
            "Preparing public catalogue preview",
            || catalog_publisher::prepare(&app),
            |preview| {
                if preview.ready {
                    (
                        true,
                        format!(
                            "Catalogue preview ready with {} profiles",
                            preview.profiles.len()
                        ),
                    )
                } else {
                    (
                        false,
                        format!("Catalogue blocked by {} issue(s)", preview.issues.len()),
                    )
                }
            },
        )
    })
    .await
    .map_err(|error| format!("Public catalogue preparation task failed: {error}"))?
}

#[tauri::command]
#[cfg(feature = "developer")]
async fn publish_public_catalog(
    app: AppHandle,
    preview_id: String,
    confirmed: bool,
) -> Result<CatalogPublication, String> {
    tauri::async_runtime::spawn_blocking(move || {
        activity::track(
            &app,
            "Player public catalogue",
            ActivityKind::Publishing,
            "Publishing the reviewed catalogue",
            || catalog_publisher::publish(&preview_id, confirmed),
            |publication| (true, publication.message.clone()),
        )
    })
    .await
    .map_err(|error| format!("Public catalogue publication task failed: {error}"))?
}

#[tauri::command]
#[cfg(feature = "developer")]
async fn prepare_manifest_content_release(
    app: AppHandle,
    profile_id: String,
) -> Result<ContentReleasePreview, String> {
    let title = format!("{profile_id} content release");
    tauri::async_runtime::spawn_blocking(move || {
        activity::track(
            &app,
            title,
            ActivityKind::Verifying,
            "Preparing manifest-only release",
            || content_publisher::prepare(&app, &profile_id),
            |preview| {
                if preview.ready {
                    (true, "Content-only release preview ready".into())
                } else {
                    (
                        false,
                        format!(
                            "Content release blocked by {} issue(s)",
                            preview.issues.len()
                        ),
                    )
                }
            },
        )
    })
    .await
    .map_err(|error| format!("Content release preparation task failed: {error}"))?
}

#[tauri::command]
#[cfg(feature = "developer")]
async fn publish_manifest_content_release(
    app: AppHandle,
    preview_id: String,
    confirmed: bool,
) -> Result<ContentReleasePublication, String> {
    tauri::async_runtime::spawn_blocking(move || {
        activity::track(
            &app,
            "Manifest-only GitHub release",
            ActivityKind::Publishing,
            "Publishing the reviewed manifest without package assets",
            || content_publisher::publish(&preview_id, confirmed),
            |publication| (true, publication.message.clone()),
        )
    })
    .await
    .map_err(|error| format!("Content release publication task failed: {error}"))?
}

#[tauri::command]
async fn prepare_modpack_transaction(
    app: AppHandle,
    request: TransactionRequest,
) -> Result<TransactionPreview, String> {
    let activity_kind = match request.kind {
        updater::TransactionKind::Update => ActivityKind::Updating,
        updater::TransactionKind::Repair => ActivityKind::Repairing,
    };
    let title = format!("{} modpack", request.profile_id);
    tauri::async_runtime::spawn_blocking(move || {
        activity::track(
            &app,
            title,
            activity_kind,
            "Downloading, staging and verifying trusted files",
            || updater::prepare(&app, &request),
            |preview| {
                (
                    preview.ready || preview.nothing_to_do,
                    preview.message.clone(),
                )
            },
        )
    })
    .await
    .map_err(|error| format!("Modpack staging task failed: {error}"))?
}

#[tauri::command]
async fn apply_modpack_transaction(
    app: AppHandle,
    preview_id: String,
    confirmed: bool,
) -> Result<TransactionOutcome, String> {
    tauri::async_runtime::spawn_blocking(move || {
        activity::track(
            &app,
            "Modpack update or repair",
            ActivityKind::Updating,
            "Backing up and applying reviewed files",
            || updater::apply(&app, &preview_id, confirmed),
            |outcome| {
                (
                    outcome.success,
                    if outcome.success {
                        outcome.message.clone()
                    } else if !outcome.error.is_empty() {
                        outcome.error.clone()
                    } else {
                        outcome.message.clone()
                    },
                )
            },
        )
    })
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
    let title = format!("{profile_id} restore point");
    tauri::async_runtime::spawn_blocking(move || {
        activity::track(
            &app,
            title,
            ActivityKind::Restoring,
            "Staging and verifying the selected restore point",
            || restore_points::prepare(&app, &profile_id, &backup_id),
            |preview| (preview.ready, preview.message.clone()),
        )
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
        activity::track(
            &app,
            "Modpack restore",
            ActivityKind::Restoring,
            "Backing up current files and restoring the reviewed point",
            || restore_points::apply(&app, &preview_id, confirmed),
            |outcome| {
                (
                    outcome.success,
                    if outcome.success {
                        outcome.message.clone()
                    } else if !outcome.error.is_empty() {
                        outcome.error.clone()
                    } else {
                        outcome.message.clone()
                    },
                )
            },
        )
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
    let title = format!("{profile_id} Safe Launch");
    tauri::async_runtime::spawn_blocking(move || {
        activity::track(
            &app,
            title,
            ActivityKind::Launching,
            "Disabling trusted optional files and starting the game",
            || safe_launch::start(&app, &profile_id, confirmed),
            |outcome| (true, outcome.message.clone()),
        )
    })
    .await
    .map_err(|error| format!("Safe Launch task failed: {error}"))?
}

#[tauri::command]
async fn recover_safe_launch(
    app: AppHandle,
    profile_id: String,
    confirmed: bool,
) -> Result<SafeLaunchRecovery, String> {
    let title = format!("{profile_id} Safe Launch recovery");
    tauri::async_runtime::spawn_blocking(move || {
        activity::track(
            &app,
            title,
            ActivityKind::Restoring,
            "Restoring trusted optional files",
            || safe_launch::recover(&app, &profile_id, confirmed),
            |recovery| (true, recovery.message.clone()),
        )
    })
    .await
    .map_err(|error| format!("Safe Launch recovery task failed: {error}"))?
}

#[tauri::command]
async fn verify_profile_files(
    app: AppHandle,
    profile_id: String,
) -> Result<FileVerification, String> {
    let title = format!("{profile_id} file verification");
    tauri::async_runtime::spawn_blocking(move || {
        activity::track(
            &app,
            title,
            ActivityKind::Verifying,
            "Hashing required modpack files",
            || {
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
                manifest::verify_required_files(&profile, &loaded.manifest)
            },
            |verification| {
                let failures = verification.missing.len()
                    + verification.changed.len()
                    + verification.unsafe_entries.len();
                (
                    failures == 0,
                    if failures == 0 {
                        format!("All {} required files match", verification.checked)
                    } else {
                        format!(
                            "{failures} of {} required files need attention",
                            verification.checked
                        )
                    },
                )
            },
        )
    })
    .await
    .map_err(|error| format!("file verification task failed: {error}"))?
}

#[tauri::command]
fn launch_profile(app: AppHandle, profile_id: String) -> Result<LaunchOutcome, String> {
    let title = format!("{profile_id} launch");
    activity::track(
        &app,
        title,
        ActivityKind::Launching,
        "Checking readiness and starting the configured game",
        || {
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
        },
        |outcome| (true, outcome.message.clone()),
    )
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
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
        });

    #[cfg(feature = "developer")]
    let builder = builder.invoke_handler(tauri::generate_handler![
        bootstrap,
        list_activity,
        clear_finished_activity,
        get_storage_report,
        clean_storage,
        refresh_public_catalog,
        select_profile,
        save_profile,
        save_manifest_content,
        detect_installations,
        prepare_minecraft_bootstrap,
        github_publisher_status,
        create_github_repository,
        prepare_modpack_release,
        publish_modpack_release,
        prepare_public_catalog,
        publish_public_catalog,
        prepare_manifest_content_release,
        publish_manifest_content_release,
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
    ]);

    #[cfg(not(feature = "developer"))]
    let builder = builder.invoke_handler(tauri::generate_handler![
        bootstrap,
        list_activity,
        clear_finished_activity,
        get_storage_report,
        clean_storage,
        refresh_public_catalog,
        select_profile,
        save_profile,
        detect_installations,
        prepare_minecraft_bootstrap,
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
    ]);

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
