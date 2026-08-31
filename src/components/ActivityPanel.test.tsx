import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { clearFinishedActivity, listActivity } from "../api";
import type { ActivityItem } from "../types";
import { ActivityPanel } from "./ActivityPanel";

vi.mock("../api", () => ({
  listActivity: vi.fn(),
  clearFinishedActivity: vi.fn(),
}));

const active: ActivityItem = {
  id: "active-1",
  title: "Minecraft file verification",
  kind: "verifying",
  message: "Hashing required modpack files",
  progress: null,
  bytesDone: null,
  bytesTotal: null,
  startedAt: 1_788_134_400_000,
  updatedAt: 1_788_134_400_000,
  done: false,
  success: null,
};

const finished: ActivityItem = {
  ...active,
  id: "finished-1",
  title: "Public modpack catalogue",
  kind: "catalogue",
  message: "Public catalogue refreshed.",
  progress: 1,
  done: true,
  success: true,
};

describe("Activity Centre", () => {
  it("shows real recent operations and clears only finished entries", async () => {
    vi.mocked(listActivity).mockResolvedValue([active, finished]);
    vi.mocked(clearFinishedActivity).mockResolvedValue([active]);
    const onNotice = vi.fn();
    render(<ActivityPanel onBack={vi.fn()} onNotice={onNotice} />);

    expect(await screen.findByText("Minecraft file verification")).toBeInTheDocument();
    expect(screen.getByText("Public modpack catalogue")).toBeInTheDocument();
    expect(screen.getByText("Active")).toBeInTheDocument();
    expect(screen.getByText("Completed")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Clear finished" }));

    await waitFor(() => expect(clearFinishedActivity).toHaveBeenCalledOnce());
    expect(screen.queryByText("Public modpack catalogue")).not.toBeInTheDocument();
    expect(screen.getByText("Minecraft file verification")).toBeInTheDocument();
    expect(onNotice).toHaveBeenCalledWith(expect.stringContaining("Active operations were preserved"));
  });
});
