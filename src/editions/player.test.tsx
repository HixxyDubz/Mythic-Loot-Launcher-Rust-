import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Sidebar } from "../components/Sidebar";
import { testHealth, testProfiles } from "../test/fixtures";
import { launcherEdition, publisherAvailable } from "./player";

describe("Player edition", () => {
  it("identifies itself as Player and omits the publishing workspace", () => {
    expect(launcherEdition).toBe("player");
    expect(publisherAvailable).toBe(false);

    render(
      <Sidebar
        profiles={testProfiles}
        health={testProfiles.map(testHealth)}
        selectedId={testProfiles[0].id}
        edition="player"
        publisherAvailable={false}
        onSelect={vi.fn()}
        onSettings={vi.fn()}
        onActivity={vi.fn()}
        onStorage={vi.fn()}
        onPublisher={vi.fn()}
        onAddModpack={vi.fn()}
      />,
    );

    expect(screen.getByText("Player edition")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /storage/i })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /publisher/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /add modpack/i })).not.toBeInTheDocument();
  });
});
