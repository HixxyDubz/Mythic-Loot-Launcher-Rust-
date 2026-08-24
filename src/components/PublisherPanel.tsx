import { useState } from "react";
import { ArrowLeft, CheckCircle2, CloudUpload, Eye, LockKeyhole, RefreshCw, ShieldAlert } from "lucide-react";
import { createGithubRepository, githubPublisherStatus } from "../api";
import type { PublisherStatus, RepositoryCreation, RepositoryRequest } from "../types";

interface PublisherPanelProps {
  modpackName: string;
  onBack: () => void;
  onNotice: (message: string) => void;
}

export function PublisherPanel({ modpackName, onBack, onNotice }: PublisherPanelProps) {
  const [status, setStatus] = useState<PublisherStatus | null>(null);
  const [repository, setRepository] = useState("");
  const [description, setDescription] = useState(`${modpackName} release repository`);
  const [visibility, setVisibility] = useState<"private" | "public">("private");
  const [previewed, setPreviewed] = useState(false);
  const [confirmed, setConfirmed] = useState(false);
  const [busy, setBusy] = useState(false);
  const [creation, setCreation] = useState<RepositoryCreation | null>(null);

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

  function preview() {
    setConfirmed(false);
    setCreation(null);
    setPreviewed(true);
  }

  async function createRepository() {
    const request: RepositoryRequest = { repository, description, visibility, confirmed };
    setBusy(true);
    try {
      const result = await createGithubRepository(request);
      setCreation(result);
      setConfirmed(false);
      onNotice(result.message);
    } catch (error) {
      onNotice(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  const canPreview = Boolean(status?.authenticated && repository.includes("/") && !busy);

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
            <span><strong>{status?.authenticated ? `Ready as ${status.account || "authenticated user"}` : "Preflight required"}</strong><small>{status?.message || "Check GitHub before preparing a repository mutation."}</small></span>
          </div>
        </section>

        <section className="settings-section panel-card">
          <div className="section-title"><CloudUpload /><div><h2>Repository draft</h2><p>This creates an empty repository only. Packaging and release upload are a separate reviewed step.</p></div></div>
          <div className="form-stack">
            <label className="field"><span>Repository (owner/name)</span><input value={repository} placeholder="HixxyDubz/Mythic-Loot-Modpack" onChange={(event) => { setRepository(event.target.value); setPreviewed(false); }} /></label>
            <label className="field"><span>Description</span><input value={description} maxLength={350} onChange={(event) => { setDescription(event.target.value); setPreviewed(false); }} /></label>
            <label className="field"><span>Visibility</span><select value={visibility} onChange={(event) => { setVisibility(event.target.value as "private" | "public"); setPreviewed(false); }}><option value="private">Private (recommended)</option><option value="public">Public</option></select></label>
          </div>
          <button className="primary-action publisher-preview" onClick={preview} disabled={!canPreview}><Eye size={17} /> Preview repository creation</button>
        </section>

        {previewed && (
          <section className="settings-section panel-card mutation-preview">
            <div className="section-title"><Eye /><div><h2>External change preview</h2><p>Nothing has been created yet.</p></div></div>
            <dl className="pack-facts">
              <div><dt>Repository</dt><dd>{repository}</dd></div>
              <div><dt>Visibility</dt><dd>{visibility}</dd></div>
              <div><dt>Initial files</dt><dd>None</dd></div>
              <div><dt>Action</dt><dd>Create repository on GitHub</dd></div>
            </dl>
            <label className="confirmation-row"><input type="checkbox" checked={confirmed} onChange={(event) => setConfirmed(event.target.checked)} /><span>I confirm that the launcher may create this GitHub repository.</span></label>
            <button className="primary-action danger-action" onClick={() => void createRepository()} disabled={!confirmed || busy}>
              {busy ? <RefreshCw className="spin" size={17} /> : <CloudUpload size={17} />} Create repository
            </button>
          </section>
        )}

        {creation && <section className="settings-section panel-card creation-result"><CheckCircle2 /><div><h2>{creation.repository} created</h2><p>{creation.url || creation.message}</p></div></section>}
      </div>
    </main>
  );
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
