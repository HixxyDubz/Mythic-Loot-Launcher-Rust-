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
