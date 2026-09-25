import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { inspectGameInstallation } from "../api";
import { testProfiles } from "../test/fixtures";
import type { GameInstallationCheck } from "../types";
import { GameInstallationInspector } from "./GameInstallationInspector";

vi.mock("../api", () => ({ inspectGameInstallation: vi.fn() }));
const result: GameInstallationCheck = { profileId: "seven_days_main", game: "seven_days", paths: [{ label: "Game directory", path: "C:\\Game", status: "present" }], clientFound: true, gameVersion: null, requiredGameVersion: "3.1", comparison: "unknown", steamBuildId: "123456", notes: ["Read-only evidence"] };
beforeEach(() => { vi.resetAllMocks(); vi.mocked(inspectGameInstallation).mockResolvedValue(result); });
describe("Local installation evidence", () => {
  it("runs only on request and never presents a Steam build as the game version", async () => {
    render(<GameInstallationInspector profile={testProfiles[1]} disabled={false} />);
    expect(inspectGameInstallation).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Check local game installation" }));
    expect(await screen.findByText("123456")).toBeInTheDocument();
    expect(screen.getByText("Cannot determine")).toBeInTheDocument();
    expect(screen.getByText("Game version compatibility cannot be confirmed")).toBeInTheDocument();
    expect(inspectGameInstallation).toHaveBeenCalledWith(testProfiles[1]);
  });
  it.each(["gameDir", "installDir", "gameExePath"] as const)("discards pending results after the %s selection changes", async (field) => {
    let finish!: (value: GameInstallationCheck) => void;
    vi.mocked(inspectGameInstallation).mockReturnValue(new Promise((resolve) => { finish = resolve; }));
    const view = render(<GameInstallationInspector profile={testProfiles[1]} disabled={false} />);
    fireEvent.click(screen.getByRole("button", { name: "Check local game installation" }));
    view.rerender(<GameInstallationInspector profile={{ ...testProfiles[1], [field]: "C:\\Other" }} disabled={false} />);
    await act(async () => { finish(result); });
    expect(screen.queryByText("123456")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Check local game installation" })).toBeEnabled();
  });
  it("clears earlier evidence when the next inspection fails", async () => {
    render(<GameInstallationInspector profile={testProfiles[1]} disabled={false} />);
    fireEvent.click(screen.getByRole("button", { name: "Check local game installation" })); await screen.findByText("123456");
    vi.mocked(inspectGameInstallation).mockRejectedValue(new Error("Inspection failed"));
    fireEvent.click(screen.getByRole("button", { name: "Check local game installation" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Inspection failed");
    expect(screen.queryByText("123456")).not.toBeInTheDocument();
  });
  it("shows native mismatch and unsafe-path findings without offering to change settings", async () => {
    vi.mocked(inspectGameInstallation).mockResolvedValue({ ...result, gameVersion: "2.0.1", comparison: "mismatch", paths: [{ label: "Game directory", path: "relative", status: "unsafe" }] });
    render(<GameInstallationInspector profile={testProfiles[1]} disabled={false} />);
    fireEvent.click(screen.getByRole("button", { name: "Check local game installation" }));
    expect(await screen.findByText("Declared game version differs from the published requirement")).toBeInTheDocument();
    expect(screen.getByText(/Unsafe or unreadable path/)).toBeInTheDocument();
    expect(screen.getAllByRole("button")).toHaveLength(1);
  });
});
