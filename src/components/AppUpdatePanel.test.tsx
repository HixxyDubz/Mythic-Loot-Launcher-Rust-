import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { applyAppUpdate, checkAppUpdate, getAppUpdateResult, prepareAppUpdate } from "../api";
import type { AppUpdatePreview } from "../types";
import { AppUpdatePanel } from "./AppUpdatePanel";

vi.mock("../api", () => ({
  applyAppUpdate: vi.fn(),
  checkAppUpdate: vi.fn(),
  getAppUpdateResult: vi.fn(),
  prepareAppUpdate: vi.fn(),
}));

vi.mock("@launcher-edition", () => ({
  launcherEdition: "player",
  publisherAvailable: false,
  EditionAppUpdatePublisherPanel: () => null,
}));

const preview: AppUpdatePreview = {
  previewId: "preview-123",
  feedUrl: "https://github.com/HixxyDubz/Mythic-Loot-Launcher-Rust-/releases/latest/download/launcher-update-player.json",
  currentVersion: "0.1.0",
  latestVersion: "0.2.0",
  releaseNotes: "Safer Player application updates.",
  publishedAt: "2026-09-01T12:00:00Z",
  mandatory: false,
  minimumSupportedVersion: "0.1.0",
  assetBytes: 4_194_304,
  assetSha256: "a".repeat(64),
  updateAvailable: true,
  supported: true,
  canInstall: true,
  message: "Player 0.2.0 is available.",
};

describe("App update workspace", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(checkAppUpdate).mockResolvedValue(preview);
    vi.mocked(getAppUpdateResult).mockResolvedValue(null);
  });

  it("shows the reviewed checksum and requires confirmation before restart", async () => {
    vi.mocked(prepareAppUpdate).mockResolvedValue({
      stageId: "stage-123",
      version: "0.2.0",
      path: "C:\\LauncherData\\app-update-staging\\stage-123\\Mythic-Loot-Launcher-Player.exe",
      bytes: preview.assetBytes,
      sha256: preview.assetSha256,
      ready: true,
      message: "The Player update was downloaded and verified.",
    });
    vi.mocked(applyAppUpdate).mockResolvedValue({
      version: "0.2.0",
      helperStarted: true,
      message: "Player will close, update and restart.",
    });

    render(<AppUpdatePanel onBack={vi.fn()} onNotice={vi.fn()} />);

    expect(await screen.findByText(preview.assetSha256)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /download and verify/i }));
    const restart = await screen.findByRole("button", { name: /update and restart player/i });
    expect(restart).toBeDisabled();
    fireEvent.click(screen.getByRole("checkbox"));
    fireEvent.click(restart);

    await waitFor(() => expect(prepareAppUpdate).toHaveBeenCalledWith("preview-123"));
    expect(applyAppUpdate).toHaveBeenCalledWith("stage-123", true);
  });

  it("reports an absent first release without inventing an update", async () => {
    vi.mocked(checkAppUpdate).mockRejectedValue(new Error("GitHub returned 404"));
    render(<AppUpdatePanel onBack={vi.fn()} onNotice={vi.fn()} />);

    expect(await screen.findByText("No public Player update feed is available yet")).toBeInTheDocument();
    expect(screen.getByText("GitHub returned 404")).toBeInTheDocument();
    expect(prepareAppUpdate).not.toHaveBeenCalled();
  });
});
