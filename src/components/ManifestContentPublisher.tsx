import { useState } from "react";
import { CheckCircle2, CloudUpload, Eye, FileCheck2, Newspaper, RefreshCw, ShieldAlert } from "lucide-react";
import { prepareManifestContentRelease, publishManifestContentRelease } from "../api";
import type { ContentReleasePreview, ContentReleasePublication } from "../types";

interface ManifestContentPublisherProps {
  profileId: string;
  githubAuthenticated: boolean;
  onNotice: (message: string) => void;
}

export function ManifestContentPublisher({
  profileId,
  githubAuthenticated,
  onNotice,
}: ManifestContentPublisherProps) {
  const [preview, setPreview] = useState<ContentReleasePreview | null>(null);
  const [confirmed, setConfirmed] = useState(false);
  const [publication, setPublication] = useState<ContentReleasePublication | null>(null);
  const [busy, setBusy] = useState(false);

  async function prepare() {
    setBusy(true);
    setConfirmed(false);
    setPublication(null);
    try {
      const result = await prepareManifestContentRelease(profileId);
      setPreview(result);
      onNotice(result.ready
        ? "Content-only release preview ready. No package files were rebuilt or uploaded."
        : `Content-only release preparation stopped with ${result.issues.length} safety issue${result.issues.length === 1 ? "" : "s"}.`);
    } catch (error) {
      setPreview(null);
      onNotice(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function publish() {
    if (!preview) return;
    setBusy(true);
    try {
      const result = await publishManifestContentRelease(preview.previewId, confirmed);
      setPublication(result);
      setConfirmed(false);
      setPreview(null);
      onNotice(result.message);
    } catch (error) {
      onNotice(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <section className="settings-section panel-card release-builder">
        <div className="section-title">
          <Newspaper />
          <div>
            <h2>Content-only manifest release</h2>
            <p>Publish saved News, Rules and Changelog changes without rebuilding or uploading the modpack package.</p>
          </div>
        </div>
        <div className="safety-note publisher-safety">
          <FileCheck2 size={15} /> The destination is derived from this profile's fixed GitHub latest-manifest URL. Rust preserves and validates the current package URLs, hashes, file inventory and modpack version, then stages one reviewed manifest asset.
        </div>
        <button className="primary-action publisher-preview" onClick={() => void prepare()} disabled={busy}>
          {busy ? <RefreshCw className="spin" size={17} /> : <Eye size={17} />} Prepare content-only release
        </button>
      </section>

      {preview && (
        <section className={`settings-section panel-card package-preview ${preview.ready ? "ready" : "blocked"}`}>
          <div className="section-title">
            {preview.ready ? <CheckCircle2 /> : <ShieldAlert />}
            <div>
              <h2>{preview.ready ? "Content release preview ready" : "Content release blocked"}</h2>
              <p>{preview.ready ? "Nothing has been uploaded. Review the immutable package references and exact manifest asset." : "Publish or refresh the current modpack package before creating a content-only release."}</p>
            </div>
          </div>
          <dl className="pack-facts preview-facts">
            {preview.repository && <div><dt>Destination</dt><dd>{preview.repository} · {preview.tag || "Not ready"}</dd></div>}
            {preview.ready && <div><dt>Modpack unchanged</dt><dd>Version {preview.modpackVersion} · {preview.requiredFileCount.toLocaleString()} tracked files</dd></div>}
            {preview.ready && <div><dt>Package references</dt><dd>{preview.packageAssetsPreserved} immutable asset{preview.packageAssetsPreserved === 1 ? "" : "s"} preserved</dd></div>}
            {preview.ready && <div><dt>Public content</dt><dd>{preview.rulesCount} rules · {preview.changelogCount} changelog entries</dd></div>}
            {preview.ready && <div><dt>Only upload</dt><dd>{formatBytes(preview.bytes)} manifest · SHA-256 {preview.sha256.slice(0, 12)}…</dd></div>}
            {preview.ready && <div><dt>Player feed</dt><dd title={preview.manifestUrl}>{preview.manifestUrl}</dd></div>}
          </dl>
          {preview.ready && (
            <div className="release-assets" aria-label="Reviewed content release assets">
              <strong>Reviewed GitHub assets</strong>
              <span title={preview.manifestPath}><b>1. Trusted manifest only</b><small>{formatBytes(preview.bytes)} · no package ZIP</small></span>
            </div>
          )}
          {preview.issues.length > 0 && <ul className="safety-issues">{preview.issues.map((issue) => <li key={issue}>{issue}</li>)}</ul>}
          {preview.ready && (
            <>
              <label className="confirmation-row">
                <input type="checkbox" checked={confirmed} onChange={(event) => setConfirmed(event.target.checked)} />
                <span>I confirm publishing {preview.tag} as the latest release with only this reviewed manifest. Existing modpack package assets will not be uploaded again.</span>
              </label>
              {!githubAuthenticated && <p className="publish-gate">Run Check GitHub and authenticate before publication. The reviewed local preview remains available.</p>}
              <button className="primary-action danger-action" onClick={() => void publish()} disabled={!confirmed || !githubAuthenticated || busy}>
                {busy ? <RefreshCw className="spin" size={17} /> : <CloudUpload size={17} />} Publish content-only release
              </button>
            </>
          )}
        </section>
      )}

      {publication && (
        <section className="settings-section panel-card creation-result">
          <CheckCircle2 />
          <div><h2>{publication.tag} published</h2><p>{publication.url || publication.message}</p></div>
        </section>
      )}
    </>
  );
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
