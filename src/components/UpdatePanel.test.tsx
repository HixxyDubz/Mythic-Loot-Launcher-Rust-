import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import type { ComponentProps } from "react";
import { UpdatePanel } from "./UpdatePanel";
import { testProfiles } from "../test/fixtures";
import type { RestorePointSummary, TransactionPreview } from "../types";

const api = vi.hoisted(() => ({
  listRestorePoints: vi.fn(), prepareModpackTransaction: vi.fn(), applyModpackTransaction: vi.fn(),
  applyRestorePoint: vi.fn(), deleteRestorePoint: vi.fn(), prepareRestorePoint: vi.fn(),
}));
vi.mock("../api", () => api);

const first = { ...testProfiles[0], installDir: "C:\\Review\\PackA" };
const second = { ...testProfiles[1], installDir: "C:\\Review\\PackB" };
const props: ComponentProps<typeof UpdatePanel> = {
  profile: first,
  health: { profileId: first.id, status: "updateRequired", headline: "", details: [] },
  manifest: { profileId: first.id, valid: true, manifestVersion: "1.0", modpackVersion: "1.0.1", releaseDate: "", requiredFileCount: 1, optionalFileCount: 0, obsoleteFileCount: 0, updateSize: 1, source: "", errors: [], announcement: "", newsBannerUrl: "", rulesGuide: { howToJoin: "", rules: [], commonFixes: [] }, changelog: [] },
  onBack: vi.fn(), onNotice: vi.fn(), onCompleted: vi.fn(async () => {}),
};
const preview: TransactionPreview = {
  previewId: "pack-a-plan", profileId: first.id, kind: "update", version: "1.0.1", source: "", stagedFiles: 1, stagedBytes: 1, existingFilesToBackup: 1, newFiles: 0, obsoletePaths: 0, issues: [], ready: true, nothingToDo: false, message: "Pack A staged",
};
const point: RestorePointSummary = { backupId: "point-a", profileId: first.id, label: "pre_update", createdAt: 1, sizeBytes: 1, fileCount: 1, removesOnRestore: 0, localModpackVersion: "1.0.0", valid: true, issues: [] };

beforeEach(() => {
  vi.clearAllMocks();
  api.listRestorePoints.mockResolvedValue([point]);
  api.prepareModpackTransaction.mockResolvedValue(preview);
  api.applyModpackTransaction.mockImplementation(() => new Promise(() => {}));
  api.applyRestorePoint.mockImplementation(() => new Promise(() => {}));
});

it("invalidates confirmation when switching profile or installation folder", async () => {
  const view = render(<UpdatePanel {...props} />);
  fireEvent.click(screen.getByRole("button", { name: /prepare update safely/i }));
  await screen.findByRole("button", { name: /apply verified update/i });
  fireEvent.click(screen.getByRole("checkbox", { name: /I confirm that the launcher may back up/i }));
  view.rerender(<UpdatePanel {...props} profile={second} />);
  expect(screen.queryByRole("button", { name: /apply verified update/i })).not.toBeInTheDocument();
  expect(api.applyModpackTransaction).not.toHaveBeenCalled();
  view.rerender(<UpdatePanel {...props} />);
  fireEvent.click(screen.getByRole("button", { name: /prepare update safely/i }));
  await screen.findByRole("button", { name: /apply verified update/i });
  view.rerender(<UpdatePanel {...props} profile={{ ...first, installDir: "C:\\Review\\Different" }} />);
  expect(screen.queryByRole("button", { name: /apply verified update/i })).not.toBeInTheDocument();
});

it("ignores a staging response arriving after the profile changed", async () => {
  let resolve!: (value: TransactionPreview) => void;
  api.prepareModpackTransaction.mockImplementation(() => new Promise<TransactionPreview>((done) => { resolve = done; }));
  const view = render(<UpdatePanel {...props} />);
  fireEvent.click(screen.getByRole("button", { name: /prepare update safely/i }));
  view.rerender(<UpdatePanel {...props} profile={second} />);
  await act(async () => resolve(preview));
  expect(screen.queryByText("Pack A staged")).not.toBeInTheDocument();
  expect(props.onNotice).not.toHaveBeenCalledWith("Pack A staged");
});

it("blocks restore controls while applying and passes the reviewed profile identity", async () => {
  render(<UpdatePanel {...props} />);
  await screen.findByRole("button", { name: /review restore/i });
  fireEvent.click(screen.getByRole("button", { name: /prepare update safely/i }));
  await screen.findByRole("button", { name: /apply verified update/i });
  fireEvent.click(screen.getByRole("checkbox", { name: /I confirm that the launcher may back up/i }));
  fireEvent.click(screen.getByRole("button", { name: /apply verified update/i }));
  expect(api.applyModpackTransaction).toHaveBeenCalledWith(preview.previewId, true, first.id);
  expect(screen.getByRole("button", { name: /review restore/i })).toBeDisabled();
  expect(screen.getByRole("button", { name: /delete pre update/i })).toBeDisabled();
  expect(screen.getByRole("button", { name: /back/i })).toBeDisabled();
});

it("blocks update and repair while applying a restore", async () => {
  api.prepareRestorePoint.mockResolvedValue({ ...preview, backupId: point.backupId, label: point.label,
    createdAt: 1, localModpackVersion: "1.0.0", filesToRemove: 0 });
  render(<UpdatePanel {...props} />);
  fireEvent.click(await screen.findByRole("button", { name: /review restore/i }));
  await screen.findByRole("button", { name: /restore verified point/i });
  fireEvent.click(screen.getByRole("checkbox", { name: /I confirm that the launcher may create/i }));
  fireEvent.click(screen.getByRole("button", { name: /restore verified point/i }));
  expect(api.applyRestorePoint).toHaveBeenCalledWith(preview.previewId, true, first.id);
  expect(screen.getByRole("button", { name: /prepare update safely/i })).toBeDisabled();
  expect(screen.getByRole("button", { name: /prepare changed files only/i })).toBeDisabled();
});
