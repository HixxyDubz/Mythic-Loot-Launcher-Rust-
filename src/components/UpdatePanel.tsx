import { useEffect, useState } from "react";
import {
  ArchiveRestore,
  ArrowLeft,
  CheckCircle2,
  Download,
  HardDrive,
  RefreshCw,
  RotateCcw,
  ShieldAlert,
  ShieldCheck,
  Trash2,
  Wrench,
} from "lucide-react";
import {
  applyModpackTransaction,
  applyRestorePoint,
  deleteRestorePoint,
  listRestorePoints,
  prepareModpackTransaction,
  prepareRestorePoint,
} from "../api";
import type {
  GameProfile,
  ManifestSummary,
  ProfileHealth,
  RestoreOutcome,
  RestorePointSummary,
  RestorePreview,
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
  const [restorePoints, setRestorePoints] = useState<RestorePointSummary[]>([]);
  const [restorePreview, setRestorePreview] = useState<RestorePreview | null>(null);
  const [restoreOutcome, setRestoreOutcome] = useState<RestoreOutcome | null>(null);
  const [restoreConfirmed, setRestoreConfirmed] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<string | null>(null);
  const [deleteConfirmed, setDeleteConfirmed] = useState(false);
  const [restoreBusy, setRestoreBusy] = useState(false);

  useEffect(() => {
    setRestorePreview(null);
    setRestoreOutcome(null);
    setDeleteTarget(null);
    void loadRestorePoints();
  }, [profile.id]);

  async function loadRestorePoints() {
    try {
      setRestorePoints(await listRestorePoints(profile.id));
    } catch (error) {
      onNotice(errorMessage(error));
    }
  }

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
      if (result.success) {
        await onCompleted();
        await loadRestorePoints();
      }
    } catch (error) {
      onNotice(errorMessage(error));
    } finally {
      setBusy(false);
      setBusyKind(null);
    }
  }

  async function reviewRestore(point: RestorePointSummary) {
    setRestoreBusy(true);
    setRestorePreview(null);
    setRestoreOutcome(null);
    setRestoreConfirmed(false);
    setDeleteTarget(null);
    try {
      const result = await prepareRestorePoint(profile.id, point.backupId);
      setRestorePreview(result);
      onNotice(result.message);
    } catch (error) {
      onNotice(errorMessage(error));
    } finally {
      setRestoreBusy(false);
    }
  }

  async function restore() {
    if (!restorePreview) return;
    setRestoreBusy(true);
    try {
      const result = await applyRestorePoint(restorePreview.previewId, restoreConfirmed);
      setRestoreOutcome(result);
      setRestoreConfirmed(false);
      onNotice(result.message);
      if (result.success) {
        setRestorePreview(null);
        await onCompleted();
        await loadRestorePoints();
      }
    } catch (error) {
      onNotice(errorMessage(error));
    } finally {
      setRestoreBusy(false);
    }
  }

  async function removeRestorePoint() {
    if (!deleteTarget) return;
    setRestoreBusy(true);
    try {
      const message = await deleteRestorePoint(profile.id, deleteTarget, deleteConfirmed);
      onNotice(message);
      setDeleteTarget(null);
      setDeleteConfirmed(false);
      setRestorePreview((current) => current?.backupId === deleteTarget ? null : current);
      await loadRestorePoints();
    } catch (error) {
      onNotice(errorMessage(error));
    } finally {
      setRestoreBusy(false);
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

        <section className="settings-section panel-card restore-history">
          <div className="section-title">
            <ArchiveRestore />
            <div><h2>Recovery history</h2><p>Transactional backups are kept in launcher-owned storage. The newest five points are retained automatically.</p></div>
            <button className="detect-button" onClick={() => void loadRestorePoints()} disabled={restoreBusy}><RefreshCw size={15} /> Refresh</button>
          </div>
          {restorePoints.length === 0 ? (
            <div className="restore-empty"><HardDrive /><span><strong>No restore points yet</strong><small>A verified point is created immediately before the first live write of an update, repair, or restore.</small></span></div>
          ) : (
            <div className="restore-list">
              {restorePoints.map((point) => (
                <article className={`restore-row ${point.valid ? '' : 'invalid'}`} key={point.backupId}>
                  <ArchiveRestore />
                  <div className="restore-copy">
                    <strong>{backupLabel(point.label)}</strong>
                    <small>{formatDate(point.createdAt)} · {point.fileCount.toLocaleString()} files · {formatBytes(point.sizeBytes)}{point.localModpackVersion ? ` · v${point.localModpackVersion}` : ''}</small>
                    {point.removesOnRestore > 0 && <small>{point.removesOnRestore.toLocaleString()} update-created paths will be removed</small>}
                    {!point.valid && <small className="outcome-error">{point.issues[0] || 'This backup cannot be restored safely.'}</small>}
                  </div>
                  <div className="restore-actions">
                    <button className="secondary-action" onClick={() => void reviewRestore(point)} disabled={!point.valid || restoreBusy}>Review restore</button>
                    <button className="icon-danger" aria-label={`Delete ${backupLabel(point.label)}`} onClick={() => { setDeleteTarget(point.backupId); setDeleteConfirmed(false); }} disabled={restoreBusy}><Trash2 size={15} /></button>
                  </div>
                </article>
              ))}
            </div>
          )}
        </section>

        {restorePreview && (
          <section className="settings-section panel-card transaction-preview ready restore-preview">
            <div className="section-title"><ShieldCheck /><div><h2>Restore candidate verified</h2><p>{restorePreview.message}</p></div></div>
            <dl className="pack-facts preview-facts">
              <div><dt>Restore point</dt><dd>{backupLabel(restorePreview.label)}</dd></div>
              <div><dt>Created</dt><dd>{formatDate(restorePreview.createdAt)}</dd></div>
              <div><dt>Restored version</dt><dd>{restorePreview.localModpackVersion || 'Unrecorded'}</dd></div>
              <div><dt>Staged payload</dt><dd>{restorePreview.stagedFiles.toLocaleString()} files · {formatBytes(restorePreview.stagedBytes)}</dd></div>
              <div><dt>Current files protected</dt><dd>{restorePreview.existingFilesToBackup.toLocaleString()}</dd></div>
              <div><dt>Update-created paths removed</dt><dd>{restorePreview.filesToRemove.toLocaleString()}</dd></div>
            </dl>
            <div className="safety-note transaction-backup-note"><HardDrive size={15} /> A second recovery backup of the current installation is created before this restore changes any live path.</div>
            <label className="confirmation-row"><input type="checkbox" checked={restoreConfirmed} onChange={(event) => setRestoreConfirmed(event.target.checked)} /><span>I confirm that the launcher may create a recovery backup and transactionally restore this reviewed point.</span></label>
            <button className="primary-action danger-action" onClick={() => void restore()} disabled={!restoreConfirmed || restoreBusy}>
              {restoreBusy ? <RefreshCw className="spin" size={17} /> : <ArchiveRestore size={17} />} Restore verified point
            </button>
          </section>
        )}

        {deleteTarget && (
          <section className="settings-section panel-card delete-restore-confirmation">
            <ShieldAlert />
            <div><h2>Delete this restore point?</h2><p>This removes only the selected launcher-owned ZIP. It cannot be undone and does not change the live modpack.</p></div>
            <label className="confirmation-row"><input type="checkbox" checked={deleteConfirmed} onChange={(event) => setDeleteConfirmed(event.target.checked)} /><span>I understand this recovery file will be permanently deleted.</span></label>
            <div className="delete-actions"><button className="secondary-action" onClick={() => { setDeleteTarget(null); setDeleteConfirmed(false); }}>Cancel</button><button className="primary-action danger-action" onClick={() => void removeRestorePoint()} disabled={!deleteConfirmed || restoreBusy}><Trash2 size={16} /> Delete restore point</button></div>
          </section>
        )}

        {restoreOutcome && (
          <section className={`settings-section panel-card transaction-outcome ${restoreOutcome.success ? 'success' : restoreOutcome.rolledBack ? 'rolled-back' : 'failed'}`}>
            {restoreOutcome.success ? <CheckCircle2 /> : restoreOutcome.rolledBack ? <RotateCcw /> : <ShieldAlert />}
            <div>
              <h2>{restoreOutcome.success ? 'Restore complete' : restoreOutcome.rolledBack ? 'Restore rolled back safely' : 'Restore needs attention'}</h2>
              <p>{restoreOutcome.message}</p>
              <small>{restoreOutcome.restored.length} restored · {restoreOutcome.removed.length} removed{restoreOutcome.recoveryBackupPath ? ` · Recovery backup: ${restoreOutcome.recoveryBackupPath}` : ''}</small>
              {restoreOutcome.error && <small className="outcome-error">Cause: {restoreOutcome.error}</small>}
              {restoreOutcome.rollbackError && <small className="outcome-error">Rollback: {restoreOutcome.rollbackError}</small>}
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

function formatDate(timestamp: number): string {
  if (!timestamp) return 'Unknown date';
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(new Date(timestamp * 1000));
}

function backupLabel(label: string): string {
  return label.replace(/_/g, ' ').replace(/\b\w/g, (value: string) => value.toUpperCase());
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
