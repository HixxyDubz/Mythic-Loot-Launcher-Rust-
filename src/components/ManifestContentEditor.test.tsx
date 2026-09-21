import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { applyContentRecovery, inspectContentRecovery, saveManifestContent } from "../api";
import { testBootstrapPayload } from "../test/fixtures";
import { ManifestContentEditor } from "./ManifestContentEditor";

vi.mock("../api", () => ({
  saveManifestContent: vi.fn(),
  inspectContentRecovery: vi.fn(), applyContentRecovery: vi.fn(),
}));

describe("Developer manifest content editor", () => {
  it("ignores a pending save after switching profiles", async () => {
    const payload = testBootstrapPayload();
    let finish!: (value: { changed: boolean; payload: typeof payload }) => void;
    vi.mocked(saveManifestContent).mockReturnValue(new Promise((resolve) => { finish = resolve; }));
    const onPayload = vi.fn(); const onSaved = vi.fn(); const onNotice = vi.fn();
    const view = render(<ManifestContentEditor profileId="minecraft_main" manifest={payload.manifests[0]} onNotice={onNotice} onPayload={onPayload} onSaved={onSaved} />);
    fireEvent.click(screen.getByRole("button", { name: "Save manifest content locally" }));
    expect(screen.getByLabelText("News announcement")).toBeDisabled();
    view.rerender(<ManifestContentEditor profileId="seven_days_main" manifest={payload.manifests[1]} onNotice={onNotice} onPayload={onPayload} onSaved={onSaved} />);
    await act(async () => { finish({ changed: true, payload }); });
    expect(onPayload).not.toHaveBeenCalled(); expect(onSaved).not.toHaveBeenCalled(); expect(onNotice).not.toHaveBeenCalled();
  });

  it("clears a previous recovery review after saving edited content", async () => {
    const payload = testBootstrapPayload();
    vi.mocked(inspectContentRecovery).mockResolvedValue({ profileId: "minecraft_main", draftRevision: "old", hasDraft: true, limited: false, candidates: [] });
    vi.mocked(saveManifestContent).mockResolvedValue({ changed: true, payload });
    render(<ManifestContentEditor profileId="minecraft_main" manifest={payload.manifests[0]} onNotice={vi.fn()} onPayload={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "Review saved content recovery" }));
    await screen.findByLabelText("Content recovery entry");
    fireEvent.click(screen.getByRole("checkbox"));
    fireEvent.change(screen.getByLabelText("News announcement"), { target: { value: "New saved text" } });
    fireEvent.click(screen.getByRole("button", { name: "Save manifest content locally" }));
    await waitFor(() => expect(screen.queryByLabelText("Content recovery entry")).not.toBeInTheDocument());
  });
  it("replaces unsaved editor fields with the native recovered result and invalidates release previews", async () => {
    const payload = testBootstrapPayload();
    const content = { announcement: "Recovered", newsBannerUrl: "", rulesGuide: { howToJoin: "", rules: [], commonFixes: [] }, changelog: [] };
    vi.mocked(inspectContentRecovery).mockResolvedValue({ profileId: "minecraft_main", draftRevision: "absent", hasDraft: false, limited: false, candidates: [{ id: "cached-reviewed", label: "Cached text", content, problem: null }] });
    const updated = structuredClone(payload); updated.manifests[0].announcement = "Recovered";
    vi.mocked(applyContentRecovery).mockResolvedValue({ changed: true, payload: updated });
    const onSaved = vi.fn(); const onPayload = vi.fn();
    render(<ManifestContentEditor profileId="minecraft_main" manifest={payload.manifests[0]} onNotice={vi.fn()} onPayload={onPayload} onSaved={onSaved} />);
    fireEvent.change(screen.getByLabelText("News announcement"), { target: { value: "Unsaved edit" } });
    fireEvent.click(screen.getByRole("button", { name: "Review saved content recovery" }));
    fireEvent.change(await screen.findByLabelText("Content recovery entry"), { target: { value: "cached-reviewed" } });
    fireEvent.click(screen.getByRole("checkbox")); fireEvent.click(screen.getByRole("button", { name: "Recover reviewed content locally" }));
    await waitFor(() => expect(screen.getByLabelText("News announcement")).toHaveValue("Recovered"));
    expect(onSaved).toHaveBeenCalledOnce(); expect(onPayload).toHaveBeenCalledWith(updated);
  });
  it("saves real typed news, rules and changelog content through native persistence", async () => {
    const payload = testBootstrapPayload();
    vi.mocked(saveManifestContent).mockResolvedValue({ changed: true, payload });
    const onNotice = vi.fn();
    const onPayload = vi.fn();
    render(
      <ManifestContentEditor
        profileId="minecraft_main"
        manifest={payload.manifests[0]}
        onNotice={onNotice}
        onPayload={onPayload}
      />,
    );

    fireEvent.change(screen.getByLabelText("News announcement"), {
      target: { value: "  A live balance update is ready.  " },
    });
    fireEvent.change(screen.getByLabelText("News banner HTTPS URL"), {
      target: { value: "https://example.com/banner.webp" },
    });
    fireEvent.change(screen.getByLabelText("How to install or join"), {
      target: { value: "Import the profile, then launch it." },
    });
    fireEvent.change(screen.getByLabelText("Rules (one per line)"), {
      target: { value: "Be kind\nNo exploits" },
    });
    fireEvent.change(screen.getByLabelText("Common fixes (one per line)"), {
      target: { value: "Run Repair\nRestart the launcher" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Add entry" }));
    fireEvent.change(screen.getByLabelText("Changelog version 1"), {
      target: { value: "1.1.0" },
    });
    fireEvent.change(screen.getByLabelText("Changelog notes 1"), {
      target: { value: "A focused update." },
    });
    fireEvent.change(screen.getByLabelText("Added in entry 1 (one per line)"), {
      target: { value: "New rewards" },
    });
    fireEvent.change(screen.getByLabelText("Changed in entry 1 (one per line)"), {
      target: { value: "Balanced loot" },
    });
    fireEvent.change(screen.getByLabelText("Fixed in entry 1 (one per line)"), {
      target: { value: "Recipe issue" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save manifest content locally" }));

    await waitFor(() => expect(saveManifestContent).toHaveBeenCalledWith(
      "minecraft_main",
      expect.objectContaining({
        announcement: "A live balance update is ready.",
        newsBannerUrl: "https://example.com/banner.webp",
        rulesGuide: {
          howToJoin: "Import the profile, then launch it.",
          rules: ["Be kind", "No exploits"],
          commonFixes: ["Run Repair", "Restart the launcher"],
        },
        changelog: [expect.objectContaining({
          version: "1.1.0",
          added: ["New rewards"],
          changed: ["Balanced loot"],
          fixed: ["Recipe issue"],
          notes: "A focused update.",
        })],
      }),
    ));
    expect(onPayload).toHaveBeenCalledWith(payload);
    expect(onNotice).toHaveBeenCalledWith(expect.stringContaining("included in the next modpack release"));
  });
});
