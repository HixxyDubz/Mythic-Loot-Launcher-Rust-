import type { BootstrapPayload, GameProfile, ProfileHealth } from "./types";

export const previewProfiles: GameProfile[] = [
  {
    id: "minecraft_main",
    game: "minecraft",
    displayName: "Minecraft - Mythic Loot Server",
    serverName: "Mythic Loot Minecraft",
    serverIp: "",
    serverPort: 25565,
    requiredGameVersion: "1.21.1",
    requiredModpackVersion: "1.0.1",
    localModpackVersion: "",
    manifestPath: "manifests/minecraft_main.json",
    installDir: "",
    gameDir: "",
    gameExePath: "",
    launchArgs: "",
    discordInvite: "",
    updateSource:
      "https://github.com/HixxyDubz/Mythic-Loot-Minecraft-Modpack/releases/download/v1.0.1/minecraft_main_1.0.1.zip",
    manifestUrl:
      "https://github.com/HixxyDubz/Mythic-Loot-Minecraft-Modpack/releases/latest/download/minecraft_main-manifest.json",
    deploymentSubdir: "",
    logoPath: "/assets/minecraft.png",
  },
  {
    id: "seven_days_main",
    game: "seven_days",
    displayName: "7 Days To Die - Mythic Loot Server",
    serverName: "Mythic Loot 7 Days",
    serverIp: "",
    serverPort: 26900,
    requiredGameVersion: "1.0",
    requiredModpackVersion: "1.0.0",
    localModpackVersion: "",
    manifestPath: "manifests/seven_days_main.json",
    installDir: "",
    gameDir: "",
    gameExePath: "",
    launchArgs: "",
    discordInvite: "",
    updateSource: "",
    manifestUrl:
      "https://github.com/HixxyDubz/Mythic-Loot-7DTD-Modpack/releases/latest/download/seven_days_main-manifest.json",
    deploymentSubdir: "Mods",
    logoPath: "/assets/seven-days.png",
  },
];

export function previewHealth(profile: GameProfile): ProfileHealth {
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
    headline: "Ready to play",
    details: ["Game client found", "Modpack folder found"],
  };
}

export function previewPayload(): BootstrapPayload {
  return {
    config: {
      schemaVersion: 1,
      selectedProfileId: "minecraft_main",
      profiles: structuredClone(previewProfiles),
      preferences: {
        reduceMotion: false,
        autoCheckUpdates: true,
        closeAfterLaunch: false,
      },
    },
    games: [
      ["minecraft", "Minecraft", "minecraft"],
      ["seven_days", "7 Days to Die", "steam"],
      ["palworld", "Palworld", "steam"],
      ["core_keeper", "Core Keeper", "steam"],
      ["marvel_heroes", "Marvel Heroes", "steam"],
      ["valheim", "Valheim", "steam"],
      ["factorio", "Factorio", "steam"],
      ["stardew_valley", "Stardew Valley", "steam"],
      ["hytale", "Hytale", "manual"],
      ["world_of_warcraft", "World of Warcraft", "manual"],
      ["runescape", "RuneScape", "manual"],
      ["city_of_heroes", "City of Heroes - Sanctuary", "manual"],
    ].map(([id, displayName, detectionKind]) => ({ id, displayName, detectionKind })),
    health: previewProfiles.map(previewHealth),
    dataDir: "Browser preview · native persistence is available in the Tauri app",
  };
}
