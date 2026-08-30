import type { BootstrapPayload, GameProfile, ManifestSummary, ProfileHealth } from "../types";

// Test-only component inputs. This module is never imported by production code.
export const testProfiles: GameProfile[] = [
  {
    id: "minecraft_main",
    game: "minecraft",
    displayName: "Mythic Loot Minecraft",
    requiredGameVersion: "1.21.1",
    requiredModpackVersion: "1.0.1",
    localModpackVersion: "",
    manifestPath: "manifests/minecraft_main.json",
    installDir: "",
    gameDir: "",
    gameExePath: "",
    launchArgs: "",
    minecraftLauncher: "",
    discordInvite: "",
    updateSource:
      "https://github.com/HixxyDubz/Mythic-Loot-Minecraft-Modpack/releases/download/v1.0.1/minecraft_main_1.0.1.zip",
    manifestUrl:
      "https://github.com/HixxyDubz/Mythic-Loot-Minecraft-Modpack/releases/latest/download/minecraft_main-manifest.json",
    deploymentSubdir: "",
    logoPath: "/assets/minecraft.png",
    catalogVisible: true,
  },
  {
    id: "seven_days_main",
    game: "seven_days",
    displayName: "Mythic Loot 7 Days",
    requiredGameVersion: "1.0",
    requiredModpackVersion: "1.0.0",
    localModpackVersion: "",
    manifestPath: "manifests/seven_days_main.json",
    installDir: "",
    gameDir: "",
    gameExePath: "",
    launchArgs: "",
    minecraftLauncher: "",
    discordInvite: "",
    updateSource: "",
    manifestUrl:
      "https://github.com/HixxyDubz/Mythic-Loot-7DTD-Modpack/releases/latest/download/seven_days_main-manifest.json",
    deploymentSubdir: "Mods",
    logoPath: "/assets/seven-days.png",
    catalogVisible: true,
  },
];

export function testHealth(profile: GameProfile): ProfileHealth {
  if (!profile.gameExePath) {
    return {
      profileId: profile.id,
      status: "setupRequired",
      headline: "Choose or detect the game client",
      details: ["No game executable is configured."],
    };
  }
  if (!profile.installDir) {
    return {
      profileId: profile.id,
      status: "setupRequired",
      headline: "Choose the modpack folder",
      details: ["Game client configured"],
    };
  }
  if (
    profile.requiredModpackVersion &&
    profile.localModpackVersion !== profile.requiredModpackVersion
  ) {
    return {
      profileId: profile.id,
      status: "updateRequired",
      headline: "The modpack version needs attention",
      details: [
        `Installed: ${profile.localModpackVersion || "Not verified"} · Required: ${profile.requiredModpackVersion}`,
      ],
    };
  }
  return {
    profileId: profile.id,
    status: "ready",
    headline: "Modpack is ready to launch",
    details: ["Game client found", "Modpack folder found"],
  };
}

export function testBootstrapPayload(profiles = structuredClone(testProfiles)): BootstrapPayload {
  const manifests: ManifestSummary[] = profiles.map((profile) => ({
    profileId: profile.id,
    valid: true,
    manifestVersion: "1.0",
    modpackVersion: profile.requiredModpackVersion,
    releaseDate: profile.game === "minecraft" ? "2026-08-13" : "2026-06-22",
    requiredFileCount: profile.game === "minecraft" ? 2067 : 0,
    optionalFileCount: 0,
    obsoleteFileCount: 0,
    updateSize: null,
    source: "test input",
    errors: [],
  }));
  return {
    config: {
      schemaVersion: 2,
      selectedProfileId: "minecraft_main",
      profiles,
      preferences: {
        reduceMotion: false,
        autoCheckUpdates: true,
        closeAfterLaunch: false,
      },
    },
    games: [],
    health: profiles.map(testHealth),
    manifests,
    dataDir: "test-only data directory",
  };
}
