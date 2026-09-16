import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { inspectMinecraftMetadata } from "../api";
import type { MinecraftMetadataInspection } from "../types";
import { MinecraftMetadataInspector } from "./MinecraftMetadataInspector";

vi.mock("../api", () => ({ inspectMinecraftMetadata: vi.fn() }));
const result: MinecraftMetadataInspection = {
  profileId: "minecraft_main", directory: "C:\\Pack", gameVersion: "1.21.1", modLoader: "neoforge-21.1.248", canUse: true,
  sources: [{ fileName: "minecraftinstance.json", instanceMetadata: true, gameVersion: "1.21.1", modLoader: "neoforge-21.1.248" }],
  issues: [], expectedGameVersion: "1.21.1", expectedModLoader: "neoforge-21.1.248", comparison: "matches",
};
const props = { profileId: "minecraft_main", directory: "C:\\Pack", disabled: false, onNotice: vi.fn() };
beforeEach(() => { vi.resetAllMocks(); vi.mocked(inspectMinecraftMetadata).mockResolvedValue(result); });

describe("Read-only Minecraft metadata inspector", () => {
  it("inspects on request and only applies two fields after explicit review", async () => {
    const onUse = vi.fn();
    render(<MinecraftMetadataInspector {...props} onUse={onUse} />);
    expect(inspectMinecraftMetadata).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Inspect Minecraft metadata" }));
    expect(await screen.findByText("Instance metadata matches the published requirement")).toBeInTheDocument();
    expect(inspectMinecraftMetadata).toHaveBeenCalledWith("minecraft_main", "C:\\Pack");
    expect(onUse).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Use detected version and loader" }));
    expect(onUse).toHaveBeenCalledWith("1.21.1", "neoforge-21.1.248");
    expect(props.onNotice).toHaveBeenCalledWith(expect.stringContaining("Nothing was uploaded"));
  });

  it("has no authoring action in the player settings check", async () => {
    render(<MinecraftMetadataInspector {...props} />);
    fireEvent.click(screen.getByRole("button", { name: "Inspect Minecraft metadata" }));
    await screen.findByText("Instance metadata matches the published requirement");
    expect(screen.queryByRole("button", { name: "Use detected version and loader" })).not.toBeInTheDocument();
  });

  it("does not apply ambiguous metadata or imply compatibility", async () => {
    vi.mocked(inspectMinecraftMetadata).mockResolvedValue({ ...result, canUse: false, comparison: "unknown", issues: ["Conflicting export metadata"] });
    const onUse = vi.fn();
    render(<MinecraftMetadataInspector {...props} onUse={onUse} />);
    fireEvent.click(screen.getByRole("button", { name: "Inspect Minecraft metadata" }));
    expect(await screen.findByText("Installed compatibility cannot be confirmed")).toBeInTheDocument();
    expect(screen.getByText("Conflicting export metadata")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Use detected version and loader" })).toBeDisabled();
    expect(onUse).not.toHaveBeenCalled();
  });

  it.each(["folder", "profile"])("discards a late result after changing the %s", async (change) => {
    let finish!: (value: MinecraftMetadataInspection) => void;
    vi.mocked(inspectMinecraftMetadata).mockReturnValue(new Promise((resolve) => { finish = resolve; }));
    const onUse = vi.fn();
    const view = render(<MinecraftMetadataInspector {...props} onUse={onUse} />);
    fireEvent.click(screen.getByRole("button", { name: "Inspect Minecraft metadata" }));
    view.rerender(<MinecraftMetadataInspector {...props} directory={change === "folder" ? "C:\\Other" : props.directory} profileId={change === "profile" ? "other" : props.profileId} onUse={onUse} />);
    await act(async () => { finish(result); });
    expect(screen.queryByText("Instance metadata matches the published requirement")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Use detected version and loader" })).not.toBeInTheDocument();
    expect(onUse).not.toHaveBeenCalled();
  });

  it("clears earlier evidence if a later inspection fails", async () => {
    render(<MinecraftMetadataInspector {...props} />);
    fireEvent.click(screen.getByRole("button", { name: "Inspect Minecraft metadata" }));
    await screen.findByText("Instance metadata matches the published requirement");
    vi.mocked(inspectMinecraftMetadata).mockRejectedValue(new Error("Cannot read metadata"));
    fireEvent.click(screen.getByRole("button", { name: "Inspect Minecraft metadata" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Cannot read metadata");
    expect(screen.queryByText("Instance metadata matches the published requirement")).not.toBeInTheDocument();
    await waitFor(() => expect(screen.getByRole("button", { name: "Inspect Minecraft metadata" })).toBeEnabled());
  });
});
