import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { testBootstrapPayload } from "../test/fixtures";
import { Dashboard } from "./Dashboard";

describe("manifest content pages", () => {
  it("renders published news, structured changelog, rules and common fixes", () => {
    const payload = testBootstrapPayload();
    const manifest = payload.manifests[0];
    manifest.announcement = "The new balance update is ready.";
    manifest.changelog = [{
      version: "1.1.0",
      date: "2026-08-31",
      added: ["New progression rewards"],
      changed: ["Balanced early loot"],
      fixed: ["Corrected a recipe"],
      notes: "A focused quality update.",
    }];
    manifest.rulesGuide = {
      howToJoin: "Import the profile, then use Smart Launch.",
      rules: ["Respect other players"],
      commonFixes: ["Run Verify files after a launcher update"],
    };
    render(<Dashboard
      profile={payload.config.profiles[0]}
      health={payload.health[0]}
      manifest={manifest}
      busy={false}
      onOpenSettings={vi.fn()}
      onOpenSmartLaunch={vi.fn()}
      onVerifyFiles={vi.fn()}
      onOpenUpdates={vi.fn()}
      onOpenSafeLaunch={vi.fn()}
    />);

    fireEvent.click(screen.getByRole("button", { name: "News" }));
    expect(screen.getByText("The new balance update is ready.")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Changelog" }));
    expect(screen.getByText("New progression rewards")).toBeInTheDocument();
    expect(screen.getByText("Corrected a recipe")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Rules" }));
    expect(screen.getByText("Respect other players")).toBeInTheDocument();
    expect(screen.getByText("Run Verify files after a launcher update")).toBeInTheDocument();
  });
});
