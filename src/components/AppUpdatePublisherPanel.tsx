import { useState } from "react";
import { openPath } from "@tauri-apps/plugin-opener";
import {
  CheckCircle2,
  CloudUpload,
  FileCheck2,
  FolderOpen,
  LoaderCircle,
  PackageCheck,
  ShieldAlert,
} from "lucide-react";
import { preparePlayerAppRelease, publishPlayerAppRelease } from "../api";
import type { AppReleasePreview, AppReleasePublication } from "../types";

interface AppUpdatePublisherPanelProps {
  onNotice: (message: string) => void;
}

export function AppUpdatePublisherPanel({ onNotice }: AppUpdatePublisherPanelProps) {
  const [buildManifestPath, setBuildManifestPath] = useState("");
  const [releaseNotes, setReleaseNotes] = useState("");
  const [preview, setPreview] = useState<AppReleasePreview | null>(null);
  const [publication, setPublication] = useState<AppReleasePublication | null>(null);
  const [confirmed, setConfirmed] = useState(false);
  const [busy, setBusy] = useState(false);

  function invalidate() {
    setPreview(null);
    setPublication(null);
    setConfirmed(false);
  }

  async function prepare() {
    setBusy(true);
    setConfirmed(false);
    setPublication(null);
    try {
      const result = await preparePlayerAppRelease({ buildManifestPath, releaseNotes });
      setPreview(result);
      onNotice(result.message);
    } catch (error) {
      onNotice(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function publish() {
    if (!preview || !confirmed) return;
    setBusy(true);
    try {
      const result = await publishPlayerAppRelease(preview.previewId, true);
      setPublication(result);
      setConfirmed(false);
      onNotice(result.message);
    } catch (error) {
      onNotice(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="settings-section panel-card app-release-publisher">
      <div className="section-title"><CloudUpload /><div><h2>Publish the public Player app</h2><p>Developer-only workflow. It reads the real Windows build manifest, independently re-hashes both Player artifacts, then prepares the fixed latest feed for HixxyDubz/Mythic-Loot-Launcher-Rust-.</p></div></div>
      <div className="form-grid">
        <label className="field field-wide">
          <span>Windows build manifest</span>
          <input value={buildManifestPath} onChange={(event) => { setBuildManifestPath(event.target.value); invalidate(); }} placeholder="Leave blank to use artifacts\windows\build-manifest.json from the current project" />
        </label>
        <label className="field field-wide">
          <span>Release notes</span>
          <textarea value={releaseNotes} onChange={(event) => { setReleaseNotes(event.target.value); invalidate(); }} placeholder="What changed for Player users?" maxLength={4000} />
        </label>
      </div>
      <button className="primary-action app-release-prepare" onClick={() => void prepare()} disabled={busy}>
        {busy ? <LoaderCircle className="spin" size={16} /> : <PackageCheck size={16} />} Verify packaged Player release
      </button>

      {preview && (
        <article className={`app-release-preview ${preview.ready ? "ready" : "blocked"}`}>
          <header><FileCheck2 /><div><strong>{preview.ready ? `${preview.tag} is ready for review` : "Release preparation is blocked"}</strong><small>{preview.repository} · GitHub is unchanged</small></div></header>
          {preview.assets.length > 0 && <div className="release-assets">{preview.assets.map((asset) => <span key={asset.fileName}><b>{asset.fileName}</b><small>{formatBytes(asset.bytes)} · {asset.sha256}</small></span>)}</div>}
          {preview.issues.length > 0 && <ul className="safety-issues">{preview.issues.map((issue) => <li key={issue}>{issue}</li>)}</ul>}
          {preview.outputDirectory && <button className="secondary-action" onClick={() => void openPath(preview.outputDirectory).catch((error) => onNotice(errorMessage(error)))}><FolderOpen size={14} /> Open reviewed assets</button>}
          {preview.ready && !publication && (
            <>
              <label className="confirmation-row"><input type="checkbox" checked={confirmed} onChange={(event) => setConfirmed(event.target.checked)} /><span>I reviewed these exact assets. Publish immutable {preview.tag} and replace the public latest Player feed.</span></label>
              <button className="danger-action" onClick={() => void publish()} disabled={!confirmed || busy}>{busy ? <LoaderCircle className="spin" size={16} /> : <CloudUpload size={16} />} Publish Player app update</button>
            </>
          )}
        </article>
      )}

      {publication && <article className="app-release-publication"><CheckCircle2 /><div><strong>Player app release published</strong><p>{publication.tag} · {publication.assets} immutable assets</p><small>{publication.url}</small></div></article>}
      <div className="publisher-safety safety-note"><ShieldAlert size={15} /> No Developer executable, token, arbitrary repository or unverified build artifact can enter this release.</div>
    </section>
  );
}

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KiB", "MiB", "GiB"];
  const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  return `${(bytes / (1024 ** index)).toFixed(index === 0 ? 0 : 1)} ${units[index]}`;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
