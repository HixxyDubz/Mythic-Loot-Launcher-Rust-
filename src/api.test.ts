import { describe, expect, it } from "vitest";
import { testProfiles } from "./test/fixtures";
import {
  bootstrap,
  inspectMinecraftMetadata,
  inspectGameInstallation,
  inspectContentRecovery,
  applyContentRecovery,
  getSafeLaunchStatus,
  githubPublisherStatus,
  listRestorePoints,
  prepareSupportBundle,
  preparePublicCatalog,
  publishPublicCatalog,
  refreshPublicCatalog,
} from "./api";

describe("native API boundary", () => {
  it("fails closed outside Tauri instead of returning production fallback data", async () => {
    await expect(bootstrap()).rejects.toThrow(/requires the native Mythic Loot Launcher/i);
    await expect(inspectGameInstallation(testProfiles[1])).rejects.toThrow(/requires the native Mythic Loot Launcher/i);
    await expect(inspectContentRecovery("minecraft_main")).rejects.toThrow(/requires the native Mythic Loot Launcher/i);
    await expect(applyContentRecovery("minecraft_main", null, "absent", true)).rejects.toThrow(/requires the native Mythic Loot Launcher/i);
    await expect(inspectMinecraftMetadata("minecraft_main", "C:\\Pack")).rejects.toThrow(/requires the native Mythic Loot Launcher/i);
    await expect(refreshPublicCatalog()).rejects.toThrow(/requires the native Mythic Loot Launcher/i);
    await expect(preparePublicCatalog()).rejects.toThrow(/requires the native Mythic Loot Launcher/i);
    await expect(publishPublicCatalog("preview", false)).rejects.toThrow(/requires the native Mythic Loot Launcher/i);
    await expect(githubPublisherStatus()).rejects.toThrow(/requires the native Mythic Loot Launcher/i);
    await expect(listRestorePoints("minecraft_main")).rejects.toThrow(/requires the native Mythic Loot Launcher/i);
    await expect(getSafeLaunchStatus("minecraft_main")).rejects.toThrow(/requires the native Mythic Loot Launcher/i);
    await expect(prepareSupportBundle("minecraft_main")).rejects.toThrow(/requires the native Mythic Loot Launcher/i);
  });
});
