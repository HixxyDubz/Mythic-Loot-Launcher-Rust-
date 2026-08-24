import { invoke, isTauri } from "@tauri-apps/api/core";
import type {
  BootstrapPayload,
  DetectedInstall,
  GameProfile,
  LaunchOutcome,
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

export async function launchProfile(profileId: string): Promise<LaunchOutcome> {
  if (!runningInTauri) {
    throw new Error("Game launch is only available in the native Tauri app.");
  }
  return invoke<LaunchOutcome>("launch_profile", { profileId });
}
