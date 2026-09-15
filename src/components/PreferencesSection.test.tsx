import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { PreferencesSection } from "./PreferencesSection";
import { testBootstrapPayload } from "../test/fixtures";

describe("Launcher preferences", () => {
  it("saves all preferences separately and keeps manual refresh available", async () => {
    const save = vi.fn(async () => undefined);
    const refresh = vi.fn();
    const notice = vi.fn();
    render(<PreferencesSection preferences={testBootstrapPayload().config.preferences} busy={false} onSave={save} onNotice={notice} onRefreshCatalogue={refresh} />);
    fireEvent.click(screen.getByRole("checkbox", { name: /check for app and modpack/i }));
    fireEvent.click(screen.getByRole("checkbox", { name: /close mythic loot/i }));
    fireEvent.click(screen.getByRole("checkbox", { name: /reduce motion/i }));
    fireEvent.click(screen.getByRole("checkbox", { name: /show decorative background/i }));
    fireEvent.change(screen.getByLabelText("Colour theme"), { target: { value: "slate" } });
    fireEvent.change(screen.getByLabelText("Interface font"), { target: { value: "verdana" } });
    fireEvent.click(screen.getByRole("button", { name: /save launcher preferences/i }));
    await screen.findByRole("button", { name: /save launcher preferences/i });
    expect(save).toHaveBeenCalledWith({ autoCheckUpdates: false, closeAfterLaunch: true, reduceMotion: true, theme: "slate", font: "verdana", decorativeBackground: false });
    fireEvent.click(screen.getByRole("button", { name: /refresh catalogue now/i }));
    expect(refresh).toHaveBeenCalledOnce();
  });

  it("reports save failures without claiming success or discarding the draft", async () => {
    const notice = vi.fn();
    render(<PreferencesSection preferences={testBootstrapPayload().config.preferences} busy={false} onSave={async () => { throw new Error("Disk unavailable"); }} onNotice={notice} onRefreshCatalogue={() => undefined} />);
    fireEvent.change(screen.getByLabelText("Colour theme"), { target: { value: "slate" } });
    fireEvent.click(screen.getByRole("button", { name: /save launcher preferences/i }));
    await screen.findByRole("button", { name: /save launcher preferences/i });
    expect(notice).toHaveBeenCalledWith("Disk unavailable");
    expect(screen.getByLabelText("Colour theme")).toHaveValue("slate");
  });
});
