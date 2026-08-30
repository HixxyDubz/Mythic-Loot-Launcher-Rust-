import { useMemo, useState } from "react";
import { ArrowLeft, CloudUpload, PackagePlus, ShieldCheck } from "lucide-react";
import type { GameDefinition, GameProfile } from "../types";

interface ModpackManagerPanelProps {
  games: GameDefinition[];
  profiles: GameProfile[];
  busy: boolean;
  onBack: () => void;
  onCreate: (profile: GameProfile) => void;
}

export function ModpackManagerPanel({ games, profiles, busy, onBack, onCreate }: ModpackManagerPanelProps) {
  const [displayName, setDisplayName] = useState("");
  const [id, setId] = useState("");
  const [idEdited, setIdEdited] = useState(false);
  const [game, setGame] = useState(games[0]?.id ?? "minecraft");
  const [requiredGameVersion, setRequiredGameVersion] = useState("");
  const [requiredModpackVersion, setRequiredModpackVersion] = useState("1.0.0");
  const [repository, setRepository] = useState("");
  const [sourceDir, setSourceDir] = useState("");
  const [deploymentSubdir, setDeploymentSubdir] = useState("");
  const [logoPath, setLogoPath] = useState(defaultLogo(games[0]?.id ?? "minecraft"));

  const normalizedId = normalizeId(id);
  const duplicate = profiles.some((profile) => profile.id.toLowerCase() === normalizedId.toLowerCase());
  const repositoryValid = !repository.trim() || /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repository.trim());
  const ready = Boolean(displayName.trim() && normalizedId && requiredModpackVersion.trim() && !duplicate && repositoryValid && !busy);
  const manifestUrl = useMemo(
    () => repository.trim() && normalizedId
      ? `https://github.com/${repository.trim()}/releases/latest/download/${normalizedId}-manifest.json`
      : "",
    [repository, normalizedId],
  );

  function updateName(value: string) {
    setDisplayName(value);
    if (!idEdited) setId(normalizeId(value));
  }

  function updateGame(value: string) {
    setGame(value);
    if (value === "seven_days" && !deploymentSubdir) setDeploymentSubdir("Mods");
    if (value === "minecraft" && deploymentSubdir === "Mods") setDeploymentSubdir("");
    setLogoPath(defaultLogo(value));
  }

  function create() {
    if (!ready) return;
    onCreate({
      id: normalizedId,
      game,
      displayName: displayName.trim(),
      requiredGameVersion: requiredGameVersion.trim(),
      requiredModpackVersion: requiredModpackVersion.trim(),
      localModpackVersion: "",
      manifestPath: `manifests/${normalizedId}.json`,
      installDir: sourceDir.trim(),
      gameDir: "",
      gameExePath: "",
      launchArgs: "",
      minecraftLauncher: "",
      discordInvite: "",
      updateSource: "",
      manifestUrl,
      deploymentSubdir: deploymentSubdir.trim(),
      logoPath: logoPath.trim() || "/assets/mythic-loot-logo.jpg",
      catalogVisible: false,
    });
  }

  return (
    <main className="settings-page modpack-manager-page">
      <div className="settings-header">
        <button className="back-button" onClick={onBack}><ArrowLeft size={18} /> Back</button>
        <div><span className="eyebrow">DEVELOPER WORKSPACE</span><h1>Add a modpack</h1></div>
        <button className="primary-action save-button" onClick={create} disabled={!ready}>
          <PackagePlus size={17} /> {busy ? "Creating…" : "Create modpack"}
        </button>
      </div>

      <div className="settings-layout">
        <section className="settings-section panel-card">
          <div className="section-title"><PackagePlus /><div><h2>Public identity</h2><p>Create the catalogue identity first. Local player paths are never published.</p></div></div>
          <div className="form-grid">
            <Field label="Display name" value={displayName} placeholder="Mythic Loot Modpack" onChange={updateName} />
            <Field label="Modpack ID" value={id} placeholder="mythic_loot_modpack" onChange={(value) => { setIdEdited(true); setId(normalizeId(value)); }} />
            <label className="field">
              <span>Game adapter</span>
              <select value={game} onChange={(event) => updateGame(event.target.value)}>
                {games.map((definition) => <option key={definition.id} value={definition.id}>{definition.displayName}</option>)}
              </select>
            </label>
            <Field label="Required game version" value={requiredGameVersion} placeholder="Optional" onChange={setRequiredGameVersion} />
            <Field label="Initial modpack version" value={requiredModpackVersion} placeholder="1.0.0" onChange={setRequiredModpackVersion} />
            <Field label="Deployment subfolder" value={deploymentSubdir} placeholder="For example Mods" onChange={setDeploymentSubdir} />
            <Field label="Public artwork path or URL" value={logoPath} onChange={setLogoPath} />
          </div>
          {duplicate && <p className="form-error">A modpack with this ID already exists.</p>}
        </section>

        <section className="settings-section panel-card">
          <div className="section-title"><CloudUpload /><div><h2>GitHub distribution</h2><p>Use an existing owner/repository now, or leave it empty and create one later in Publisher.</p></div></div>
          <div className="form-stack">
            <Field label="Repository (owner/name)" value={repository} placeholder="HixxyDubz/Mythic-Loot-Modpack" onChange={setRepository} />
            <label className="field"><span>Generated manifest URL</span><input value={manifestUrl} readOnly placeholder="Generated after a repository is entered" /></label>
            <Field label="Developer source folder" value={sourceDir} placeholder="C:\Path\To\Modpack" onChange={setSourceDir} />
          </div>
          {!repositoryValid && <p className="form-error">Repository must use owner/name format.</p>}
        </section>

        <section className="settings-section panel-card creation-next-step">
          <div className="section-title"><ShieldCheck /><div><h2>What happens next</h2><p>The new modpack becomes the selected Developer profile. Use Publisher to privacy-scan, package, preview, and publish its first immutable GitHub release.</p></div></div>
        </section>
      </div>
    </main>
  );
}

export function normalizeId(value: string): string {
  return value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, "_")
    .replace(/^[_-]+|[_-]+$/g, "")
    .slice(0, 64);
}

function defaultLogo(game: string): string {
  if (game === "minecraft") return "/assets/minecraft.png";
  if (game === "seven_days") return "/assets/seven-days.png";
  return "/assets/mythic-loot-logo.jpg";
}

function Field({ label, value, placeholder, onChange }: { label: string; value: string; placeholder?: string; onChange: (value: string) => void }) {
  return (
    <label className="field">
      <span>{label}</span>
      <input value={value} placeholder={placeholder} onChange={(event) => onChange(event.target.value)} />
    </label>
  );
}
