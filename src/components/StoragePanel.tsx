import { useEffect, useMemo, useState } from "react";
import { openPath } from "@tauri-apps/plugin-opener";
import {
  ArchiveRestore,
  ArrowLeft,
  Database,
  FolderOpen,
  HardDrive,
  LoaderCircle,
  RefreshCw,
  ShieldCheck,
  Trash2,
  TriangleAlert,
} from "lucide-react";
import { cleanStorage, getStorageReport } from "../api";
import type { StorageBucket, StorageCleanupKind, StorageReport } from "../types";

interface StoragePanelProps {
  onBack: () => void;
  onNotice: (message: string) => void;
}

const cleanupCopy: Record<StorageCleanupKind, { title: string; detail: string }> = {
  oldBackups: {
    title: "Clean old restore points",
    detail: "Keep the newest five ZIP restore points for every known modpack profile.",
  },
  metadataCache: {
    title: "Clear catalogue cache",
    detail: "Remove only the downloaded public catalogue cache. Trusted manifests and local settings stay intact.",
  },
  temporaryWork: {
    title: "Clean old temporary work",
    detail: "Remove launcher staging and Developer preview entries older than 24 hours. Recent work stays intact.",
  },
};

export function StoragePanel({ onBack, onNotice }: StoragePanelProps) {
  const [report, setReport] = useState<StorageReport | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [cleanupKind, setCleanupKind] = useState<StorageCleanupKind | null>(null);
  const [confirmed, setConfirmed] = useState(false);

  async function refresh() {
    setLoading(true);
    try {
      setReport(await getStorageReport());
    } catch (error) {
      onNotice(errorMessage(error));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void refresh();
    // The native report is intentionally taken once when this workspace opens.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const cleanupBytes = useMemo(() => {
    if (!report) return new Map<StorageCleanupKind, number>();
    const totals = new Map<StorageCleanupKind, number>();
    for (const bucket of report.buckets) {
      if (!bucket.cleanupKind) continue;
      totals.set(bucket.cleanupKind, (totals.get(bucket.cleanupKind) ?? 0) + bucket.bytesUsed);
    }
    return totals;
  }, [report]);

  async function openDataFolder() {
    if (!report) return;
    try {
      await openPath(report.dataDir);
    } catch (error) {
      onNotice(errorMessage(error));
    }
  }

  async function applyCleanup() {
    if (!cleanupKind || !confirmed) return;
    setBusy(true);
    try {
      const outcome = await cleanStorage(cleanupKind, true);
      setReport(outcome.report);
      setCleanupKind(null);
      setConfirmed(false);
      onNotice(outcome.message);
    } catch (error) {
      onNotice(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  function chooseCleanup(kind: StorageCleanupKind) {
    setCleanupKind(kind);
    setConfirmed(false);
  }

  return (
    <main className="settings-page storage-page">
      <div className="settings-header">
        <button className="back-button" onClick={onBack}><ArrowLeft size={18} /> Back</button>
        <div><span className="eyebrow">LOCAL DISK USAGE</span><h1>Storage</h1></div>
        <button className="detect-button" onClick={() => void refresh()} disabled={loading || busy}>
          <RefreshCw className={loading ? "spin" : ""} size={16} /> Refresh
        </button>
      </div>

      {loading && !report ? (
        <section className="panel-card storage-loading"><LoaderCircle className="spin" /><h2>Measuring storage…</h2><p>Linked folders are skipped and never traversed.</p></section>
      ) : report ? (
        <>
          <section className="storage-totals">
            <SummaryCard icon={Database} label="Launcher data" bytes={report.launcherBytes} />
            <SummaryCard icon={HardDrive} label="Configured modpacks" bytes={report.profileBytes} />
            <button className="panel-card storage-folder" onClick={() => void openDataFolder()}>
              <FolderOpen /><span><small>Launcher data folder</small><strong>Open folder</strong><em>{report.dataDir}</em></span>
            </button>
          </section>

          <section className="settings-section panel-card storage-cleanup">
            <div className="section-title"><ShieldCheck /><div><h2>Safe cleanup</h2><p>Only the fixed launcher-owned categories below can be changed. Modpack folders, settings, manifests, Minecraft imports and Safe Launch recovery are never cleanup targets.</p></div></div>
            <div className="storage-cleanup-grid">
              {(Object.keys(cleanupCopy) as StorageCleanupKind[]).map((kind) => (
                <button key={kind} onClick={() => chooseCleanup(kind)} disabled={busy}>
                  {kind === "oldBackups" ? <ArchiveRestore /> : <Trash2 />}
                  <span><strong>{cleanupCopy[kind].title}</strong><small>{cleanupCopy[kind].detail}</small><em>{formatBytes(cleanupBytes.get(kind) ?? 0)} currently in this category</em></span>
                </button>
              ))}
            </div>
          </section>

          {cleanupKind && (
            <section className="panel-card storage-confirmation">
              <TriangleAlert />
              <div><h2>Confirm {cleanupCopy[cleanupKind].title.toLowerCase()}</h2><p>{cleanupCopy[cleanupKind].detail} The native command receives this category only—not a filesystem path.</p></div>
              <label className="confirmation-row"><input type="checkbox" checked={confirmed} onChange={(event) => setConfirmed(event.target.checked)} /><span>I confirm that the launcher may clean this reviewed launcher-owned category.</span></label>
              <div className="storage-confirm-actions">
                <button className="secondary-action" onClick={() => { setCleanupKind(null); setConfirmed(false); }} disabled={busy}>Cancel</button>
                <button className="danger-action" onClick={() => void applyCleanup()} disabled={!confirmed || busy}>
                  {busy ? <LoaderCircle className="spin" size={16} /> : <Trash2 size={16} />} Apply cleanup
                </button>
              </div>
            </section>
          )}

          <section className="settings-section panel-card storage-inventory">
            <div className="section-title"><HardDrive /><div><h2>Storage inventory</h2><p>Measured {formatTime(report.measuredAt)}. Counts are read-only; inaccessible or redirected entries are reported instead of followed.</p></div></div>
            <div className="storage-bucket-list">
              {report.buckets.map((bucket) => <BucketRow key={bucket.key} bucket={bucket} />)}
            </div>
          </section>

          {(report.issues.length > 0 || report.truncated) && (
            <section className="panel-card storage-issues"><TriangleAlert /><div><h2>Some storage was not measured</h2>{report.issues.map((issue) => <p key={issue}>{issue}</p>)}</div></section>
          )}
        </>
      ) : (
        <section className="panel-card storage-loading"><TriangleAlert /><h2>Storage report unavailable</h2><p>Use Refresh to try the native scan again.</p></section>
      )}
    </main>
  );
}

function SummaryCard({ icon: Icon, label, bytes }: { icon: typeof Database; label: string; bytes: number }) {
  return <article className="panel-card storage-total"><Icon /><span><small>{label}</small><strong>{formatBytes(bytes)}</strong></span></article>;
}

function BucketRow({ bucket }: { bucket: StorageBucket }) {
  return (
    <article className="storage-bucket">
      <span><strong>{bucket.label}</strong><small>{bucket.category} · {bucket.exists ? `${bucket.fileCount} files · ${bucket.directoryCount} folders` : "Not created"}{bucket.truncated ? " · Partial measurement" : ""}</small><em>{bucket.path}</em></span>
      <b>{formatBytes(bucket.bytesUsed)}</b>
    </article>
  );
}

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KiB", "MiB", "GiB", "TiB"];
  const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const value = bytes / (1024 ** index);
  return `${index === 0 ? Math.round(value) : value.toFixed(1)} ${units[index]}`;
}

function formatTime(timestamp: number): string {
  if (!Number.isFinite(timestamp) || timestamp <= 0) return "at an unknown time";
  return new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(new Date(timestamp));
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
