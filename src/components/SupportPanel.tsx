import { useEffect, useState } from "react";
import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
import {
  ArrowLeft,
  CheckCircle2,
  Clipboard,
  FileArchive,
  FileText,
  FolderOpen,
  LifeBuoy,
  LoaderCircle,
  RefreshCw,
  ShieldCheck,
  TriangleAlert,
} from "lucide-react";
import { createSupportBundle, prepareSupportBundle } from "../api";
import type { GameProfile, SupportBundleOutcome, SupportPreview } from "../types";

interface SupportPanelProps {
  profile: GameProfile;
  onBack: () => void;
  onNotice: (message: string) => void;
}

export function SupportPanel({ profile, onBack, onNotice }: SupportPanelProps) {
  const [preview, setPreview] = useState<SupportPreview | null>(null);
  const [outcome, setOutcome] = useState<SupportBundleOutcome | null>(null);
  const [loading, setLoading] = useState(true);
  const [exporting, setExporting] = useState(false);
  const [confirmed, setConfirmed] = useState(false);

  async function refresh() {
    setLoading(true);
    setConfirmed(false);
    setOutcome(null);
    try {
      setPreview(await prepareSupportBundle(profile.id));
    } catch (error) {
      setPreview(null);
      onNotice(errorMessage(error));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void refresh();
    // Refresh when the workspace is opened for this keyed profile.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function copySummary() {
    if (!preview) return;
    try {
      await navigator.clipboard.writeText(preview.summary);
      onNotice("The reviewed redacted support summary was copied.");
    } catch (error) {
      onNotice(errorMessage(error));
    }
  }

  async function openLatestLog() {
    if (!preview?.latestLogPath) return;
    try {
      await openPath(preview.latestLogPath);
    } catch (error) {
      onNotice(errorMessage(error));
    }
  }

  async function exportBundle() {
    if (!preview || !confirmed || outcome) return;
    setExporting(true);
    try {
      const result = await createSupportBundle(preview.previewId, true);
      setOutcome(result);
      setConfirmed(false);
      onNotice(result.message);
    } catch (error) {
      onNotice(errorMessage(error));
    } finally {
      setExporting(false);
    }
  }

  return (
    <main className="settings-page support-page">
      <div className="settings-header">
        <button className="back-button" onClick={onBack}><ArrowLeft size={18} /> Back</button>
        <div><span className="eyebrow">PRIVACY-REDACTED DIAGNOSTICS</span><h1>Support</h1></div>
        <button className="detect-button" onClick={() => void refresh()} disabled={loading || exporting}>
          <RefreshCw className={loading ? "spin" : ""} size={16} /> Review again
        </button>
      </div>

      {loading && !preview ? (
        <section className="panel-card support-loading"><LoaderCircle className="spin" /><h2>Preparing a private review…</h2><p>No support bundle is written during this step.</p></section>
      ) : preview ? (
        <>
          <section className="panel-card support-hero">
            <LifeBuoy />
            <div><span className="eyebrow">{preview.displayName}</span><h2>Review exactly what will be shared</h2><p>{preview.message}</p></div>
          </section>

          <section className="support-overview">
            <article className="panel-card support-source">
              <FileText />
              <span><small>Latest known game log</small><strong>{preview.latestLogName || "No supported log found"}</strong><em>{preview.latestLogPath || "The bundle will contain the launcher summary only."}</em></span>
              {preview.latestLogPath && <button className="secondary-action" onClick={() => void openLatestLog()}><FolderOpen size={15} /> Open source log</button>}
            </article>
            <article className="panel-card support-privacy">
              <ShieldCheck />
              <span><small>Privacy boundary</small><strong>Server configuration is never included</strong><em>Account paths, obvious secrets, email addresses and network addresses are redacted. Options and full configuration files are excluded.</em></span>
            </article>
          </section>

          <section className="panel-card support-review">
            <div className="section-title"><FileArchive /><div><h2>Reviewed bundle contents</h2><p>Only these fixed files can be written. The log is capped at 500 lines and 512 KiB from a source no larger than 64 MiB.</p></div></div>
            <div className="support-file-list">
              {preview.files.map((file) => <span key={file}><CheckCircle2 size={14} /> {file}</span>)}
            </div>
            <dl className="preview-facts support-facts">
              <div><dt>Source log</dt><dd>{formatBytes(preview.sourceBytes)}</dd></div>
              <div><dt>Redacted excerpt</dt><dd>{formatBytes(preview.includedBytes)}</dd></div>
              <div><dt>Excerpt limited</dt><dd>{preview.truncated ? "Yes" : "No"}</dd></div>
            </dl>
          </section>

          <section className="support-preview-grid">
            <article className="panel-card support-preview-card">
              <header><div><strong>summary.txt</strong><small>Exact redacted text</small></div><button className="secondary-action" onClick={() => void copySummary()}><Clipboard size={14} /> Copy summary</button></header>
              <pre>{preview.summary}</pre>
            </article>
            <article className="panel-card support-preview-card">
              <header><div><strong>{preview.latestLogName ? `${preview.latestLogName}.redacted.txt` : "No log excerpt"}</strong><small>{preview.redactedLog ? "Exact bounded redacted text" : "No supported game log was found"}</small></div></header>
              <pre>{preview.redactedLog || "This export will not contain a log file."}</pre>
            </article>
          </section>

          {preview.issues.length > 0 && (
            <section className="panel-card support-issues"><TriangleAlert /><div><h2>Some diagnostic data was unavailable</h2>{preview.issues.map((issue) => <p key={issue}>{issue}</p>)}</div></section>
          )}

          {!outcome ? (
            <section className="panel-card support-export">
              <ShieldCheck />
              <div><h2>Create this reviewed bundle</h2><p>The native exporter accepts only this in-memory preview ID and writes to the launcher-owned support folder. It never accepts an arbitrary source or destination path.</p></div>
              <label className="confirmation-row"><input type="checkbox" checked={confirmed} onChange={(event) => setConfirmed(event.target.checked)} /><span>I reviewed the exact redacted summary and log excerpt above and approve this export.</span></label>
              <button className="primary-action" onClick={() => void exportBundle()} disabled={!preview.ready || !confirmed || exporting}>
                {exporting ? <LoaderCircle className="spin" size={16} /> : <FileArchive size={16} />} Create support bundle
              </button>
            </section>
          ) : (
            <section className="panel-card support-outcome">
              <CheckCircle2 />
              <div><h2>Support bundle created</h2><p>{outcome.fileName} · {formatBytes(outcome.bytes)}</p><small>SHA-256: {outcome.sha256}</small><em>{outcome.path}</em></div>
              <div className="support-outcome-actions">
                <button className="secondary-action" onClick={() => void openPath(outcome.directory).catch((error) => onNotice(errorMessage(error)))}><FolderOpen size={15} /> Open folder</button>
                <button className="primary-action" onClick={() => void revealItemInDir(outcome.path).catch((error) => onNotice(errorMessage(error)))}><FileArchive size={15} /> Show bundle</button>
              </div>
            </section>
          )}
        </>
      ) : (
        <section className="panel-card support-loading"><TriangleAlert /><h2>Support review unavailable</h2><p>Use Review again to retry the native log discovery and redaction.</p></section>
      )}
    </main>
  );
}

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KiB", "MiB", "GiB"];
  const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const value = bytes / (1024 ** index);
  return `${index === 0 ? Math.round(value) : value.toFixed(1)} ${units[index]}`;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
