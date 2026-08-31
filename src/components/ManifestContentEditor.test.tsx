import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { saveManifestContent } from "../api";
import { testBootstrapPayload } from "../test/fixtures";
import { ManifestContentEditor } from "./ManifestContentEditor";

vi.mock("../api", () => ({
  saveManifestContent: vi.fn(),
}));

describe("Developer manifest content editor", () => {
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
