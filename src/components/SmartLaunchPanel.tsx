import { useState } from "react";
import {
  ArrowLeft,
  CheckCircle2,
  Gamepad2,
  HardDrive,
  RefreshCw,
  Rocket,
  ShieldAlert,
  ShieldCheck,
  Wrench,
} from "lucide-react";
import type {
  FileVerification,
  GameProfile,
  LaunchOutcome,
  ManifestSummary,
  ProfileHealth,
  TransactionKind,
  TransactionOutcome,
  TransactionPreview,
  TransactionRequest,
} from "../types";

type SmartLaunchPhase =
  | "idle"
  | "checking"
  | "staging"
  | "review"
  | "applying"
  | "rechecking"
  | "launched"
  | "blocked"
  | "failed";

interface SmartLaunchPanelProps {
  profile: GameProfile;
  health: ProfileHealth;
  manifest: ManifestSummary;
  onBack: () => void;
  onNotice: (message: string) => void;
  onVerify: (profileId: string) => Promise<FileVerification>;
  onPrepare: (request: TransactionRequest) => Promise<TransactionPreview>;
  onApply: (previewId: string, confirmed: boolean) => Promise<TransactionOutcome>;
  onRefresh: () => Promise<void>;
  onLaunch: (profileId: string) => Promise<LaunchOutcome>;
}

export function SmartLaunchPanel({
  profile,
  health,
  manifest,
  onBack,
  onNotice,
  onVerify,
  onPrepare,
  onApply,
  onRefresh,
  onLaunch,
}: SmartLaunchPanelProps) {
  const [phase, setPhase] = useState<SmartLaunchPhase>("idle");
  const [verification, setVerification] = useState<FileVerification | null>(null);
  const [preview, setPreview] = useState<TransactionPreview | null>(null);
  const [outcome, setOutcome] = useState<TransactionOutcome | null>(null);
  const [confirmed, setConfirmed] = useState(false);
  const [message, setMessage] = useState("");

  const configured = Boolean(
    profile.installDir &&
    profile.gameExePath &&
    manifest.valid &&
    manifest.requiredFileCount > 0,
  );
  const busy = ["checking", "staging", "applying", "rechecking"].includes(phase);

  async function checkAndStage() {
    setPreview(null);
    setOutcome(null);
    setConfirmed(false);
    setMessage("");

    if (!configured) {
      block("Smart Launch needs a game or launcher path, a modpack folder, and a trusted manifest with tracked files.");
      return;
    }

    try {
      setPhase("checking");
      const checked = await onVerify(profile.id);
      setVerification(checked);

      if (checked.unsafeEntries.length > 0) {
        block(`The trusted manifest contains ${checked.unsafeEntries.length} unsafe path${checked.unsafeEntries.length === 1 ? "" : "s"}. No live files were changed and the game was not opened.`);
        return;
      }

      const decision = decideSmartLaunch(profile, manifest, checked);
      if (decision === "launch") {
        await openVerifiedGame(checked);
        return;
      }

      setPhase("staging");
      const candidate = await onPrepare({ profileId: profile.id, kind: decision });
      setPreview(candidate);
      setMessage(candidate.message);
      onNotice(candidate.message);

      if (candidate.ready) {
        setPhase("review");
        return;
      }

      if (candidate.nothingToDo) {
        await recheckAndLaunch();
        return;
      }

      block(candidate.issues[0] || candidate.message || "The maintenance candidate could not be verified.");
    } catch (error) {
      fail(errorMessage(error));
    }
  }

  async function applyAndLaunch() {
    if (!preview || !confirmed) return;

    try {
      setPhase("applying");
      const applied = await onApply(preview.previewId, true);
      setOutcome(applied);
      setConfirmed(false);
      onNotice(applied.message);

      if (!applied.success) {
        const detail = applied.rolledBack
          ? `${applied.message} The previous installation was restored.`
          : applied.error || applied.message;
        block(detail);
        return;
      }

      await onRefresh();
      await recheckAndLaunch();
    } catch (error) {
      fail(errorMessage(error));
    }
  }

  async function recheckAndLaunch() {
    setPhase("rechecking");
    const checked = await onVerify(profile.id);
    setVerification(checked);
    const failures = verificationFailures(checked);

    if (failures > 0) {
      block(`The final integrity check still found ${failures} file${failures === 1 ? "" : "s"} needing attention. The game was not opened.`);
      return;
    }

    await openVerifiedGame(checked);
  }

  async function openVerifiedGame(checked: FileVerification) {
    const failures = verificationFailures(checked);
    if (failures > 0) {
      block(`Verification found ${failures} file${failures === 1 ? "" : "s"} needing attention. The game was not opened.`);
      return;
    }

    const launched = await onLaunch(profile.id);
    setPhase("launched");
    setMessage(launched.message);
    onNotice(launched.message);
  }

  function block(reason: string) {
    setPhase("blocked");
    setMessage(reason);
    onNotice(reason);
  }

  function fail(reason: string) {
    setPhase("failed");
    setMessage(reason);
    onNotice(reason);
  }

  return (
    <main className="settings-page smart-launch-page">
      <div className="settings-header">
        <button className="back-button" onClick={onBack} disabled={busy}><ArrowLeft size={18} /> Back</button>
        <div><span className="eyebrow">CHECK · MAINTAIN · VERIFY · OPEN</span><h1>Smart Launch</h1></div>
        <div className={`readiness-pill ${health.status}`}><i /> {healthLabel(health.status)}</div>
      </div>

      <div className="settings-layout">
        <section className="settings-section panel-card smart-launch-hero">
          <div className="section-title">
            <Rocket />
            <div>
              <h2>One safe path from modpack health to game launch</h2>
              <p>Smart Launch verifies every tracked file, stages only the required maintenance, and opens the game or selected Minecraft launcher only after a clean final check.</p>
            </div>
          </div>
          <div className="smart-pipeline" aria-label="Smart Launch sequence">
            {(["Check", "Stage", "Apply", "Recheck", "Open"] as const).map((step, index) => (
              <span key={step} className={pipelineClass(phase, index)}>{step}</span>
            ))}
          </div>
          <div className="safety-note"><ShieldCheck size={15} /> Nothing writes to the live modpack unless a verified maintenance candidate is shown here and you explicitly approve it.</div>
        </section>

        {!configured && (
          <section className="settings-section panel-card transaction-warning">
            <ShieldAlert />
            <div><h2>Setup is required first</h2><p>Configure the game or launcher path, existing modpack folder, and trusted manifest before using Smart Launch.</p></div>
          </section>
        )}

        <section className="settings-section panel-card smart-launch-action">
          <Gamepad2 />
          <div>
            <span className="eyebrow">{launchTarget(profile)}</span>
            <h2>{phase === "idle" ? "Ready for a complete preflight" : phaseTitle(phase)}</h2>
            <p>{message || "No live file will be changed during the initial check. If maintenance is necessary, you will review it before anything is applied."}</p>
          </div>
          <button className="primary-action" onClick={() => void checkAndStage()} disabled={busy || phase === "review"}>
            {busy ? <RefreshCw className="spin" size={17} /> : <Rocket size={17} />}
            {busy ? busyLabel(phase) : phase === "launched" ? "Check and launch again" : "Check and Smart Launch"}
          </button>
        </section>

        {verification && (
          <section className={`settings-section panel-card smart-check-result ${verificationFailures(verification) === 0 ? "success" : "attention"}`}>
            {verificationFailures(verification) === 0 ? <CheckCircle2 /> : <Wrench />}
            <div>
              <h2>{verificationFailures(verification) === 0 ? "Tracked files verified" : "Maintenance identified"}</h2>
              <p>{verification.current} of {verification.checked} files are current · {verification.missing.length} missing · {verification.changed.length} changed · {verification.unsafeEntries.length} unsafe</p>
            </div>
          </section>
        )}

        {preview && (
          <section className={`settings-section panel-card transaction-preview ${preview.ready ? "ready" : "nothing"}`}>
            <div className="section-title">
              {preview.ready ? <ShieldCheck /> : <CheckCircle2 />}
              <div><h2>{preview.ready ? `${titleKind(preview.kind)} candidate verified` : "No live changes are required"}</h2><p>{preview.message}</p></div>
            </div>
            <dl className="pack-facts preview-facts">
              <div><dt>Action</dt><dd>{titleKind(preview.kind)}</dd></div>
              <div><dt>Target version</dt><dd>{preview.version}</dd></div>
              <div><dt>Staged payload</dt><dd>{preview.stagedFiles.toLocaleString()} files · {formatBytes(preview.stagedBytes)}</dd></div>
              <div><dt>Existing files backed up</dt><dd>{preview.existingFilesToBackup.toLocaleString()}</dd></div>
              <div><dt>New files journaled</dt><dd>{preview.newFiles.toLocaleString()}</dd></div>
              <div><dt>Obsolete live paths</dt><dd>{preview.obsoletePaths.toLocaleString()}</dd></div>
            </dl>
            {preview.ready && (
              <>
                <div className="safety-note transaction-backup-note"><HardDrive size={15} /> A validated restore point is created immediately before the first live write. The existing rollback system protects partial failures.</div>
                <label className="confirmation-row">
                  <input type="checkbox" checked={confirmed} onChange={(event) => setConfirmed(event.target.checked)} />
                  <span>I confirm that Smart Launch may back up and transactionally modify this modpack using the reviewed candidate.</span>
                </label>
                <button className="primary-action danger-action" onClick={() => void applyAndLaunch()} disabled={!confirmed || busy}>
                  {busy ? <RefreshCw className="spin" size={17} /> : <ShieldCheck size={17} />} Apply, recheck and launch
                </button>
              </>
            )}
          </section>
        )}

        {outcome?.success && (
          <section className="settings-section panel-card transaction-outcome success">
            <CheckCircle2 />
            <div><h2>{titleKind(outcome.kind)} applied safely</h2><p>{outcome.message}</p><small>{outcome.applied.length} applied · {outcome.removed.length} removed · final verification follows</small></div>
          </section>
        )}

        {phase === "launched" && (
          <section className="settings-section panel-card smart-launched">
            <Gamepad2 />
            <div><h2>Verified launch complete</h2><p>{message}</p><small>Smart Launch does not discover, configure, start, stop, or join game servers.</small></div>
          </section>
        )}

        {(phase === "blocked" || phase === "failed") && (
          <section className="settings-section panel-card transaction-outcome failed">
            <ShieldAlert />
            <div><h2>{phase === "blocked" ? "Manual action required" : "Smart Launch could not complete"}</h2><p>{message}</p><small>No game was opened by this attempt.</small></div>
          </section>
        )}
      </div>
    </main>
  );
}

export function decideSmartLaunch(
  profile: GameProfile,
  manifest: ManifestSummary,
  verification: FileVerification,
): TransactionKind | "launch" {
  if (profile.localModpackVersion !== manifest.modpackVersion) return "update";
  return verification.missing.length > 0 || verification.changed.length > 0
    ? "repair"
    : "launch";
}

function verificationFailures(verification: FileVerification): number {
  return verification.missing.length + verification.changed.length + verification.unsafeEntries.length;
}

function pipelineClass(phase: SmartLaunchPhase, index: number): string {
  const activeIndex: Record<SmartLaunchPhase, number> = {
    idle: -1,
    checking: 0,
    staging: 1,
    review: 1,
    applying: 2,
    rechecking: 3,
    launched: 4,
    blocked: -1,
    failed: -1,
  };
  const active = activeIndex[phase];
  if (active === index) return "active";
  if (active > index || phase === "launched") return "complete";
  return "";
}

function phaseTitle(phase: SmartLaunchPhase): string {
  const titles: Record<SmartLaunchPhase, string> = {
    idle: "Ready for a complete preflight",
    checking: "Checking every tracked file",
    staging: "Staging trusted maintenance",
    review: "Review required before live changes",
    applying: "Applying the confirmed transaction",
    rechecking: "Running the final integrity check",
    launched: "Game or launcher opened",
    blocked: "Launch paused safely",
    failed: "Launch attempt stopped",
  };
  return titles[phase];
}

function busyLabel(phase: SmartLaunchPhase): string {
  if (phase === "checking") return "Checking files…";
  if (phase === "staging") return "Staging safely…";
  if (phase === "applying") return "Applying safely…";
  return "Rechecking files…";
}

function launchTarget(profile: GameProfile): string {
  if (profile.game !== "minecraft") return profile.displayName;
  if (profile.minecraftLauncher === "curseforge") return "CURSEFORGE";
  if (profile.minecraftLauncher === "modrinth") return "MODRINTH";
  return "MINECRAFT LAUNCHER";
}

function titleKind(kind: TransactionKind): string {
  return kind === "update" ? "Update" : "Repair";
}

function healthLabel(status: ProfileHealth["status"]): string {
  const labels: Record<ProfileHealth["status"], string> = {
    ready: "READY",
    updateRequired: "UPDATE REQUIRED",
    repairNeeded: "REPAIR NEEDED",
    gamePathMissing: "PATH MISSING",
    setupRequired: "SETUP REQUIRED",
    checking: "CHECKING",
    failed: "CHECK FAILED",
  };
  return labels[status];
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let index = 0;
  while (value >= 1024 && index < units.length - 1) {
    value /= 1024;
    index += 1;
  }
  return `${value.toFixed(value >= 100 ? 0 : 1)} ${units[index]}`;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
