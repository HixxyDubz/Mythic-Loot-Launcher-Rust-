import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { testProfiles } from "../test/fixtures";
import { Sidebar } from "./Sidebar";

describe("Archived catalogue profiles", () => {
  const profiles = testProfiles.map((p, i) => ({ ...p, catalogVisible: i === 1 }));
  const actions = { onSettings: vi.fn(), onActivity: vi.fn(), onStorage: vi.fn(), onSupport: vi.fn(), onAppUpdate: vi.fn(), onPublisher: vi.fn(), onAddModpack: vi.fn() };
  it("hides archived Player entries but lets the player access their retained installation", () => {
    const onSelect = vi.fn();
    render(<Sidebar {...actions} profiles={profiles} health={[]} selectedId="seven_days_main" edition="player" publisherAvailable={false} onSelect={onSelect} />);
    expect(screen.queryByText("Mythic Loot Minecraft")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Show archived modpacks (1)" }));
    expect(screen.getByText("Archived · local files kept")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /Mythic Loot Minecraft/ }));
    expect(onSelect).toHaveBeenCalledWith("minecraft_main");
  });
  it("always retains hidden Developer drafts in the authoring list", () => {
    render(<Sidebar {...actions} profiles={profiles} health={[]} selectedId="seven_days_main" edition="developer" publisherAvailable onSelect={vi.fn()} />);
    expect(screen.getByText("Mythic Loot Minecraft")).toBeInTheDocument();
    expect(screen.queryByText("Archived · local files kept")).not.toBeInTheDocument();
  });
});
