import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { prepareManifestContentRelease, publishManifestContentRelease } from "../api";
import { ManifestContentPublisher } from "./ManifestContentPublisher";

vi.mock("../api", () => ({
  prepareManifestContentRelease: vi.fn(),
  publishManifestContentRelease: vi.fn(),
}));

describe("Developer content-only publication", () => {
  it("reviews one manifest, preserves package references and requires confirmation", async () => {
    vi.mocked(prepareManifestContentRelease).mockResolvedValue({
      previewId: "content-preview",
      profileId: "minecraft_main",
      repository: "HixxyDubz/Mythic-Loot-Minecraft-Modpack",
      tag: "content-1788134400-abc1234567",
      manifestUrl: "https://github.com/HixxyDubz/Mythic-Loot-Minecraft-Modpack/releases/latest/download/minecraft_main-manifest.json",
      manifestPath: "C:\\Launcher Data\\content-release-previews\\minecraft_main-manifest.json",
      modpackVersion: "1.0.1",
      bytes: 616_266,
      sha256: "a".repeat(64),
      packageAssetsPreserved: 1,
      requiredFileCount: 2_067,
      rulesCount: 2,
      changelogCount: 1,
      issues: [],
      ready: true,
    });
    vi.mocked(publishManifestContentRelease).mockResolvedValue({
      profileId: "minecraft_main",
      repository: "HixxyDubz/Mythic-Loot-Minecraft-Modpack",
      tag: "content-1788134400-abc1234567",
      manifestUrl: "https://github.com/HixxyDubz/Mythic-Loot-Minecraft-Modpack/releases/latest/download/minecraft_main-manifest.json",
      url: "https://github.com/HixxyDubz/Mythic-Loot-Minecraft-Modpack/releases/tag/content-1788134400-abc1234567",
      message: "Published only the manifest.",
    });
    const onNotice = vi.fn();
    render(
      <ManifestContentPublisher
        profileId="minecraft_main"
        githubAuthenticated
        onNotice={onNotice}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Prepare content-only release" }));
    expect(await screen.findByRole("heading", { name: "Content release preview ready" })).toBeInTheDocument();
    expect(screen.getByText("1. Trusted manifest only")).toBeInTheDocument();
    expect(screen.getByText(/1 immutable asset preserved/i)).toBeInTheDocument();
    const publish = screen.getByRole("button", { name: "Publish content-only release" });
    expect(publish).toBeDisabled();
    fireEvent.click(screen.getByRole("checkbox"));
    expect(publish).toBeEnabled();
    fireEvent.click(publish);

    await waitFor(() => expect(publishManifestContentRelease).toHaveBeenCalledWith(
      "content-preview",
      true,
    ));
    expect(await screen.findByRole("heading", { name: "content-1788134400-abc1234567 published" })).toBeInTheDocument();
  });
});
