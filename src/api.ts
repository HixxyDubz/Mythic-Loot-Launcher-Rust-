import { invoke, isTauri } from "@tauri-apps/api/core";
import type {
  BootstrapPayload,
  DetectedInstall,
  FileVerification,
  GameProfile,
  LaunchOutcome,
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
import { previewPayload } from "./mock";

export const runningInTauri = isTauri();

export async function bootstrap(): Promise<BootstrapPayload> {
  return runningInTauri
    ? invoke<BootstrapPayload>("bootstrap")
    : Promise.resolve(previewPayload());
}

export async function selectProfile(profileId: string): Promise<BootstrapPayload | null> {
  return runningInTauri
    ? invoke<BootstrapPayload>("select_profile", { profileId })
    : null;
}

export async function saveProfile(profile: GameProfile): Promise<BootstrapPayload | null> {
  return runningInTauri
    ? invoke<BootstrapPayload>("save_profile", { profile })
    : null;
}

export async function detectInstallations(profile: GameProfile): Promise<DetectedInstall[]> {
  return runningInTauri
    ? invoke<DetectedInstall[]>("detect_installations", { profile })
    : [];
}

export async function verifyProfileFiles(profileId: string): Promise<FileVerification> {
  if (!runningInTauri) {
    throw new Error("File verification is only available in the native Tauri app.");
  }
  return invoke<FileVerification>("verify_profile_files", { profileId });
}

export async function launchProfile(profileId: string): Promise<LaunchOutcome> {
  if (!runningInTauri) {
    throw new Error("Game launch is only available in the native Tauri app.");
  }
  return invoke<LaunchOutcome>("launch_profile", { profileId });
}

export async function githubPublisherStatus(): Promise<PublisherStatus> {
  if (!runningInTauri) {
    return {
      ghAvailable: false,
      authenticated: false,
      account: "",
      message: "GitHub CLI preflight is only available in the native Developer app.",
    };
  }
  return invoke<PublisherStatus>("github_publisher_status");
}

export async function createGithubRepository(request: RepositoryRequest): Promise<RepositoryCreation> {
  if (!runningInTauri) {
    throw new Error("Repository creation is only available in the native Developer app.");
  }
  return invoke<RepositoryCreation>("create_github_repository", { request });
}

export async function prepareModpackRelease(request: PackageRequest): Promise<PackagePreview> {
  if (!runningInTauri) {
    throw new Error("Local release packaging is only available in the native Developer app.");
  }
  return invoke<PackagePreview>("prepare_modpack_release", { request });
}

export async function publishModpackRelease(
  previewId: string,
  confirmed: boolean,
): Promise<ReleasePublication> {
  if (!runningInTauri) {
    throw new Error("GitHub release publishing is only available in the native Developer app.");
  }
  return invoke<ReleasePublication>("publish_modpack_release", { previewId, confirmed });
}

export async function prepareModpackTransaction(
  request: TransactionRequest,
): Promise<TransactionPreview> {
  if (!runningInTauri) {
    throw new Error("Safe update staging is only available in the native Tauri app.");
  }
  return invoke<TransactionPreview>("prepare_modpack_transaction", { request });
}

export async function applyModpackTransaction(
  previewId: string,
  confirmed: boolean,
): Promise<TransactionOutcome> {
  if (!runningInTauri) {
    throw new Error("Modpack updates and repairs are only available in the native Tauri app.");
  }
  return invoke<TransactionOutcome>("apply_modpack_transaction", { previewId, confirmed });
}

export async function listRestorePoints(profileId: string): Promise<RestorePointSummary[]> {
  return runningInTauri
    ? invoke<RestorePointSummary[]>("list_restore_points", { profileId })
    : [];
}

export async function prepareRestorePoint(
  profileId: string,
  backupId: string,
): Promise<RestorePreview> {
  if (!runningInTauri) {
    throw new Error("Restore staging is only available in the native Tauri app.");
  }
  return invoke<RestorePreview>("prepare_restore_point", { profileId, backupId });
}

export async function applyRestorePoint(
  previewId: string,
  confirmed: boolean,
): Promise<RestoreOutcome> {
  if (!runningInTauri) {
    throw new Error("Restore points are only available in the native Tauri app.");
  }
  return invoke<RestoreOutcome>("apply_restore_point", { previewId, confirmed });
}

export async function deleteRestorePoint(
  profileId: string,
  backupId: string,
  confirmed: boolean,
): Promise<string> {
  if (!runningInTauri) {
    throw new Error("Restore-point deletion is only available in the native Tauri app.");
  }
  return invoke<string>("delete_restore_point", { profileId, backupId, confirmed });
}

export async function getSafeLaunchStatus(profileId: string): Promise<SafeLaunchStatus> {
  if (!runningInTauri) {
    return {
      profileId,
      active: false,
      sessionId: "",
      installDir: "",
      gameProcessId: 0,
      gameProcessRunning: false,
      disabledFiles: 0,
      startedAt: 0,
      recoverable: false,
      message: "No Safe Launch session is active.",
    };
  }
  return invoke<SafeLaunchStatus>("safe_launch_status", { profileId });
}

export async function startSafeLaunch(
  profileId: string,
  confirmed: boolean,
): Promise<SafeLaunchOutcome> {
  if (!runningInTauri) {
    throw new Error("Safe Launch is only available in the native Tauri app.");
  }
  return invoke<SafeLaunchOutcome>("start_safe_launch", { profileId, confirmed });
}

export async function recoverSafeLaunch(
  profileId: string,
  confirmed: boolean,
): Promise<SafeLaunchRecovery> {
  if (!runningInTauri) {
    throw new Error("Safe Launch recovery is only available in the native Tauri app.");
  }
  return invoke<SafeLaunchRecovery>("recover_safe_launch", { profileId, confirmed });
}
