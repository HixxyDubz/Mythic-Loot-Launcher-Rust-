import { invoke, isTauri } from "@tauri-apps/api/core";
import type {
  BootstrapPayload,
  DetectedInstall,
  FileVerification,
  GameProfile,
  LaunchOutcome,
  ServerStatus,
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

export async function refreshServerStatus(profile: GameProfile, useCache = true): Promise<ServerStatus> {
  if (!runningInTauri) {
    return {
      profileId: profile.id,
      configured: Boolean(profile.serverIp),
      checked: false,
      online: null,
      players: null,
      maxPlayers: null,
      latencyMs: null,
      version: "",
      motd: "",
      map: "",
      message: "Native protocol checks are available in the Tauri app.",
      cached: false,
      checkedAtEpoch: null,
    };
  }
  return invoke<ServerStatus>("refresh_server_status", { profile, useCache });
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
