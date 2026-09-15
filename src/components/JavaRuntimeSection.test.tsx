import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { JavaRuntimeSection } from "./JavaRuntimeSection";
import { detectJavaRuntimes, prepareJavaArguments } from "../api";
import { testProfiles } from "../test/fixtures";

vi.mock("../api", () => ({ detectJavaRuntimes: vi.fn(), prepareJavaArguments: vi.fn() }));
beforeEach(() => vi.clearAllMocks());
const direct = { ...testProfiles[0], gameExePath: "C:\\Java\\bin\\java.exe", launchArgs: "-cp libraries Main" };

describe("Minecraft runtime controls", () => {
  it.each(["curseforge", "modrinth"])("does not expose direct-Java memory mutation for %s", (minecraftLauncher) => {
    render(<JavaRuntimeSection profile={{ ...direct, minecraftLauncher }} busy={false} onChange={vi.fn()} onNotice={vi.fn()} />);
    expect(screen.queryByRole("button", { name: /update launch arguments/i })).not.toBeInTheDocument();
    expect(screen.getByText(/set memory in that launcher/i)).toBeInTheDocument();
  });

  it("applies memory through the native editor into the unsaved profile draft", async () => {
    const change = vi.fn();
    vi.mocked(prepareJavaArguments).mockResolvedValue('"-cp" "libraries" "-Xms3072M" "-Xmx6144M" "Main"');
    render(<JavaRuntimeSection profile={direct} busy={false} onChange={change} onNotice={vi.fn()} />);
    fireEvent.change(screen.getByLabelText("Memory preset to apply"), { target: { value: "6144" } });
    fireEvent.click(screen.getByRole("button", { name: /update launch arguments/i }));
    await waitFor(() => expect(change).toHaveBeenCalledWith({ ...direct, launchArgs: '"-cp" "libraries" "-Xms3072M" "-Xmx6144M" "Main"' }));
    expect(prepareJavaArguments).toHaveBeenCalledWith(direct, 6144);
  });

  it("switches to direct Java only through an explicit unsaved draft change", () => {
    const change = vi.fn();
    const profile = { ...direct, minecraftLauncher: "curseforge", gameExePath: "C:\\CurseForge.exe", launchArgs: "--profile existing" };
    render(<JavaRuntimeSection profile={profile} busy={false} onChange={change} onNotice={vi.fn()} />);
    expect(change).not.toHaveBeenCalled();
    fireEvent.click(screen.getByText("Advanced: switch to direct Java"));
    fireEvent.click(screen.getByRole("button", { name: /clear launch target/i }));
    expect(change).toHaveBeenCalledWith({ ...profile, minecraftLauncher: "", gameExePath: "", launchArgs: "" });
  });

  it("rejects stale editor results after another field changes", async () => {
    let finish!: (value: string) => void;
    vi.mocked(prepareJavaArguments).mockReturnValue(new Promise((resolve) => { finish = resolve; }));
    const change = vi.fn(); const notice = vi.fn();
    const view = render(<JavaRuntimeSection profile={direct} busy={false} onChange={change} onNotice={notice} />);
    fireEvent.click(screen.getByRole("button", { name: /update launch arguments/i }));
    view.rerender(<JavaRuntimeSection profile={{ ...direct, launchArgs: "NewMain" }} busy={false} onChange={change} onNotice={notice} />);
    finish("old arguments");
    await waitFor(() => expect(notice).toHaveBeenCalledWith(expect.stringContaining("settings changed")));
    expect(change).not.toHaveBeenCalled();
  });

  it("shows an empty real scan instead of invented runtimes", async () => {
    vi.mocked(detectJavaRuntimes).mockResolvedValue({ runtimes: [], limited: false });
    render(<JavaRuntimeSection profile={direct} busy={false} onChange={vi.fn()} onNotice={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: /find installed java/i }));
    expect(await screen.findByText(/no java runtimes found/i)).toBeInTheDocument();
  });
});
