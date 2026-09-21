import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { applyContentRecovery, inspectContentRecovery } from "../api";
import { testBootstrapPayload } from "../test/fixtures";
import type { ContentRecoveryState } from "../types";
import { ContentRecoveryPanel } from "./ContentRecoveryPanel";

vi.mock("../api", () => ({ applyContentRecovery: vi.fn(), inspectContentRecovery: vi.fn() }));
const content = { announcement: "Older reviewed news", newsBannerUrl: "", rulesGuide: { howToJoin: "", rules: [], commonFixes: [] }, changelog: [] };
const review: ContentRecoveryState = { profileId: "minecraft_main", draftRevision: "old-hash", hasDraft: true, limited: false, candidates: [{ id: "legacy-reviewed", label: "Cached manifest content", content, problem: null }] };
const props = { profileId: "minecraft_main", disabled: false, onBusy: vi.fn(), onApplied: vi.fn(), onNotice: vi.fn() };
beforeEach(() => { vi.resetAllMocks(); vi.mocked(inspectContentRecovery).mockResolvedValue(structuredClone(review)); vi.mocked(applyContentRecovery).mockResolvedValue({ changed: true, payload: testBootstrapPayload() }); });
async function open() { fireEvent.click(screen.getByRole("button", { name: "Review saved content recovery" })); await screen.findByLabelText("Content recovery entry"); }

describe("Developer content recovery", () => {
  it("requires explicit selection and confirmation and sends only reviewed identifiers", async () => {
    render(<ContentRecoveryPanel {...props} />);
    expect(inspectContentRecovery).not.toHaveBeenCalled(); await open();
    const recover = screen.getByRole("button", { name: "Recover reviewed content locally" });
    expect(recover).toBeDisabled();
    fireEvent.change(screen.getByLabelText("Content recovery entry"), { target: { value: "legacy-reviewed" } });
    expect(screen.getByLabelText("Reviewed content preview")).toHaveTextContent("Older reviewed news");
    expect(recover).toBeDisabled();
    fireEvent.click(screen.getByRole("checkbox")); fireEvent.click(recover);
    await waitFor(() => expect(applyContentRecovery).toHaveBeenCalledWith("minecraft_main", "legacy-reviewed", "old-hash", true));
    expect(props.onApplied).toHaveBeenCalled();
    expect(props.onNotice).toHaveBeenCalledWith(expect.stringContaining("Nothing was published"));
  });
  it("discards only after confirmation and explains the retained recovery copy", async () => {
    render(<ContentRecoveryPanel {...props} />); await open();
    const discard = screen.getByRole("button", { name: "Discard saved draft with recovery copy" });
    expect(discard).toBeDisabled(); fireEvent.click(screen.getByRole("checkbox")); fireEvent.click(discard);
    await waitFor(() => expect(applyContentRecovery).toHaveBeenCalledWith("minecraft_main", null, "old-hash", true));
    expect(props.onNotice).toHaveBeenCalledWith(expect.stringContaining("recovery copy was retained"));
  });
  it("requires a fresh review after failure and does not replace editor content", async () => {
    vi.mocked(applyContentRecovery).mockRejectedValue(new Error("Saved draft changed since review"));
    render(<ContentRecoveryPanel {...props} />); await open();
    fireEvent.click(screen.getByRole("checkbox")); fireEvent.click(screen.getByRole("button", { name: "Discard saved draft with recovery copy" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("changed since review");
    expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
    expect(props.onApplied).not.toHaveBeenCalled();
  });
  it("resets confirmation when choosing another entry and refuses damaged recovery", async () => {
    vi.mocked(inspectContentRecovery).mockResolvedValue({ ...review, candidates: [...review.candidates, { id: "draft-broken", label: "Damaged draft", content: null, problem: "Cannot recover damaged bytes automatically" }] });
    render(<ContentRecoveryPanel {...props} />); await open();
    fireEvent.click(screen.getByRole("checkbox"));
    fireEvent.change(screen.getByLabelText("Content recovery entry"), { target: { value: "draft-broken" } });
    expect(screen.getByRole("checkbox")).not.toBeChecked();
    expect(screen.getByRole("alert")).toHaveTextContent("damaged bytes");
    fireEvent.click(screen.getByRole("checkbox"));
    expect(screen.getByRole("button", { name: "Recover reviewed content locally" })).toBeDisabled();
  });
  it("does not invent history or allow discarding a nonexistent draft", async () => {
    vi.mocked(inspectContentRecovery).mockResolvedValue({ ...review, hasDraft: false, draftRevision: "absent", candidates: [] });
    render(<ContentRecoveryPanel {...props} />); await open();
    expect(screen.getByText(/No recovery copies are available yet/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("checkbox"));
    expect(screen.getByRole("button", { name: "Discard saved draft with recovery copy" })).toBeDisabled();
  });
  it("ignores a recovery response after leaving the editor", async () => {
    let finish!: (result: { changed: boolean; payload: ReturnType<typeof testBootstrapPayload> }) => void;
    vi.mocked(applyContentRecovery).mockReturnValue(new Promise((resolve) => { finish = resolve; }));
    const view = render(<ContentRecoveryPanel {...props} />); await open();
    fireEvent.click(screen.getByRole("checkbox")); fireEvent.click(screen.getByRole("button", { name: "Discard saved draft with recovery copy" }));
    view.unmount();
    await act(async () => { finish({ changed: true, payload: testBootstrapPayload() }); });
    expect(props.onApplied).not.toHaveBeenCalled(); expect(props.onNotice).not.toHaveBeenCalled();
  });
});
