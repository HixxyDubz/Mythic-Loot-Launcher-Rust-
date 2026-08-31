import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import App from "./App";
import { detectedModpackBase, isMinecraftSyncTarget, SettingsPanel } from "./components/SettingsPanel";
import { normalizeId } from "./components/ModpackManagerPanel";
import { refreshPublicCatalog } from "./api";
import { testBootstrapPayload, testProfiles } from "./test/fixtures";

vi.mock("./api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./api")>();
  const { testBootstrapPayload, testHealth } = await import("./test/fixtures");
  return {
    ...actual,
    bootstrap: vi.fn(async () => testBootstrapPayload()),
    refreshPublicCatalog: vi.fn(async () => ({
      payload: testBootstrapPayload(),
      summary: {
        catalogChanged: false,
        manifestsChanged: 0,
        manifestsChecked: 2,
        online: false,
        message: "Using verified local data.",
      },
    })),
    selectProfile: vi.fn(async (profileId: string) => {
      const payload = testBootstrapPayload();
      payload.config.selectedProfileId = profileId;
      return payload;
    }),
    saveProfile: vi.fn(async (profile) => {
      const payload = testBootstrapPayload();
      const exists = payload.config.profiles.some((item) => item.id === profile.id);
      payload.config.profiles = exists
        ? payload.config.profiles.map((item) => item.id === profile.id ? profile : item)
        : [...payload.config.profiles, profile];
      if (!exists) payload.config.selectedProfileId = profile.id;
      payload.health = payload.config.profiles.map(testHealth);
      payload.manifests.push({
        profileId: profile.id,
        valid: false,
        manifestVersion: "",
        modpackVersion: "",
        releaseDate: "",
        requiredFileCount: 0,
        optionalFileCount: 0,
        obsoleteFileCount: 0,
        updateSize: null,
        source: "unavailable",
        errors: ["No published manifest yet"],
        announcement: "",
        newsBannerUrl: "",
        rulesGuide: { howToJoin: "", rules: [], commonFixes: [] },
        changelog: [],
      });
      return payload;
    }),
    githubPublisherStatus: vi.fn(async () => ({
      ghAvailable: false,
      authenticated: false,
      account: "",
      message: "GitHub CLI is unavailable in this automated test.",
    })),
    listRestorePoints: vi.fn(async () => []),
    getSafeLaunchStatus: vi.fn(async (profileId: string) => ({
      profileId,
      active: false,
      sessionId: "",
      installDir: "",
      gameProcessId: 0,
      gameProcessRunning: false,
      disabledFiles: 0,
      startedAt: 0,
      recoverable: false,
      message: "No Safe Launch session is active.",
    })),
  };
});

describe("Mythic Loot launcher shell", () => {
  it("keeps a detected game root separate from its managed modpack subfolder", () => {
    expect(detectedModpackBase("C:\\Games\\7 Days To Die", "Mods")).toBe("C:\\Games\\7 Days To Die\\Mods");
    expect(detectedModpackBase("C:\\Games\\Minecraft", "")).toBe("C:\\Games\\Minecraft");
    expect(isMinecraftSyncTarget("curseforge")).toBe(true);
    expect(isMinecraftSyncTarget("modrinth")).toBe(true);
    expect(isMinecraftSyncTarget("official")).toBe(false);
  });

  it("records a detected CurseForge profile as the Minecraft sync target", async () => {
    const onSave = vi.fn();
    const onPrepare = vi.fn(async () => ({
      launcher: "curseforge" as const,
      fileName: "Mythic Loot Minecraft-curseforge-bootstrap.zip",
      path: "C:\\Bootstrap\\Mythic Loot Minecraft-curseforge-bootstrap.zip",
      bytes: 512,
      sha256: "a".repeat(64),
      message: "Bootstrap ZIP ready.",
    }));
    render(
      <SettingsPanel
        profile={testProfiles[0]}
        games={testBootstrapPayload().games}
        dataDir="Test data directory"
        busy={false}
        candidates={[{
          label: "CurseForge · Minecraft Very Vanilla",
          exePath: "C:\\Launchers\\CurseForge.exe",
          installDir: "C:\\Users\\Player\\curseforge\\minecraft\\Instances\\Minecraft Very Vanilla",
          source: "curseforge",
        }]}
        onBack={() => undefined}
        onDetect={() => undefined}
        onSave={onSave}
        onPrepareMinecraftBootstrap={onPrepare}
        onNotice={() => undefined}
      />,
    );

    expect(screen.getByText(/CurseForge and Modrinth are supported sync targets/i)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /CurseForge · Minecraft Very Vanilla/ }));
    expect(screen.getByText("Selected launcher: CurseForge")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /save settings/i }));
    expect(onSave).toHaveBeenCalledWith(expect.objectContaining({
      minecraftLauncher: "curseforge",
      installDir: "C:\\Users\\Player\\curseforge\\minecraft\\Instances\\Minecraft Very Vanilla",
    }));
    fireEvent.click(screen.getByRole("button", { name: /prepare curseforge import/i }));
    await screen.findByText("Mythic Loot Minecraft-curseforge-bootstrap.zip");
    expect(onPrepare).toHaveBeenCalledWith({ profileId: "minecraft_main", launcher: "curseforge" });
    expect(screen.getByText(/In CurseForge choose Import/i)).toBeInTheDocument();
  });

  it("applies the 7DTD Mods child when a detected Steam install is selected", () => {
    const onSave = vi.fn();
    render(
      <SettingsPanel
        profile={testProfiles[1]}
        games={testBootstrapPayload().games}
        dataDir="Test data directory"
        busy={false}
        candidates={[{
          label: "Steam · 7 Days To Die",
          exePath: "C:\\Games\\7 Days To Die\\7DaysToDie.exe",
          installDir: "C:\\Games\\7 Days To Die",
          source: "steam",
        }]}
        onBack={() => undefined}
        onDetect={() => undefined}
        onSave={onSave}
        onPrepareMinecraftBootstrap={async () => { throw new Error("not used"); }}
        onNotice={() => undefined}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: /Steam · 7 Days To Die/ }));
    expect(screen.getByLabelText("Game directory")).toHaveValue("C:\\Games\\7 Days To Die");
    expect(screen.getByLabelText("Modpack base folder")).toHaveValue("C:\\Games\\7 Days To Die\\Mods");
    fireEvent.click(screen.getByRole("button", { name: /save settings/i }));
    expect(onSave).toHaveBeenCalledWith(expect.objectContaining({
      gameDir: "C:\\Games\\7 Days To Die",
      installDir: "C:\\Games\\7 Days To Die\\Mods",
    }));
  });

  it("loads truthful first-run readiness and opens settings", async () => {
    render(<App />);
    expect(await screen.findByRole("heading", { name: "Mythic Loot Minecraft" })).toBeInTheDocument();
    expect(screen.getAllByText("Choose or detect the game client").length).toBeGreaterThan(0);
    fireEvent.click(screen.getByRole("button", { name: /complete setup/i }));
    expect(await screen.findByText("Public modpack identity")).toBeInTheDocument();
    expect(screen.getByLabelText("Game or launcher executable")).toBeInTheDocument();
  });

  it("merges a refreshed public catalogue into the visible modpack list", async () => {
    const newProfile = {
      ...testProfiles[0],
      id: "catalog_pack",
      displayName: "Catalogue Pack",
      installDir: "",
      gameExePath: "",
      localModpackVersion: "",
    };
    const refreshed = testBootstrapPayload([...structuredClone(testProfiles), newProfile]);
    vi.mocked(refreshPublicCatalog).mockResolvedValueOnce({
      payload: refreshed,
      summary: {
        catalogChanged: true,
        manifestsChanged: 1,
        manifestsChecked: 3,
        online: true,
        message: "Public catalogue refreshed.",
      },
    });
    render(<App />);
    expect(await screen.findByText("Catalogue Pack")).toBeInTheDocument();
    expect(screen.getByText("Public catalogue refreshed.")).toBeInTheDocument();
  });

  it("creates a normalized Developer modpack profile", async () => {
    expect(normalizeId(" My New Pack! ")).toBe("my_new_pack");
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: /add modpack/i }));
    expect(await screen.findByRole("heading", { name: "Add a modpack" })).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Display name"), { target: { value: "My New Pack" } });
    fireEvent.change(screen.getByLabelText("Repository (owner/name)"), { target: { value: "HixxyDubz/My-New-Pack" } });
    expect(screen.getByLabelText("Modpack ID")).toHaveValue("my_new_pack");
    fireEvent.click(screen.getByRole("button", { name: /create modpack/i }));
    expect(await screen.findByText(/My New Pack was created/i)).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "GitHub Publisher" })).toBeInTheDocument();
  });

  it("keeps GitHub repository creation behind preflight and preview", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: /publisher/i }));
    expect(await screen.findByRole("heading", { name: "GitHub Publisher" })).toBeInTheDocument();
    expect(screen.getByText("Preflight required")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /prepare release locally/i })).toBeDisabled();
    expect(screen.queryByRole("button", { name: /^create repository$/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /publish github release/i })).not.toBeInTheDocument();
  });

  it("keeps live modpack mutation behind staging preview and confirmation", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: /complete setup/i }));
    fireEvent.change(screen.getByLabelText("Game or launcher executable"), {
      target: { value: "C:\\Games\\fixture.exe" },
    });
    fireEvent.change(screen.getByLabelText("Modpack base folder"), {
      target: { value: "C:\\Modpacks\\Fixture" },
    });
    fireEvent.click(screen.getByRole("button", { name: /save settings/i }));
    await screen.findByRole("button", { name: /update & repair/i });
    await waitFor(() => expect(screen.getByRole("button", { name: /update & repair/i })).toBeEnabled());
    fireEvent.click(screen.getByRole("button", { name: /update & repair/i }));
    expect(await screen.findByRole("heading", { name: "Update & Repair" })).toBeInTheDocument();
    expect(screen.getByText("Live files stay untouched during preparation")).toBeInTheDocument();
    expect(await screen.findByRole("heading", { name: "Recovery history" })).toBeInTheDocument();
    expect(screen.getByText("No restore points yet")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /apply verified/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /restore verified point/i })).not.toBeInTheDocument();
  });

  it("keeps Safe Launch truthful when the manifest has no optional extras", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: /complete setup/i }));
    fireEvent.change(screen.getByLabelText("Game or launcher executable"), {
      target: { value: "C:\\Games\\fixture.exe" },
    });
    fireEvent.change(screen.getByLabelText("Modpack base folder"), {
      target: { value: "C:\\Modpacks\\Fixture" },
    });
    fireEvent.click(screen.getByRole("button", { name: /save settings/i }));
    const safeLaunch = await screen.findByRole("button", { name: /safe launch/i });
    await waitFor(() => expect(safeLaunch).toBeEnabled());
    fireEvent.click(safeLaunch);
    expect(await screen.findByRole("heading", { name: "Safe Launch" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "No optional extras are declared" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /start safe launch/i })).not.toBeInTheDocument();
  });
});
