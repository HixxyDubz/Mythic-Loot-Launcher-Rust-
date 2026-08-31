import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { openPath } from "@tauri-apps/plugin-opener";
import { cleanStorage, getStorageReport } from "../api";
import type { StorageReport } from "../types";
import { StoragePanel } from "./StoragePanel";

vi.mock("../api", () => ({
  getStorageReport: vi.fn(),
  cleanStorage: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-opener", () => ({
  openPath: vi.fn(),
}));

const report: StorageReport = {
  dataDir: "C:\\LauncherData",
  launcherBytes: 2048,
  profileBytes: 4096,
  measuredAt: 1_788_134_400_000,
  temporaryRetentionHours: 24,
  backupsKeptPerProfile: 5,
  buckets: [
    {
      key: "profile:minecraft_main",
      label: "Mythic Loot Minecraft",
      category: "Modpack",
      path: "C:\\Minecraft",
      bytesUsed: 4096,
      fileCount: 12,
      directoryCount: 2,
      exists: true,
      truncated: false,
      cleanupKind: null,
    },
    {
      key: "catalog",
      label: "Verified catalogue cache",
      category: "Cache",
      path: "C:\\LauncherData\\catalog",
      bytesUsed: 1024,
      fileCount: 1,
      directoryCount: 0,
      exists: true,
      truncated: false,
      cleanupKind: "metadataCache",
    },
  ],
  issues: [],
  truncated: false,
};

describe("Storage workspace", () => {
  it("shows real native usage and requires confirmation before fixed-target cleanup", async () => {
    vi.mocked(getStorageReport).mockResolvedValue(report);
    vi.mocked(cleanStorage).mockResolvedValue({
      kind: "metadataCache",
      deletedEntries: 1,
      reclaimedBytes: 1024,
      skippedEntries: 0,
      complete: true,
      message: "Verified catalogue cache: removed 1 entry and reclaimed 1024 bytes.",
      report: { ...report, launcherBytes: 1024, buckets: report.buckets.map((bucket) => bucket.key === "catalog" ? { ...bucket, bytesUsed: 0, fileCount: 0 } : bucket) },
    });
    const onNotice = vi.fn();
    render(<StoragePanel onBack={vi.fn()} onNotice={onNotice} />);

    expect(await screen.findByText("Mythic Loot Minecraft")).toBeInTheDocument();
    expect(screen.getByText("2.0 KiB")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /clear catalogue cache/i }));
    const apply = screen.getByRole("button", { name: /apply cleanup/i });
    expect(apply).toBeDisabled();
    fireEvent.click(screen.getByRole("checkbox"));
    fireEvent.click(apply);

    await waitFor(() => expect(cleanStorage).toHaveBeenCalledWith("metadataCache", true));
    expect(onNotice).toHaveBeenCalledWith(expect.stringContaining("removed 1 entry"));
  });

  it("opens only the native-reported launcher data folder", async () => {
    vi.mocked(getStorageReport).mockResolvedValue(report);
    vi.mocked(openPath).mockResolvedValue(undefined);
    render(<StoragePanel onBack={vi.fn()} onNotice={vi.fn()} />);

    fireEvent.click(await screen.findByRole("button", { name: /launcher data folder/i }));
    await waitFor(() => expect(openPath).toHaveBeenCalledWith("C:\\LauncherData"));
  });
});
