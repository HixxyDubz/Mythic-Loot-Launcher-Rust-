import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { chooseLocalPath, loadPublishingChoices, prepareModpackRelease, publishModpackRelease, savePublishingChoices } from "../api";
import { testBootstrapPayload, testProfiles } from "../test/fixtures";
import type { PackagePreview, PublishingChoices } from "../types";
import { PublisherPanel } from "./PublisherPanel";

vi.mock("../api", () => ({
  chooseLocalPath: vi.fn(), loadPublishingChoices: vi.fn(), savePublishingChoices: vi.fn(),
  prepareModpackRelease: vi.fn(), publishModpackRelease: vi.fn(),
  createGithubRepository: vi.fn(), githubPublisherStatus: vi.fn(), preparePublicCatalog: vi.fn(), publishPublicCatalog: vi.fn(),
}));
vi.mock("./ManifestContentEditor", () => ({ ManifestContentEditor: () => null }));
vi.mock("./ManifestContentPublisher", () => ({ ManifestContentPublisher: () => null }));

const choices: PublishingChoices = {
  request: { profileId: "seven_days_main", sourceDir: "D:\\Packs\\Mods", gameVersion: "3.1", minecraftModLoader: "", version: "2.0.0", repository: "owner/pack", releaseDate: "2026-09-10", releaseNotes: "Balance pass" },
  gameVersions: ["3.1", "3.0"],
};
const preview: PackagePreview = {
  previewId: "reviewed", profileId: "seven_days_main", version: "2.0.0", gameVersion: "3.2", minecraftModLoader: "", tag: "v2.0.0", repository: "owner/pack",
  sourceDir: "E:\\Balance\\Mods", outputDir: "D:\\Local Preview", packagePath: "D:\\Local Preview\\pack.zip", manifestPath: "D:\\Local Preview\\manifest.json",
  fileCount: 2, optionalFileCount: 0, excludedCount: 1, totalBytes: 20, packageBytes: 100, packageSha256: "a".repeat(64), multipart: false, assets: [], added: 2, changed: 0, removed: 0, issues: [], ready: true,
};

function show(index = 1) {
  return render(<PublisherPanel profile={testProfiles[index]} manifest={testBootstrapPayload().manifests[index]} onBack={vi.fn()} onNotice={vi.fn()} onPayload={vi.fn()} />);
}

describe("Per-release publishing choices", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    vi.mocked(loadPublishingChoices).mockResolvedValue(structuredClone(choices));
    vi.mocked(prepareModpackRelease).mockResolvedValue(preview);
    vi.mocked(savePublishingChoices).mockResolvedValue(choices);
  });

  it("loads local choices, browses the source and reviews a newly selected game version without publishing", async () => {
    show();
    await waitFor(() => expect(screen.getByLabelText("Modpack source folder")).toHaveValue("D:\\Packs\\Mods"));
    expect(screen.getByLabelText("Game version for this release")).toHaveValue("3.1");
    vi.mocked(chooseLocalPath).mockResolvedValue("E:\\Balance\\Mods");
    fireEvent.click(screen.getByRole("button", { name: "Browse for modpack source folder" }));
    await waitFor(() => expect(screen.getByLabelText("Modpack source folder")).toHaveValue("E:\\Balance\\Mods"));
    fireEvent.change(screen.getByLabelText("Game version for this release"), { target: { value: "3.2" } });
    fireEvent.change(screen.getByLabelText("Optional files or folders"), { target: { value: "BonusMod\n  OptionalAudio  " } });
    fireEvent.click(screen.getByRole("button", { name: "Save publishing choices locally" }));
    await waitFor(() => expect(savePublishingChoices).toHaveBeenCalledWith(expect.objectContaining({ gameVersion: "3.2", sourceDir: "E:\\Balance\\Mods" })));
    await waitFor(() => expect(screen.getByRole("button", { name: "Prepare release locally" })).toBeEnabled());
    fireEvent.click(screen.getByRole("button", { name: "Prepare release locally" }));
    expect(await screen.findByRole("heading", { name: "Release preview ready" })).toBeInTheDocument();
    expect(prepareModpackRelease).toHaveBeenCalledWith(expect.objectContaining({ gameVersion: "3.2", sourceDir: "E:\\Balance\\Mods", version: "2.0.0", optionalPaths: ["BonusMod", "OptionalAudio"] }));
    expect(screen.getByRole("button", { name: "Publish GitHub release" })).toBeDisabled();
    expect(publishModpackRelease).not.toHaveBeenCalled();
    fireEvent.change(screen.getByLabelText("Game version for this release"), { target: { value: "3.3" } });
    expect(screen.queryByRole("heading", { name: "Release preview ready" })).not.toBeInTheDocument();
  });

  it("keeps the source path unchanged if the native browser is cancelled", async () => {
    show();
    await waitFor(() => expect(screen.getByLabelText("Modpack source folder")).toHaveValue(choices.request.sourceDir));
    vi.mocked(chooseLocalPath).mockResolvedValue(null);
    fireEvent.click(screen.getByRole("button", { name: "Browse for modpack source folder" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Browse for modpack source folder" })).toBeEnabled());
    expect(screen.getByLabelText("Modpack source folder")).toHaveValue(choices.request.sourceDir);
  });

  it("requires an explicit Minecraft loader instead of silently using NeoForge", async () => {
    vi.mocked(loadPublishingChoices).mockResolvedValue({ ...choices, request: { ...choices.request, profileId: "minecraft_main", gameVersion: "1.20.1" } });
    show(0);
    await waitFor(() => expect(screen.getByLabelText("Game version for this release")).toHaveValue("1.20.1"));
    expect(screen.getByRole("button", { name: "Prepare release locally" })).toBeDisabled();
    fireEvent.change(screen.getByLabelText("Minecraft loader identity"), { target: { value: "fabric-loader-0.16.10" } });
    expect(screen.getByRole("button", { name: "Prepare release locally" })).toBeEnabled();
  });

  it("blocks preparing over unreadable saved choices instead of replacing them with defaults", async () => {
    vi.mocked(loadPublishingChoices).mockRejectedValue(new Error("Saved choices are damaged"));
    show();
    expect(await screen.findByRole("alert")).toHaveTextContent("Saved choices are damaged");
    expect(screen.getByRole("button", { name: "Prepare release locally" })).toBeDisabled();
    expect(savePublishingChoices).not.toHaveBeenCalled();
  });
});
