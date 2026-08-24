import { useState } from "react";
import {
  ArrowLeft,
  CheckCircle2,
  Download,
  HardDrive,
  RefreshCw,
  RotateCcw,
  ShieldAlert,
  ShieldCheck,
  Wrench,
} from "lucide-react";
import { applyModpackTransaction, prepareModpackTransaction } from "../api";
import type {
  GameProfile,
  ManifestSummary,
  ProfileHealth,
  TransactionKind,
  TransactionOutcome,
  TransactionPreview,
} from "../types";

interface UpdatePanelProps {
  profile: GameProfile;
  health: ProfileHealth;
  manifest: ManifestSummary;
  onBack: () => void;
  onNotice: (message: string) => void;
  onCompleted: () => Promise<void>;
}

export function UpdatePanel({
  profile,
  health,
  manifest,
  onBack,
  onNotice,
  onCompleted,
}: UpdatePanelProps) {
  const [busy, setBusy] = useState(false);
  const [busyKind, setBusyKind] = useState<TransactionKind | null>(null);
  const [preview, setPreview] = useState<TransactionPreview | null>(null);
  const [confirmed, setConfirmed] = useState(false);
  const [outcome, setOutcome] = useState<TransactionOutcome | null>(null);

  async function prepare(kind: TransactionKind) {
    setBusy(true);
    setBusyKind(kind);
    setPreview(null);
    setConfirmed(false);
    setOutcome(null);
    try {
      const result = await prepareModpackTransaction({ profileId: profile.id, kind });
      setPreview(result);
      onNotice(result.message);
    } catch (error) {
      onNotice(errorMessage(error));
    } finally {
      setBusy(false);
      setBusyKind(null);
    }
  }

  async function apply() {
    if (!preview) return;
    setBusy(true);
    setBusyKind(preview.kind);
    try {
      const result = await applyModpackTransaction(preview.previewId, confirmed);
      setOutcome(result);
      setConfirmed(false);
      onNotice(result.message);
      if (result.success) await onCompleted();
    } catch (error) {
      onNotice(errorMessage(error));
    } finally {
      setBusy(false);
      setBusyKind(null);
    }
  }

  const configured = Boolean(profile.installDir && manifest.valid && manifest.requiredFileCount > 0);

  return (
    <main className="settings-page update-page">
      <div className="settings-header">
        <button className="back-button" onClick={onBack} disabled={busy}><ArrowLeft size={18} /> Back</button>
        <div><span className="eyebrow">SAFE MODPACK MAINTENANCE</span><h1>Update & Repair</h1></div>
        <div className={`readiness-pill ${health.status}`}><i /> {healthLabel(health.status)}</div>
      </div>

      <div className="settings-layout">
        <section className="settings-section panel-card transaction-safety">
          <div className="section-title"><ShieldCheck /><div><h2>Live files stay untouched during preparation</h2><p>The package is downloaded, path-checked, CRC-tested, extracted and SHA-256 verified in isolated launcher storage first.</p></div></div>
          <div className="transaction-pipeline" aria-label="Transaction safety sequence">
            {['Download', 'Stage', 'Verify', 'Backup', 'Apply', 'Verify', 'Rollback if needed'].map((step, index) => (
              <span key={`${step}-${index}`}>{step}</span>
            ))}
          </div>
        </section>

        <section className="transaction-options">
          <article className="settings-section panel-card transaction-choice">
            <Download />
            <div><span className="eyebrow">VERSION CHANGE</span><h2>Prepare update</h2><p>Stages the trusted release package. Unchanged required files may remain safely in the live installation.</p></div>
            <button className="primary-action" onClick={() => void prepare('update')} disabled={!configured || busy}>
              {busyKind === 'update' ? <RefreshCw className="spin" size={17} /> : <Download size={17} />}
              {busyKind === 'update' ? 'Downloading & verifying…' : 'Prepare update safely'}
            </button>
          </article>

          <article className="settings-section panel-card transaction-choice">
            <Wrench />
            <div><span className="eyebrow">FILE HEALTH</span><h2>Prepare repair</h2><p>Compares every required hash, then stages and applies only missing or mismatched files from the trusted package.</p></div>
            <button className="secondary-action" onClick={() => void prepare('repair')} disabled={!configured || busy}>
              {busyKind === 'repair' ? <RefreshCw className="spin" size={17} /> : <Wrench size={17} />}
              {busyKind === 'repair' ? 'Checking & staging…' : 'Prepare changed files only'}
            </button>
          </article>
        </section>

        {!configured && (
          <section className="settings-section panel-card transaction-warning">
            <ShieldAlert /><div><h2>Setup is required first</h2><p>Choose an existing modpack folder and load a valid trusted manifest before preparing maintenance.</p></div>
          </section>
        )}

        {preview && (
          <section className={`settings-section panel-card transaction-preview ${preview.ready ? 'ready' : 'nothing'}`}>
            <div className="section-title">{preview.ready ? <ShieldCheck /> : <CheckCircle2 />}<div><h2>{preview.ready ? `${titleKind(preview.kind)} candidate verified` : 'Nothing needs repair'}</h2><p>{preview.message}</p></div></div>
            <dl className="pack-facts preview-facts">
              <div><dt>Target version</dt><dd>{preview.version}</dd></div>
              <div><dt>Staged payload</dt><dd>{preview.stagedFiles.toLocaleString()} files · {formatBytes(preview.stagedBytes)}</dd></div>
              <div><dt>Existing files backed up</dt><dd>{preview.existingFilesToBackup.toLocaleString()}</dd></div>
              <div><dt>New files journaled</dt><dd>{preview.newFiles.toLocaleString()}</dd></div>
              <div><dt>Obsolete live paths</dt><dd>{preview.obsoletePaths.toLocaleString()}</dd></div>
              {preview.source && <div><dt>Trusted package</dt><dd title={preview.source}>{preview.source}</dd></div>}
            </dl>
            {preview.ready && (
              <>
                <div className="safety-note transaction-backup-note"><HardDrive size={15} /> A validated pre-change backup is created immediately before the first live write. It remains available after success.</div>
                <label className="confirmation-row"><input type="checkbox" checked={confirmed} onChange={(event) => setConfirmed(event.target.checked)} /><span>I confirm that the launcher may back up and transactionally modify this modpack installation using the reviewed candidate.</span></label>
                <button className="primary-action danger-action" onClick={() => void apply()} disabled={!confirmed || busy}>
                  {busy ? <RefreshCw className="spin" size={17} /> : <ShieldCheck size={17} />} Apply verified {preview.kind}
                </button>
              </>
            )}
          </section>
        )}

        {outcome && (
          <section className={`settings-section panel-card transaction-outcome ${outcome.success ? 'success' : outcome.rolledBack ? 'rolled-back' : 'failed'}`}>
            {outcome.success ? <CheckCircle2 /> : outcome.rolledBack ? <RotateCcw /> : <ShieldAlert />}
            <div>
              <h2>{outcome.success ? `${titleKind(outcome.kind)} complete` : outcome.rolledBack ? `${titleKind(outcome.kind)} rolled back safely` : `${titleKind(outcome.kind)} needs attention`}</h2>
              <p>{outcome.message}</p>
              <small>{outcome.applied.length} applied · {outcome.removed.length} removed{outcome.backupPath ? ` · Backup: ${outcome.backupPath}` : ''}</small>
              {outcome.error && <small className="outcome-error">Cause: {outcome.error}</small>}
              {outcome.rollbackError && <small className="outcome-error">Rollback: {outcome.rollbackError}</small>}
            </div>
          </section>
        )}
      </div>
    </main>
  );
}

function titleKind(kind: TransactionKind): string {
  return kind === 'update' ? 'Update' : 'Repair';
}

function healthLabel(status: ProfileHealth['status']): string {
  const labels: Record<ProfileHealth['status'], string> = {
    ready: 'READY',
    updateRequired: 'UPDATE REQUIRED',
    repairNeeded: 'REPAIR NEEDED',
    gamePathMissing: 'PATH MISSING',
    setupRequired: 'SETUP REQUIRED',
    checking: 'CHECKING',
    failed: 'CHECK FAILED',
  };
  return labels[status];
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ['KB', 'MB', 'GB', 'TB'];
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
