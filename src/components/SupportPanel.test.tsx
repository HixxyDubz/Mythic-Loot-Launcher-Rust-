import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
import { createSupportBundle, prepareSupportBundle } from "../api";
import type { SupportPreview } from "../types";
import { testProfiles } from "../test/fixtures";
import { SupportPanel } from "./SupportPanel";

vi.mock("../api", () => ({
  prepareSupportBundle: vi.fn(),
  createSupportBundle: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-opener", () => ({
  openPath: vi.fn(),
  revealItemInDir: vi.fn(),
}));

const preview: SupportPreview = {
  previewId: "review-123",
  profileId: "minecraft_main",
  displayName: "Mythic Loot Minecraft",
  latestLogPath: "C:\\Users\\Owner\\Minecraft\\logs\\latest.log",
  latestLogName: "latest.log",
  sourceBytes: 4096,
  includedBytes: 72,
  truncated: true,
  summary: "Profile: Mythic Loot Minecraft\nLatest log: <HOME>\\Minecraft\\logs\\latest.log\nServer configuration included: no\n",
  redactedLog: "Connecting as <USER> from <REDACTED_IP>\ntoken=<REDACTED>",
  files: ["summary.json", "summary.txt", "logs/latest.log.redacted.txt"],
  issues: [],
  ready: true,
  message: "A privacy-redacted support bundle is ready for review. No file has been written yet.",
};

describe("Support workspace", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(prepareSupportBundle).mockResolvedValue(preview);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText: vi.fn().mockResolvedValue(undefined) },
    });
  });

  it("shows exact reviewed text and requires confirmation for the same preview", async () => {
    vi.mocked(createSupportBundle).mockResolvedValue({
      profileId: preview.profileId,
      path: "C:\\LauncherData\\support-bundles\\support.zip",
      directory: "C:\\LauncherData\\support-bundles",
      fileName: "support.zip",
      bytes: 2048,
      sha256: "a".repeat(64),
      files: preview.files,
      message: "The reviewed privacy-redacted support bundle was created.",
    });
    render(<SupportPanel profile={testProfiles[0]} onBack={vi.fn()} onNotice={vi.fn()} />);

    expect(await screen.findByText("logs/latest.log.redacted.txt")).toBeInTheDocument();
    expect(screen.getByText(/Server configuration included: no/)).toBeInTheDocument();
    expect(screen.getByText(/Connecting as <USER>/)).toBeInTheDocument();
    expect(screen.getByText(/Server configuration is never included/i)).toBeInTheDocument();
    const create = screen.getByRole("button", { name: /create support bundle/i });
    expect(create).toBeDisabled();
    fireEvent.click(screen.getByRole("checkbox"));
    fireEvent.click(create);

    await waitFor(() => expect(createSupportBundle).toHaveBeenCalledWith("review-123", true));
    expect(await screen.findByText("Support bundle created")).toBeInTheDocument();
  });

  it("opens only the native-discovered source and copies only the redacted summary", async () => {
    vi.mocked(openPath).mockResolvedValue(undefined);
    vi.mocked(revealItemInDir).mockResolvedValue(undefined);
    const onNotice = vi.fn();
    render(<SupportPanel profile={testProfiles[0]} onBack={vi.fn()} onNotice={onNotice} />);

    fireEvent.click(await screen.findByRole("button", { name: /open source log/i }));
    fireEvent.click(screen.getByRole("button", { name: /copy summary/i }));

    await waitFor(() => expect(openPath).toHaveBeenCalledWith(preview.latestLogPath));
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith(preview.summary);
    expect(onNotice).toHaveBeenCalledWith(expect.stringContaining("redacted support summary"));
  });
});
