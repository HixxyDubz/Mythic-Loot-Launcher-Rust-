import { fireEvent, render, screen } from "@testing-library/react";
import type { ComponentProps } from "react";
import { describe, expect, it, vi } from "vitest";
import { testProfiles } from "../test/fixtures";
import type {
  FileVerification,
  ManifestSummary,
  ProfileHealth,
  TransactionPreview,
} from "../types";
import { SmartLaunchPanel } from "./SmartLaunchPanel";

const profile = {
  ...testProfiles[0],
  installDir: "C:\\Modpacks\\Minecraft Very Vanilla",
  gameDir: "C:\\Modpacks\\Minecraft Very Vanilla",
  gameExePath: "C:\\Launchers\\CurseForge.exe",
  minecraftLauncher: "curseforge",
  localModpackVersion: "1.0.1",
};

const health: ProfileHealth = {
  profileId: profile.id,
  status: "ready",
  headline: "Modpack is ready to launch",
  details: [],
};

const manifest: ManifestSummary = {
  profileId: profile.id,
  valid: true,
  manifestVersion: "1.0",
  modpackVersion: "1.0.1",
  releaseDate: "2026-08-13",
  requiredFileCount: 2,
  optionalFileCount: 0,
  obsoleteFileCount: 0,
  updateSize: 2048,
  source: "test manifest",
  errors: [],
};

const cleanVerification: FileVerification = {
  profileId: profile.id,
  checked: 2,
  current: 2,
  missing: [],
  changed: [],
  unsafeEntries: [],
};

function renderPanel(overrides: Partial<ComponentProps<typeof SmartLaunchPanel>> = {}) {
  const props: ComponentProps<typeof SmartLaunchPanel> = {
    profile,
    health,
    manifest,
    onBack: vi.fn(),
    onNotice: vi.fn(),
    onVerify: vi.fn(async () => cleanVerification),
    onPrepare: vi.fn(async () => { throw new Error("not expected"); }),
    onApply: vi.fn(async () => { throw new Error("not expected"); }),
    onRefresh: vi.fn(async () => undefined),
    onLaunch: vi.fn(async () => ({ pid: 42, message: "CurseForge opened." })),
    ...overrides,
  };
  render(<SmartLaunchPanel {...props} />);
  return props;
}

describe("Smart Launch", () => {
  it("opens a current installation only after a clean verification", async () => {
    const props = renderPanel();

    fireEvent.click(screen.getByRole("button", { name: /check and smart launch/i }));

    expect(await screen.findByRole("heading", { name: "Verified launch complete" })).toBeInTheDocument();
    expect(props.onVerify).toHaveBeenCalledWith(profile.id);
    expect(props.onPrepare).not.toHaveBeenCalled();
    expect(props.onApply).not.toHaveBeenCalled();
    expect(props.onLaunch).toHaveBeenCalledWith(profile.id);
  });

  it("stages an update, requires confirmation, rechecks, then opens", async () => {
    const oldProfile = { ...profile, localModpackVersion: "1.0.0" };
    const before: FileVerification = {
      ...cleanVerification,
      current: 1,
      changed: ["mods/example.jar"],
    };
    const candidate: TransactionPreview = {
      previewId: "preview-1",
      profileId: profile.id,
      kind: "update",
      version: "1.0.1",
      source: "trusted-package.zip",
      stagedFiles: 1,
      stagedBytes: 2048,
      existingFilesToBackup: 1,
      newFiles: 0,
      obsoletePaths: 0,
      issues: [],
      ready: true,
      nothingToDo: false,
      message: "Update candidate verified.",
    };
    const onVerify = vi.fn()
      .mockResolvedValueOnce(before)
      .mockResolvedValueOnce(cleanVerification);
    const onPrepare = vi.fn(async () => candidate);
    const onApply = vi.fn(async () => ({
      profileId: profile.id,
      kind: "update" as const,
      success: true,
      applied: ["mods/example.jar"],
      removed: [],
      backupPath: "C:\\Backups\\preview-1.zip",
      rolledBack: false,
      rollbackError: "",
      message: "Update applied safely.",
      error: "",
    }));
    const onRefresh = vi.fn(async () => undefined);
    const onLaunch = vi.fn(async () => ({ pid: 42, message: "CurseForge opened." }));
    renderPanel({ profile: oldProfile, onVerify, onPrepare, onApply, onRefresh, onLaunch });

    fireEvent.click(screen.getByRole("button", { name: /check and smart launch/i }));
    expect(await screen.findByRole("heading", { name: "Update candidate verified" })).toBeInTheDocument();
    expect(onPrepare).toHaveBeenCalledWith({ profileId: profile.id, kind: "update" });

    const applyButton = screen.getByRole("button", { name: /apply, recheck and launch/i });
    expect(applyButton).toBeDisabled();
    fireEvent.click(screen.getByRole("checkbox"));
    expect(applyButton).toBeEnabled();
    fireEvent.click(applyButton);

    expect(await screen.findByRole("heading", { name: "Verified launch complete" })).toBeInTheDocument();
    expect(onApply).toHaveBeenCalledWith("preview-1", true);
    expect(onRefresh).toHaveBeenCalledOnce();
    expect(onVerify).toHaveBeenCalledTimes(2);
    expect(onLaunch).toHaveBeenCalledWith(profile.id);
  });

  it("blocks unsafe manifest paths without staging or opening", async () => {
    const unsafe: FileVerification = {
      ...cleanVerification,
      current: 1,
      unsafeEntries: ["../outside.txt"],
    };
    const onPrepare = vi.fn();
    const onLaunch = vi.fn();
    renderPanel({ onVerify: vi.fn(async () => unsafe), onPrepare, onLaunch });

    fireEvent.click(screen.getByRole("button", { name: /check and smart launch/i }));

    expect(await screen.findByRole("heading", { name: "Manual action required" })).toBeInTheDocument();
    expect(screen.getAllByText(/unsafe path/i)).toHaveLength(2);
    expect(onPrepare).not.toHaveBeenCalled();
    expect(onLaunch).not.toHaveBeenCalled();
  });
});
