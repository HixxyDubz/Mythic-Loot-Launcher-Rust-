import { useState } from "react";
import {
  Archive,
  ArrowLeft,
  BookOpen,
  CheckCircle2,
  CloudUpload,
  Eye,
  FileCheck2,
  LockKeyhole,
  RefreshCw,
  ShieldAlert,
} from "lucide-react";
import {
  createGithubRepository,
  githubPublisherStatus,
  prepareModpackRelease,
  preparePublicCatalog,
  publishModpackRelease,
  publishPublicCatalog,
} from "../api";
import type {
  BootstrapPayload,
  CatalogPreview,
  CatalogPublication,
  GameProfile,
  PackagePreview,
  PublisherStatus,
  ReleasePublication,
  RepositoryCreation,
  RepositoryRequest,
} from "../types";

interface PublisherPanelProps {
  profile: GameProfile;
  onBack: () => void;
  onNotice: (message: string) => void;
  onPayload: (payload: BootstrapPayload) => void;
}

export function PublisherPanel({ profile, onBack, onNotice, onPayload }: PublisherPanelProps) {
  const [status, setStatus] = useState<PublisherStatus | null>(null);
  const [repository, setRepository] = useState(guessRepository(profile.manifestUrl));
  const [description, setDescription] = useState(`${profile.displayName} release repository`);
  const [visibility, setVisibility] = useState<"private" | "public">("private");
  const [repositoryPreviewed, setRepositoryPreviewed] = useState(false);
  const [repositoryConfirmed, setRepositoryConfirmed] = useState(false);
  const [sourceDir, setSourceDir] = useState(profile.installDir);
  const [version, setVersion] = useState(profile.requiredModpackVersion);
  const [releaseDate, setReleaseDate] = useState(new Date().toISOString().slice(0, 10));
  const [releaseNotes, setReleaseNotes] = useState(`Release ${profile.requiredModpackVersion}`);
  const [releasePreview, setReleasePreview] = useState<PackagePreview | null>(null);
  const [releaseConfirmed, setReleaseConfirmed] = useState(false);
  const [busy, setBusy] = useState(false);
  const [creation, setCreation] = useState<RepositoryCreation | null>(null);
  const [publication, setPublication] = useState<ReleasePublication | null>(null);
  const [catalogPreview, setCatalogPreview] = useState<CatalogPreview | null>(null);
  const [catalogConfirmed, setCatalogConfirmed] = useState(false);
  const [catalogPublication, setCatalogPublication] = useState<CatalogPublication | null>(null);

  function invalidateRelease() {
    setReleasePreview(null);
    setReleaseConfirmed(false);
    setPublication(null);
  }

  async function checkGithub() {
    setBusy(true);
    try {
      const result = await githubPublisherStatus();
      setStatus(result);
      onNotice(result.message);
    } catch (error) {
      onNotice(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  function previewRepository() {
    setRepositoryConfirmed(false);
    setCreation(null);
    setRepositoryPreviewed(true);
  }

  async function createRepository() {
    const request: RepositoryRequest = {
      repository,
      description,
      visibility,
      confirmed: repositoryConfirmed,
    };
    setBusy(true);
    try {
      const result = await createGithubRepository(request);
      setCreation(result);
      setRepositoryConfirmed(false);
      onNotice(result.message);
    } catch (error) {
      onNotice(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function prepareRelease() {
    setBusy(true);
    setReleaseConfirmed(false);
    setPublication(null);
    try {
      const result = await prepareModpackRelease({
        profileId: profile.id,
        sourceDir,
        version,
        releaseDate,
        repository,
        releaseNotes,
      });
      setReleasePreview(result);
      onNotice(
        result.ready
          ? `Local release preview ready: ${result.fileCount.toLocaleString()} files scanned and packaged.`
          : `Release preparation stopped with ${result.issues.length} safety issue${result.issues.length === 1 ? "" : "s"}.`,
      );
    } catch (error) {
      setReleasePreview(null);
      onNotice(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function publishRelease() {
    if (!releasePreview) return;
    setBusy(true);
    try {
      const result = await publishModpackRelease(releasePreview.previewId, releaseConfirmed);
      setPublication(result.publication);
      onPayload(result.payload);
      setReleaseConfirmed(false);
      setCatalogPreview(null);
      setCatalogPublication(null);
      onNotice(`${result.publication.message} The local profile now points to its latest manifest.`);
    } catch (error) {
      onNotice(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function prepareCatalog() {
    setBusy(true);
    setCatalogConfirmed(false);
    setCatalogPublication(null);
    try {
      const result = await preparePublicCatalog();
      setCatalogPreview(result);
      onNotice(
        result.ready
          ? `Public catalogue preview ready with ${result.profiles.length} visible modpack${result.profiles.length === 1 ? "" : "s"}.`
          : `Catalogue preparation stopped with ${result.issues.length} safety issue${result.issues.length === 1 ? "" : "s"}.`,
      );
    } catch (error) {
      setCatalogPreview(null);
      onNotice(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function publishCatalog() {
    if (!catalogPreview) return;
    setBusy(true);
    try {
      const result = await publishPublicCatalog(catalogPreview.previewId, catalogConfirmed);
      setCatalogPublication(result);
      setCatalogConfirmed(false);
      onNotice(result.message);
    } catch (error) {
      onNotice(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  const canPreviewRepository = Boolean(status?.authenticated && repository.includes("/") && !busy);
  const canPrepareRelease = Boolean(
    repository.includes("/") && sourceDir.trim() && version.trim() && releaseDate.trim() && !busy,
  );

  return (
    <main className="settings-page publisher-page">
      <div className="settings-header">
        <button className="back-button" onClick={onBack}><ArrowLeft size={18} /> Back</button>
        <div><span className="eyebrow">DEVELOPER WORKSPACE</span><h1>GitHub Publisher</h1></div>
        <button className="detect-button" onClick={() => void checkGithub()} disabled={busy}>
          {busy ? <RefreshCw className="spin" size={16} /> : <CloudUpload size={16} />} Check GitHub
        </button>
      </div>

      <div className="settings-layout">
        <section className="settings-section panel-card">
          <div className="section-title"><LockKeyhole /><div><h2>Authenticated GitHub CLI</h2><p>No access token is stored in the launcher or exposed to React.</p></div></div>
          <div className={`publisher-status ${status?.authenticated ? "good" : "pending"}`}>
            {status?.authenticated ? <CheckCircle2 /> : <ShieldAlert />}
            <span><strong>{status?.authenticated ? `Ready as ${status.account || "authenticated user"}` : "Preflight required"}</strong><small>{status?.message || "Local packaging works without authentication. Check GitHub before creating a repository or publishing a release."}</small></span>
          </div>
        </section>

        <section className="settings-section panel-card">
          <div className="section-title"><CloudUpload /><div><h2>Repository</h2><p>Select an existing owner/name repository, or review and create an empty one.</p></div></div>
          <div className="form-stack">
            <label className="field"><span>Repository (owner/name)</span><input value={repository} placeholder="HixxyDubz/Mythic-Loot-Modpack" onChange={(event) => { setRepository(event.target.value); setRepositoryPreviewed(false); invalidateRelease(); }} /></label>
            <div className="form-grid">
              <label className="field"><span>Description</span><input value={description} maxLength={350} onChange={(event) => { setDescription(event.target.value); setRepositoryPreviewed(false); }} /></label>
              <label className="field"><span>New repository visibility</span><select value={visibility} onChange={(event) => { setVisibility(event.target.value as "private" | "public"); setRepositoryPreviewed(false); }}><option value="private">Private (recommended)</option><option value="public">Public</option></select></label>
            </div>
          </div>
          <button className="secondary-action publisher-preview" onClick={previewRepository} disabled={!canPreviewRepository}><Eye size={17} /> Preview empty repository creation</button>
        </section>

        {repositoryPreviewed && (
          <section className="settings-section panel-card mutation-preview">
            <div className="section-title"><Eye /><div><h2>External change preview</h2><p>Nothing has been created yet.</p></div></div>
            <dl className="pack-facts">
              <div><dt>Repository</dt><dd>{repository}</dd></div>
              <div><dt>Visibility</dt><dd>{visibility}</dd></div>
              <div><dt>Initial files</dt><dd>None</dd></div>
              <div><dt>Action</dt><dd>Create repository on GitHub</dd></div>
            </dl>
            <label className="confirmation-row"><input type="checkbox" checked={repositoryConfirmed} onChange={(event) => setRepositoryConfirmed(event.target.checked)} /><span>I confirm that the launcher may create this empty GitHub repository.</span></label>
            <button className="primary-action danger-action" onClick={() => void createRepository()} disabled={!repositoryConfirmed || busy}>
              {busy ? <RefreshCw className="spin" size={17} /> : <CloudUpload size={17} />} Create repository
            </button>
          </section>
        )}

        {creation && <section className="settings-section panel-card creation-result"><CheckCircle2 /><div><h2>{creation.repository} created</h2><p>{creation.url || creation.message}</p></div></section>}

        <section className="settings-section panel-card release-builder">
          <div className="section-title"><Archive /><div><h2>Local release preparation</h2><p>Scan privacy, inventory and hash files, then generate a deterministic ZIP and trusted manifest. This step does not contact GitHub.</p></div></div>
          <div className="form-grid">
            <label className="field field-wide"><span>Modpack source folder</span><input value={sourceDir} placeholder="C:\Modpacks\Mythic Loot" onChange={(event) => { setSourceDir(event.target.value); invalidateRelease(); }} /></label>
            <label className="field"><span>Version</span><input value={version} placeholder="1.0.0" onChange={(event) => { setVersion(event.target.value); invalidateRelease(); }} /></label>
            <label className="field"><span>Release date</span><input type="date" value={releaseDate} onChange={(event) => { setReleaseDate(event.target.value); invalidateRelease(); }} /></label>
            <label className="field field-wide"><span>Release notes</span><input value={releaseNotes} maxLength={20000} onChange={(event) => { setReleaseNotes(event.target.value); invalidateRelease(); }} /></label>
          </div>
          <div className="safety-note publisher-safety"><FileCheck2 size={15} /> Excludes saves, logs, screenshots, caches, upstream README/changelog documents and known per-user Minecraft files. Credential-shaped runtime content stops the build. Packages at or above 2 GiB are split into ordered, hash-verified 1 GiB release parts.</div>
          <button className="primary-action publisher-preview" onClick={() => void prepareRelease()} disabled={!canPrepareRelease}>
            {busy ? <RefreshCw className="spin" size={17} /> : <Archive size={17} />} Prepare release locally
          </button>
        </section>

        {releasePreview && (
          <section className={`settings-section panel-card package-preview ${releasePreview.ready ? "ready" : "blocked"}`}>
            <div className="section-title">{releasePreview.ready ? <CheckCircle2 /> : <ShieldAlert />}<div><h2>{releasePreview.ready ? "Release preview ready" : "Release blocked by safety checks"}</h2><p>{releasePreview.ready ? "The package and manifest exist locally. Nothing has been uploaded." : "No publishable package was produced. Resolve every issue and prepare again."}</p></div></div>
            <dl className="pack-facts preview-facts">
              <div><dt>Release</dt><dd>{releasePreview.repository} · {releasePreview.tag}</dd></div>
              <div><dt>Inventory</dt><dd>{releasePreview.fileCount.toLocaleString()} files · {formatBytes(releasePreview.totalBytes)}</dd></div>
              <div><dt>Excluded runtime entries</dt><dd>{releasePreview.excludedCount.toLocaleString()}</dd></div>
              <div><dt>Changes</dt><dd>{releasePreview.added} added · {releasePreview.changed} changed · {releasePreview.removed} removed</dd></div>
              {releasePreview.ready && <div><dt>Package</dt><dd>{releasePreview.multipart ? `Multipart · ${releasePreview.assets.length} parts` : "Single ZIP"} · {formatBytes(releasePreview.packageBytes)} · SHA-256 {releasePreview.packageSha256.slice(0, 12)}…</dd></div>}
              {releasePreview.ready && <div><dt>Local output</dt><dd title={releasePreview.outputDir}>{releasePreview.outputDir}</dd></div>}
            </dl>
            {releasePreview.ready && (
              <div className="release-assets" aria-label="Reviewed release assets">
                <strong>Reviewed GitHub assets</strong>
                {releasePreview.assets.map((asset, index) => (
                  <span key={asset.fileName} title={asset.path}>
                    <b>{index + 1}. {asset.fileName}</b>
                    <small>{formatBytes(asset.bytes)} · SHA-256 {asset.sha256.slice(0, 12)}…</small>
                  </span>
                ))}
                <span title={releasePreview.manifestPath}><b>{releasePreview.assets.length + 1}. Trusted manifest</b><small>Published after every package asset</small></span>
              </div>
            )}
            {releasePreview.issues.length > 0 && <ul className="safety-issues">{releasePreview.issues.map((issue) => <li key={issue}>{issue}</li>)}</ul>}
            {releasePreview.ready && (
              <>
                <label className="confirmation-row"><input type="checkbox" checked={releaseConfirmed} onChange={(event) => setReleaseConfirmed(event.target.checked)} /><span>I confirm publication of immutable GitHub Release {releasePreview.tag} with exactly these {releasePreview.assets.length} reviewed package asset(s) and the trusted manifest.</span></label>
                {!status?.authenticated && <p className="publish-gate">Run Check GitHub and authenticate before publication. The local preview remains available.</p>}
                <button className="primary-action danger-action" onClick={() => void publishRelease()} disabled={!releaseConfirmed || !status?.authenticated || busy}>
                  {busy ? <RefreshCw className="spin" size={17} /> : <CloudUpload size={17} />} Publish GitHub release
                </button>
              </>
            )}
          </section>
        )}

        {publication && <section className="settings-section panel-card creation-result"><CheckCircle2 /><div><h2>{publication.tag} published</h2><p>{publication.url || publication.message}</p></div></section>}

        <section className="settings-section panel-card release-builder">
          <div className="section-title"><BookOpen /><div><h2>Player public catalogue</h2><p>Build the exact server-free modpack list that Player downloads at startup. Drafts with catalogue visibility disabled stay private.</p></div></div>
          <div className="safety-note publisher-safety"><FileCheck2 size={15} /> The catalogue contains public identity, version, artwork, manifest and deployment metadata only. Local folders, executables, launcher choices, arguments and installed versions are never included.</div>
          <button className="primary-action publisher-preview" onClick={() => void prepareCatalog()} disabled={busy}>
            {busy ? <RefreshCw className="spin" size={17} /> : <BookOpen size={17} />} Prepare public catalogue
          </button>
        </section>

        {catalogPreview && (
          <section className={`settings-section panel-card package-preview ${catalogPreview.ready ? "ready" : "blocked"}`}>
            <div className="section-title">{catalogPreview.ready ? <CheckCircle2 /> : <ShieldAlert />}<div><h2>{catalogPreview.ready ? "Catalogue preview ready" : "Catalogue blocked by safety checks"}</h2><p>{catalogPreview.ready ? "Nothing has been sent to GitHub. Review the complete public profile list before confirmation." : "Fix every visible profile issue or disable its catalogue visibility, then prepare again."}</p></div></div>
            <dl className="pack-facts preview-facts">
              <div><dt>Destination</dt><dd>{catalogPreview.repository} · {catalogPreview.branch}</dd></div>
              <div><dt>Visible modpacks</dt><dd>{catalogPreview.profiles.length}</dd></div>
              <div><dt>Hidden drafts</dt><dd>{catalogPreview.hiddenProfiles}</dd></div>
              {catalogPreview.ready && <div><dt>Catalogue</dt><dd>{formatBytes(catalogPreview.bytes)} · SHA-256 {catalogPreview.sha256.slice(0, 12)}…</dd></div>}
              {catalogPreview.ready && <div><dt>Local preview</dt><dd title={catalogPreview.outputPath}>{catalogPreview.outputPath}</dd></div>}
            </dl>
            {catalogPreview.profiles.length > 0 && (
              <div className="release-assets" aria-label="Public catalogue profiles">
                <strong>Profiles Player will receive</strong>
                {catalogPreview.profiles.map((item) => (
                  <span key={item.id} title={item.manifestUrl}><b>{item.displayName}</b><small>{item.id} · v{item.version}</small></span>
                ))}
              </div>
            )}
            {catalogPreview.issues.length > 0 && <ul className="safety-issues">{catalogPreview.issues.map((issue) => <li key={issue}>{issue}</li>)}</ul>}
            {catalogPreview.ready && (
              <>
                <label className="confirmation-row"><input type="checkbox" checked={catalogConfirmed} onChange={(event) => setCatalogConfirmed(event.target.checked)} /><span>I confirm replacing {catalogPreview.publicUrl} with exactly these {catalogPreview.profiles.length} reviewed public profile(s).</span></label>
                {!status?.authenticated && <p className="publish-gate">Run Check GitHub and authenticate before catalogue publication. The local preview remains available.</p>}
                <button className="primary-action danger-action" onClick={() => void publishCatalog()} disabled={!catalogConfirmed || !status?.authenticated || busy}>
                  {busy ? <RefreshCw className="spin" size={17} /> : <CloudUpload size={17} />} Publish Player catalogue
                </button>
              </>
            )}
          </section>
        )}

        {catalogPublication && <section className="settings-section panel-card creation-result"><CheckCircle2 /><div><h2>Player catalogue published</h2><p>{catalogPublication.message} {catalogPublication.commitUrl || catalogPublication.publicUrl}</p></div></section>}
      </div>
    </main>
  );
}

function guessRepository(manifestUrl: string): string {
  const match = manifestUrl.match(/^https:\/\/github\.com\/([^/]+\/[^/]+)\//i);
  return match?.[1] ?? "";
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
