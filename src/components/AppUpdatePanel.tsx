import { useEffect, useState } from "react";
import {
  ArrowLeft,
  CheckCircle2,
  Download,
  FileCheck2,
  LoaderCircle,
  RefreshCw,
  RotateCcw,
  ShieldCheck,
  TriangleAlert,
} from "lucide-react";
import { EditionAppUpdatePublisherPanel, launcherEdition, publisherAvailable } from "@launcher-edition";
import { applyAppUpdate, checkAppUpdate, getAppUpdateResult, prepareAppUpdate } from "../api";
import type { AppUpdatePreview, AppUpdateResult, AppUpdateStage } from "../types";

interface AppUpdatePanelProps {
  onBack: () => void;
  onNotice: (message: string) => void;
}

export function AppUpdatePanel({ onBack, onNotice }: AppUpdatePanelProps) {
  const [preview, setPreview] = useState<AppUpdatePreview | null>(null);
  const [stage, setStage] = useState<AppUpdateStage | null>(null);
  const [lastResult, setLastResult] = useState<AppUpdateResult | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [confirmed, setConfirmed] = useState(false);
  const [checkError, setCheckError] = useState("");

  async function check() {
    setLoading(true);
    setCheckError("");
    setPreview(null);
    setStage(null);
    setConfirmed(false);
    try {
      setPreview(await checkAppUpdate());
    } catch (error) {
      setCheckError(errorMessage(error));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void check();
    void getAppUpdateResult().then(setLastResult).catch(() => undefined);
    // The native feed is intentionally checked when this workspace opens.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function download() {
    if (!preview?.canInstall) return;
    setBusy(true);
    try {
      const result = await prepareAppUpdate(preview.previewId);
      setStage(result);
      onNotice(result.message);
    } catch (error) {
      onNotice(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function apply() {
    if (!stage || !confirmed) return;
    setBusy(true);
    try {
      const result = await applyAppUpdate(stage.stageId, true);
      onNotice(result.message);
    } catch (error) {
      onNotice(errorMessage(error));
      setBusy(false);
    }
  }

  return (
    <main className="settings-page app-update-page">
      <div className="settings-header">
        <button className="back-button" onClick={onBack}><ArrowLeft size={18} /> Back</button>
        <div><span className="eyebrow">CHECKSUM-PROTECTED RELEASES</span><h1>App update</h1></div>
        <button className="detect-button" onClick={() => void check()} disabled={loading || busy}><RefreshCw className={loading ? "spin" : ""} size={16} /> Check again</button>
      </div>

      {lastResult && <section className={`panel-card app-update-result ${lastResult.success ? "success" : "failed"}`}>{lastResult.success ? <CheckCircle2 /> : <TriangleAlert />}<div><h2>{lastResult.success ? "Last app update succeeded" : "Last app update rolled back"}</h2><p>{lastResult.message}</p></div></section>}

      {loading ? (
        <section className="panel-card app-update-loading"><LoaderCircle className="spin" /><h2>Checking the public Player feed…</h2></section>
      ) : preview ? (
        <>
          <section className={`panel-card app-update-status ${preview.updateAvailable ? "available" : "current"}`}>
            {preview.updateAvailable ? <Download /> : <CheckCircle2 />}
            <div><span className="eyebrow">{launcherEdition.toUpperCase()} EDITION</span><h2>{preview.message}</h2><p>Installed {preview.currentVersion} · Public Player {preview.latestVersion}</p></div>
          </section>
          <section className="panel-card app-update-details">
            <div className="section-title"><FileCheck2 /><div><h2>Reviewed release metadata</h2><p>The feed is fixed to the launcher's GitHub repository. React cannot provide another URL or executable.</p></div></div>
            <dl className="preview-facts">
              <div><dt>Published</dt><dd>{formatDate(preview.publishedAt)}</dd></div>
              <div><dt>Download size</dt><dd>{formatBytes(preview.assetBytes)}</dd></div>
              <div><dt>SHA-256</dt><dd>{preview.assetSha256}</dd></div>
              <div><dt>Minimum direct-update version</dt><dd>{preview.minimumSupportedVersion || "None"}</dd></div>
            </dl>
            <article className="app-update-notes"><strong>Release notes</strong><p>{preview.releaseNotes || "No release notes were supplied."}</p></article>
            {preview.canInstall && !stage && <button className="primary-action" onClick={() => void download()} disabled={busy}>{busy ? <LoaderCircle className="spin" size={16} /> : <Download size={16} />} Download and verify Player update</button>}
            {!preview.supported && <div className="app-update-warning"><TriangleAlert /> This version is outside the direct-update range. Use the reviewed Player installer from GitHub instead.</div>}
          </section>

          {stage && (
            <section className="panel-card app-update-stage">
              <ShieldCheck />
              <div><h2>Verified update ready</h2><p>{stage.message}</p><small>{stage.path}</small></div>
              <div className="transaction-pipeline"><span>Downloaded</span><span>Size checked</span><span>SHA-256 checked</span><span>Backup</span><span>Replace &amp; recheck</span><span>Restart</span></div>
              <label className="confirmation-row"><input type="checkbox" checked={confirmed} onChange={(event) => setConfirmed(event.target.checked)} /><span>Close Player, preserve the current executable, install this verified update and restart. Roll back automatically if activation or verification fails.</span></label>
              <button className="danger-action" onClick={() => void apply()} disabled={!confirmed || busy}>{busy ? <LoaderCircle className="spin" size={16} /> : <RotateCcw size={16} />} Update and restart Player</button>
            </section>
          )}
        </>
      ) : (
        <section className="panel-card app-update-unavailable"><TriangleAlert /><div><h2>No public Player update feed is available yet</h2><p>{checkError || "The launcher repository has not published its first app release."}</p></div></section>
      )}

      {publisherAvailable && <EditionAppUpdatePublisherPanel onNotice={onNotice} />}
    </main>
  );
}

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KiB", "MiB", "GiB"];
  const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  return `${(bytes / (1024 ** index)).toFixed(index === 0 ? 0 : 1)} ${units[index]}`;
}

function formatDate(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.valueOf()) ? value : new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(date);
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
