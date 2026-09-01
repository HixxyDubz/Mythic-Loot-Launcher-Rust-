import { describe, expect, it } from "vitest";
import {
  bootstrap,
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
    await expect(refreshPublicCatalog()).rejects.toThrow(/requires the native Mythic Loot Launcher/i);
    await expect(preparePublicCatalog()).rejects.toThrow(/requires the native Mythic Loot Launcher/i);
    await expect(publishPublicCatalog("preview", false)).rejects.toThrow(/requires the native Mythic Loot Launcher/i);
    await expect(githubPublisherStatus()).rejects.toThrow(/requires the native Mythic Loot Launcher/i);
    await expect(listRestorePoints("minecraft_main")).rejects.toThrow(/requires the native Mythic Loot Launcher/i);
    await expect(getSafeLaunchStatus("minecraft_main")).rejects.toThrow(/requires the native Mythic Loot Launcher/i);
    await expect(prepareSupportBundle("minecraft_main")).rejects.toThrow(/requires the native Mythic Loot Launcher/i);
  });
});
