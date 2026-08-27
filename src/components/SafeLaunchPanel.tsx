import { useEffect, useState } from "react";
import {
  ArrowLeft,
  CheckCircle2,
  Clock3,
  Gamepad2,
  HardDrive,
  RefreshCw,
  RotateCcw,
  ShieldAlert,
  ShieldCheck,
  Sparkles,
} from "lucide-react";
import { getSafeLaunchStatus, recoverSafeLaunch, startSafeLaunch } from "../api";
import type { GameProfile, ManifestSummary, ProfileHealth, SafeLaunchStatus } from "../types";

interface SafeLaunchPanelProps {
  profile: GameProfile;
  health: ProfileHealth;
  manifest: ManifestSummary;
  onBack: () => void;
  onNotice: (message: string) => void;
}

export function SafeLaunchPanel({
  profile,
  health,
  manifest,
  onBack,
  onNotice,
}: SafeLaunchPanelProps) {
  const [status, setStatus] = useState<SafeLaunchStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [startConfirmed, setStartConfirmed] = useState(false);
  const [recoveryConfirmed, setRecoveryConfirmed] = useState(false);

  useEffect(() => {
    setStatus(null);
    setStartConfirmed(false);
    setRecoveryConfirmed(false);
    void refreshStatus();
  }, [profile.id]);

  useEffect(() => {
    if (!status?.active) return;
    const timer = window.setInterval(() => void refreshStatus(), 3000);
    return () => window.clearInterval(timer);
  }, [status?.active, profile.id]);

  async function refreshStatus() {
    try {
      setStatus(await getSafeLaunchStatus(profile.id));
    } catch (error) {
      onNotice(errorMessage(error));
    }
  }

  async function start() {
    setBusy(true);
    try {
      const result = await startSafeLaunch(profile.id, startConfirmed);
      onNotice(result.message);
      setStartConfirmed(false);
      await refreshStatus();
    } catch (error) {
      onNotice(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function recover() {
    setBusy(true);
    try {
      const result = await recoverSafeLaunch(profile.id, recoveryConfirmed);
      onNotice(result.message);
      setRecoveryConfirmed(false);
      await refreshStatus();
    } catch (error) {
      onNotice(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  const ready = health.status === "ready";
  const supported = manifest.optionalFileCount > 0;

  return (
    <main className="settings-page safe-launch-page">
      <div className="settings-header">
        <button className="back-button" onClick={onBack} disabled={busy}><ArrowLeft size={18} /> Back</button>
        <div><span className="eyebrow">TROUBLESHOOTING MODE</span><h1>Safe Launch</h1></div>
        <div className={`readiness-pill ${status?.active ? status.gameProcessRunning ? "checking" : "repairNeeded" : health.status}`}><i /> {status?.active ? status.gameProcessRunning ? "GAME RUNNING" : "RECOVERY READY" : ready ? "READY" : "SETUP REQUIRED"}</div>
      </div>

      <div className="settings-layout">
        <section className="settings-section panel-card safe-launch-hero">
          <div className="section-title"><ShieldCheck /><div><h2>Launch without optional extras</h2><p>Safe Launch temporarily moves only manifest-declared optional files aside, starts the configured game, waits for that exact process to exit, then restores and verifies every moved file.</p></div></div>
          <div className="transaction-pipeline safe-launch-pipeline" aria-label="Safe Launch safety sequence">
            {['Journal', 'Move', 'Launch', 'Wait', 'Restore', 'Verify'].map((step, index) => <span key={`${step}-${index}`}>{step}</span>)}
          </div>
        </section>

        {status?.active ? (
          <section className={`settings-section panel-card safe-session ${status.gameProcessRunning ? 'running' : 'recoverable'}`}>
            {status.gameProcessRunning ? <Gamepad2 /> : <ShieldAlert />}
            <div className="safe-session-copy">
              <span className="eyebrow">{status.gameProcessRunning ? 'ACTIVE SESSION' : 'INTERRUPTED SESSION'}</span>
              <h2>{status.gameProcessRunning ? 'The game is running in Safe Launch' : 'Optional files are waiting to be restored'}</h2>
              <p>{status.message}</p>
              <dl className="pack-facts safe-session-facts">
                <div><dt>Disabled files</dt><dd>{status.disabledFiles.toLocaleString()}</dd></div>
                <div><dt>Game process</dt><dd>{status.gameProcessId || 'Not started'}</dd></div>
                <div><dt>Started</dt><dd>{formatDate(status.startedAt)}</dd></div>
                <div><dt>Recorded folder</dt><dd title={status.installDir}>{status.installDir}</dd></div>
              </dl>
            </div>
            {status.gameProcessRunning ? (
              <div className="safety-note safe-waiting-note"><Clock3 size={15} /> Keep playing normally. Restoration begins only after the recorded process exits.</div>
            ) : (
              <>
                <div className="safety-note safe-recovery-note"><HardDrive size={15} /> Each disabled copy is checked against the persisted size and SHA-256 before it is moved back. Conflicts stop recovery without overwriting either copy.</div>
                <label className="confirmation-row"><input type="checkbox" checked={recoveryConfirmed} onChange={(event) => setRecoveryConfirmed(event.target.checked)} /><span>I have closed the game and confirm that the launcher may restore the recorded optional files.</span></label>
                <button className="primary-action" onClick={() => void recover()} disabled={!recoveryConfirmed || busy}>
                  {busy ? <RefreshCw className="spin" size={17} /> : <RotateCcw size={17} />} Restore optional files
                </button>
              </>
            )}
          </section>
        ) : (
          <section className="settings-section panel-card safe-launch-start">
            <Sparkles />
            <div>
              <span className="eyebrow">CLEAN TROUBLESHOOTING RUN</span>
              <h2>{supported ? `${manifest.optionalFileCount.toLocaleString()} optional file${manifest.optionalFileCount === 1 ? '' : 's'} can be isolated` : 'No optional extras are declared'}</h2>
              <p>{supported ? 'Required modpack files remain untouched. Optional files are moved within the same installation, never deleted or uploaded.' : 'This trusted manifest currently has no optionalFiles entries, so Safe Launch would be identical to normal Play.'}</p>
            </div>
            {!ready && <div className="safe-prerequisite"><ShieldAlert /><span><strong>The modpack must be ready first</strong><small>Complete setup, update, or repair before starting a troubleshooting session.</small></span></div>}
            {supported && ready && (
              <>
                <label className="confirmation-row"><input type="checkbox" checked={startConfirmed} onChange={(event) => setStartConfirmed(event.target.checked)} /><span>I confirm that Safe Launch may temporarily move the declared optional files and start the configured game.</span></label>
                <button className="primary-action" onClick={() => void start()} disabled={!startConfirmed || busy}>
                  {busy ? <RefreshCw className="spin" size={17} /> : <Gamepad2 size={17} />} Start Safe Launch
                </button>
              </>
            )}
          </section>
        )}

        <section className="settings-section panel-card safe-launch-guarantees">
          <div className="section-title"><CheckCircle2 /><div><h2>Persistent recovery journal</h2><p>If the launcher closes before the game, the next launcher session detects the journal. It checks whether the recorded process still exists and offers recovery only when the game is no longer running.</p></div></div>
          <div className="safe-guarantee-grid">
            <span><ShieldCheck /> Manifest paths only</span>
            <span><HardDrive /> Local files only</span>
            <span><RotateCcw /> Exact-hash recovery</span>
            <span><ShieldAlert /> Conflicts fail closed</span>
          </div>
        </section>
      </div>
    </main>
  );
}

function formatDate(timestamp: number): string {
  if (!timestamp) return 'Not recorded';
  return new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' })
    .format(new Date(timestamp * 1000));
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
