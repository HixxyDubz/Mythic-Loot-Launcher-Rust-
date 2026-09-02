import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { preparePlayerAppRelease, publishPlayerAppRelease } from "../api";
import type { AppReleasePreview } from "../types";
import { AppUpdatePublisherPanel } from "./AppUpdatePublisherPanel";

vi.mock("../api", () => ({
  preparePlayerAppRelease: vi.fn(),
  publishPlayerAppRelease: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-opener", () => ({ openPath: vi.fn() }));

const preview: AppReleasePreview = {
  previewId: "release-123",
  repository: "HixxyDubz/Mythic-Loot-Launcher-Rust-",
  tag: "v0.2.0",
  version: "0.2.0",
  releaseNotes: "Player update release.",
  feedUrl: "https://github.com/HixxyDubz/Mythic-Loot-Launcher-Rust-/releases/latest/download/launcher-update-player.json",
  outputDirectory: "C:\\LauncherData\\app-update-release-previews\\release-123",
  assets: [
    { fileName: "Mythic-Loot-Launcher-Player.exe", bytes: 10, sha256: "a".repeat(64) },
    { fileName: "Mythic-Loot-Launcher-Player-Setup.exe", bytes: 20, sha256: "b".repeat(64) },
    { fileName: "launcher-update-player.json", bytes: 30, sha256: "c".repeat(64) },
  ],
  ready: true,
  issues: [],
  message: "The exact Player release is ready for review.",
};

describe("Developer Player app release publisher", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(preparePlayerAppRelease).mockResolvedValue(preview);
  });

  it("previews only the three fixed public assets and confirms before publication", async () => {
    vi.mocked(publishPlayerAppRelease).mockResolvedValue({
      repository: preview.repository,
      tag: preview.tag,
      version: preview.version,
      url: "https://github.com/HixxyDubz/Mythic-Loot-Launcher-Rust-/releases/tag/v0.2.0",
      assets: 3,
      message: "The Player app release was published.",
    });
    render(<AppUpdatePublisherPanel onNotice={vi.fn()} />);

    fireEvent.change(screen.getByLabelText("Release notes"), { target: { value: preview.releaseNotes } });
    fireEvent.click(screen.getByRole("button", { name: /verify packaged player release/i }));
    expect(await screen.findByText("Mythic-Loot-Launcher-Player.exe")).toBeInTheDocument();
    expect(screen.getByText("Mythic-Loot-Launcher-Player-Setup.exe")).toBeInTheDocument();
    expect(screen.getByText("launcher-update-player.json")).toBeInTheDocument();
    expect(screen.queryByText(/developer\.exe/i)).not.toBeInTheDocument();

    const publish = screen.getByRole("button", { name: /publish player app update/i });
    expect(publish).toBeDisabled();
    fireEvent.click(screen.getByRole("checkbox"));
    fireEvent.click(publish);

    await waitFor(() => expect(preparePlayerAppRelease).toHaveBeenCalledWith({ buildManifestPath: "", releaseNotes: preview.releaseNotes }));
    expect(publishPlayerAppRelease).toHaveBeenCalledWith("release-123", true);
  });
});
