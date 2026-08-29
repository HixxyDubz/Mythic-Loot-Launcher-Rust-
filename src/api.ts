import { invoke, isTauri } from "@tauri-apps/api/core";
import type {
  BootstrapPayload,
  DetectedInstall,
  FileVerification,
  GameProfile,
  LaunchOutcome,
  MinecraftBootstrapArtifact,
  MinecraftBootstrapRequest,
  PackagePreview,
  PackageRequest,
  PublisherStatus,
  ReleasePublication,
  RepositoryCreation,
  RepositoryRequest,
  RestoreOutcome,
  RestorePointSummary,
  RestorePreview,
  SafeLaunchOutcome,
  SafeLaunchRecovery,
  SafeLaunchStatus,
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

export async function selectProfile(profileId: string): Promise<BootstrapPayload> {
  requireNative("Profile selection");
  return invoke<BootstrapPayload>("select_profile", { profileId });
}

export async function saveProfile(profile: GameProfile): Promise<BootstrapPayload> {
  requireNative("Profile saving");
  return invoke<BootstrapPayload>("save_profile", { profile });
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
): Promise<ReleasePublication> {
  requireNative("GitHub release publishing");
  return invoke<ReleasePublication>("publish_modpack_release", { previewId, confirmed });
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
