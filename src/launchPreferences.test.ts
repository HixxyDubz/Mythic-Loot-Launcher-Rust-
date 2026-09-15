import { beforeEach, describe, expect, it, vi } from "vitest";
const { invoke, close } = vi.hoisted(() => ({ invoke: vi.fn(), close: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke, isTauri: () => true }));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => ({ close }) }));
import { launchProfile } from "./api";

beforeEach(() => { vi.resetAllMocks(); });
describe("Close after normal launch", () => {
  it("closes only after a successful native launch requests it", async () => {
    invoke.mockResolvedValue({ pid: 12, message: "Started", closeAfterLaunch: true });
    await launchProfile("minecraft_main");
    expect(close).toHaveBeenCalledOnce();
  });
  it("stays open on launch failure or a disabled preference", async () => {
    invoke.mockRejectedValueOnce(new Error("Not ready"));
    await expect(launchProfile("minecraft_main")).rejects.toThrow("Not ready");
    invoke.mockResolvedValueOnce({ pid: 12, message: "Started", closeAfterLaunch: false });
    await launchProfile("minecraft_main");
    expect(close).not.toHaveBeenCalled();
  });
  it("does not misreport a started game as failed when closing is unavailable", async () => {
    invoke.mockResolvedValue({ pid: 12, message: "Started", closeAfterLaunch: true });
    close.mockRejectedValue(new Error("Window not available"));
    const result = await launchProfile("minecraft_main");
    expect(result.pid).toBe(12);
    expect(result.message).toContain("could not close automatically");
  });
});
