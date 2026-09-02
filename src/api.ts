import { invoke, isTauri } from "@tauri-apps/api/core";
import type {
  ActivityItem,
  AppReleasePreview,
  AppReleasePublication,
  AppReleaseRequest,
  AppUpdateApplyOutcome,
  AppUpdatePreview,
  AppUpdateResult,
  AppUpdateStage,
  BootstrapPayload,
  CatalogPreview,
  CatalogPublication,
  CatalogRefreshOutcome,
  ContentReleasePreview,
  ContentReleasePublication,
  DetectedInstall,
  FileVerification,
  GameProfile,
  LaunchOutcome,
  ManifestContentInput,
  ManifestContentSaveOutcome,
  MinecraftBootstrapArtifact,
  MinecraftBootstrapRequest,
  ModpackPublicationOutcome,
  PackagePreview,
  PackageRequest,
  PublisherStatus,
  RepositoryCreation,
  RepositoryRequest,
  RestoreOutcome,
  RestorePointSummary,
  RestorePreview,
  SafeLaunchOutcome,
  SafeLaunchRecovery,
  SafeLaunchStatus,
  StorageCleanupKind,
  StorageCleanupOutcome,
  StorageReport,
  SupportBundleOutcome,
  SupportPreview,
  TransactionOutcome,
  TransactionPreview,
  TransactionRequest,
} from "./types";

const runningInTauri = isTauri();

function requireNative(operation: string): void {
  if (!runningInTauri) {
    throw new Error(`${operation} requires the native Mythic Loot Launcher desktop application.`);
  }
}

export async function bootstrap(): Promise<BootstrapPayload> {
  requireNative("Launcher startup");
  return invoke<BootstrapPayload>("bootstrap");
}

export async function listActivity(): Promise<ActivityItem[]> {
  requireNative("Activity history");
  return invoke<ActivityItem[]>("list_activity");
}

export async function clearFinishedActivity(): Promise<ActivityItem[]> {
  requireNative("Activity history cleanup");
  return invoke<ActivityItem[]>("clear_finished_activity");
}

export async function getStorageReport(): Promise<StorageReport> {
  requireNative("Storage report");
  return invoke<StorageReport>("get_storage_report");
}

export async function cleanStorage(
  kind: StorageCleanupKind,
  confirmed: boolean,
): Promise<StorageCleanupOutcome> {
  requireNative("Storage cleanup");
  return invoke<StorageCleanupOutcome>("clean_storage", { kind, confirmed });
}

export async function prepareSupportBundle(profileId: string): Promise<SupportPreview> {
  requireNative("Support bundle review");
  return invoke<SupportPreview>("prepare_support_bundle", { profileId });
}

export async function createSupportBundle(
  previewId: string,
  confirmed: boolean,
): Promise<SupportBundleOutcome> {
  requireNative("Support bundle export");
  return invoke<SupportBundleOutcome>("create_support_bundle", { previewId, confirmed });
}

export async function checkAppUpdate(): Promise<AppUpdatePreview> {
  requireNative("App update check");
  return invoke<AppUpdatePreview>("check_app_update");
}

export async function getAppUpdateResult(): Promise<AppUpdateResult | null> {
  requireNative("App update result");
  return invoke<AppUpdateResult | null>("app_update_result");
}

export async function prepareAppUpdate(previewId: string): Promise<AppUpdateStage> {
  requireNative("Player app update download");
  return invoke<AppUpdateStage>("prepare_app_update", { previewId });
}

export async function applyAppUpdate(
  stageId: string,
  confirmed: boolean,
): Promise<AppUpdateApplyOutcome> {
  requireNative("Player app update installation");
  return invoke<AppUpdateApplyOutcome>("apply_app_update", { stageId, confirmed });
}

export async function preparePlayerAppRelease(request: AppReleaseRequest): Promise<AppReleasePreview> {
  requireNative("Player app release preparation");
  return invoke<AppReleasePreview>("prepare_player_app_release", { request });
}

export async function publishPlayerAppRelease(
  previewId: string,
  confirmed: boolean,
): Promise<AppReleasePublication> {
  requireNative("Player app release publication");
  return invoke<AppReleasePublication>("publish_player_app_release", { previewId, confirmed });
}

export async function refreshPublicCatalog(): Promise<CatalogRefreshOutcome> {
  requireNative("Public catalogue refresh");
  return invoke<CatalogRefreshOutcome>("refresh_public_catalog");
}

export async function selectProfile(profileId: string): Promise<BootstrapPayload> {
  requireNative("Profile selection");
  return invoke<BootstrapPayload>("select_profile", { profileId });
}

export async function saveProfile(profile: GameProfile): Promise<BootstrapPayload> {
  requireNative("Profile saving");
  return invoke<BootstrapPayload>("save_profile", { profile });
}

export async function saveManifestContent(
  profileId: string,
  content: ManifestContentInput,
): Promise<ManifestContentSaveOutcome> {
  requireNative("Manifest content saving");
  return invoke<ManifestContentSaveOutcome>("save_manifest_content", { profileId, content });
}

export async function prepareManifestContentRelease(profileId: string): Promise<ContentReleasePreview> {
  requireNative("Content-only release preparation");
  return invoke<ContentReleasePreview>("prepare_manifest_content_release", { profileId });
}

export async function publishManifestContentRelease(
  previewId: string,
  confirmed: boolean,
): Promise<ContentReleasePublication> {
  requireNative("Content-only release publication");
  return invoke<ContentReleasePublication>("publish_manifest_content_release", { previewId, confirmed });
}

export async function detectInstallations(profile: GameProfile): Promise<DetectedInstall[]> {
  requireNative("Game detection");
  return invoke<DetectedInstall[]>("detect_installations", { profile });
}

export async function prepareMinecraftBootstrap(
  request: MinecraftBootstrapRequest,
): Promise<MinecraftBootstrapArtifact> {
  requireNative("Minecraft launcher bootstrap creation");
  return invoke<MinecraftBootstrapArtifact>("prepare_minecraft_bootstrap", { request });
}

export async function verifyProfileFiles(profileId: string): Promise<FileVerification> {
  requireNative("File verification");
  return invoke<FileVerification>("verify_profile_files", { profileId });
}

export async function launchProfile(profileId: string): Promise<LaunchOutcome> {
  requireNative("Game launch");
  return invoke<LaunchOutcome>("launch_profile", { profileId });
}

export async function githubPublisherStatus(): Promise<PublisherStatus> {
  requireNative("GitHub publisher preflight");
  return invoke<PublisherStatus>("github_publisher_status");
}

export async function createGithubRepository(request: RepositoryRequest): Promise<RepositoryCreation> {
  requireNative("Repository creation");
  return invoke<RepositoryCreation>("create_github_repository", { request });
}

export async function prepareModpackRelease(request: PackageRequest): Promise<PackagePreview> {
  requireNative("Local release packaging");
  return invoke<PackagePreview>("prepare_modpack_release", { request });
}

export async function publishModpackRelease(
  previewId: string,
  confirmed: boolean,
): Promise<ModpackPublicationOutcome> {
  requireNative("GitHub release publishing");
  return invoke<ModpackPublicationOutcome>("publish_modpack_release", { previewId, confirmed });
}

export async function preparePublicCatalog(): Promise<CatalogPreview> {
  requireNative("Public catalogue preparation");
  return invoke<CatalogPreview>("prepare_public_catalog");
}

export async function publishPublicCatalog(
  previewId: string,
  confirmed: boolean,
): Promise<CatalogPublication> {
  requireNative("Public catalogue publishing");
  return invoke<CatalogPublication>("publish_public_catalog", { previewId, confirmed });
}

export async function prepareModpackTransaction(
  request: TransactionRequest,
): Promise<TransactionPreview> {
  requireNative("Safe update staging");
  return invoke<TransactionPreview>("prepare_modpack_transaction", { request });
}

export async function applyModpackTransaction(
  previewId: string,
  confirmed: boolean,
): Promise<TransactionOutcome> {
  requireNative("Modpack updates and repairs");
  return invoke<TransactionOutcome>("apply_modpack_transaction", { previewId, confirmed });
}

export async function listRestorePoints(profileId: string): Promise<RestorePointSummary[]> {
  requireNative("Restore-point history");
  return invoke<RestorePointSummary[]>("list_restore_points", { profileId });
}

export async function prepareRestorePoint(
  profileId: string,
  backupId: string,
): Promise<RestorePreview> {
  requireNative("Restore staging");
  return invoke<RestorePreview>("prepare_restore_point", { profileId, backupId });
}

export async function applyRestorePoint(
  previewId: string,
  confirmed: boolean,
): Promise<RestoreOutcome> {
  requireNative("Restore-point application");
  return invoke<RestoreOutcome>("apply_restore_point", { previewId, confirmed });
}

export async function deleteRestorePoint(
  profileId: string,
  backupId: string,
  confirmed: boolean,
): Promise<string> {
  requireNative("Restore-point deletion");
  return invoke<string>("delete_restore_point", { profileId, backupId, confirmed });
}

export async function getSafeLaunchStatus(profileId: string): Promise<SafeLaunchStatus> {
  requireNative("Safe Launch status");
  return invoke<SafeLaunchStatus>("safe_launch_status", { profileId });
}

export async function startSafeLaunch(
  profileId: string,
  confirmed: boolean,
): Promise<SafeLaunchOutcome> {
  requireNative("Safe Launch");
  return invoke<SafeLaunchOutcome>("start_safe_launch", { profileId, confirmed });
}

export async function recoverSafeLaunch(
  profileId: string,
  confirmed: boolean,
): Promise<SafeLaunchRecovery> {
  requireNative("Safe Launch recovery");
  return invoke<SafeLaunchRecovery>("recover_safe_launch", { profileId, confirmed });
}
