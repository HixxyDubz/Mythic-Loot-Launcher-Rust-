import { useEffect, useState } from "react";
import { ArrowLeft, Check, GitBranch, HardDrive, PackageOpen, Radar, RefreshCw, Save, X } from "lucide-react";
import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
import type { DetectedInstall, GameProfile, MinecraftBootstrapArtifact, MinecraftBootstrapRequest, MinecraftLauncher } from "../types";

interface SettingsPanelProps {
  profile: GameProfile;
  dataDir: string;
  busy: boolean;
  candidates: DetectedInstall[];
  onBack: () => void;
  onDetect: (profile: GameProfile) => void;
  onSave: (profile: GameProfile) => void;
  onPrepareMinecraftBootstrap: (request: MinecraftBootstrapRequest) => Promise<MinecraftBootstrapArtifact>;
  onNotice: (message: string) => void;
}

export function SettingsPanel({
  profile,
  dataDir,
  busy,
  candidates,
  onBack,
  onDetect,
  onSave,
  onPrepareMinecraftBootstrap,
  onNotice,
}: SettingsPanelProps) {
  const [draft, setDraft] = useState(profile);
  const [bootstrapArtifact, setBootstrapArtifact] = useState<MinecraftBootstrapArtifact | null>(null);
  const [preparingLauncher, setPreparingLauncher] = useState<MinecraftLauncher | null>(null);
  useEffect(() => {
    setDraft(profile);
    setBootstrapArtifact(null);
  }, [profile]);

  const update = <K extends keyof GameProfile>(key: K, value: GameProfile[K]) =>
    setDraft((current) => ({ ...current, [key]: value }));

  async function prepareBootstrap(launcher: MinecraftLauncher) {
    setPreparingLauncher(launcher);
    try {
      const artifact = await onPrepareMinecraftBootstrap({ profileId: draft.id, launcher });
      setBootstrapArtifact(artifact);
      onNotice(artifact.message);
    } catch (error) {
      onNotice(error instanceof Error ? error.message : String(error));
    } finally {
      setPreparingLauncher(null);
    }
  }

  async function openBootstrap(artifact: MinecraftBootstrapArtifact) {
    try {
      await openPath(artifact.path);
    } catch (error) {
      onNotice(`The import file could not be opened automatically: ${error instanceof Error ? error.message : String(error)}`);
    }
  }

  async function revealBootstrap(artifact: MinecraftBootstrapArtifact) {
    try {
      await revealItemInDir(artifact.path);
    } catch (error) {
      onNotice(`The import file could not be shown in Explorer: ${error instanceof Error ? error.message : String(error)}`);
    }
  }

  return (
    <main className="settings-page">
      <div className="settings-header">
        <button className="back-button" onClick={onBack}><ArrowLeft size={18} /> Back</button>
        <div>
          <span className="eyebrow">MODPACK SETTINGS</span>
          <h1>{profile.displayName}</h1>
        </div>
        <button className="primary-action save-button" onClick={() => onSave(draft)} disabled={busy}>
          <Save size={17} /> {busy ? "Saving…" : "Save settings"}
        </button>
      </div>

      <div className="settings-layout">
        <section className="settings-section panel-card">
          <div className="section-title"><PackageOpen /><div><h2>Modpack identity</h2><p>The name and game shown to players.</p></div></div>
          <div className="form-grid">
            <Field label="Display name" value={draft.displayName} onChange={(value) => update("displayName", value)} />
            <Field label="Game adapter" value={draft.game} onChange={(value) => update("game", value)} />
          </div>
        </section>

        <section className="settings-section panel-card">
          <div className="section-title"><GitBranch /><div><h2>Distribution channel</h2><p>Dedicated GitHub release locations used to discover and download modpack updates.</p></div></div>
          <div className="form-stack">
            <Field label="Manifest URL" value={draft.manifestUrl} placeholder="https://github.com/owner/repo/releases/latest/download/manifest.json" onChange={(value) => update("manifestUrl", value)} />
            <Field label="Package URL" value={draft.updateSource} placeholder="https://github.com/owner/repo/releases/download/v1.0.0/modpack.zip" onChange={(value) => update("updateSource", value)} />
            <Field label="Local manifest path" value={draft.manifestPath} onChange={(value) => update("manifestPath", value)} />
          </div>
        </section>

        <section className="settings-section panel-card">
          <div className="section-title">
            <HardDrive />
            <div><h2>{draft.game === "minecraft" ? "Launcher sync target" : "Game and modpack"}</h2><p>Detected paths stay local to this computer.</p></div>
            <button className="detect-button" onClick={() => onDetect(draft)} disabled={busy}><Radar size={16} /> Detect installs</button>
          </div>
          {draft.game === "minecraft" && (
            <div className="minecraft-sync-note">
              <RefreshCw size={17} />
              <div>
                <strong>CurseForge and Modrinth are supported sync targets</strong>
                <p>Create or import a Minecraft {draft.requiredGameVersion || "1.21.1"} NeoForge profile in your chosen launcher, run detection, then select that profile below. Update &amp; Repair syncs only trusted manifest files and leaves saves, logs, screenshots, options and launcher account data alone.</p>
                <small>{draft.minecraftLauncher ? `Selected launcher: ${launcherLabel(draft.minecraftLauncher)}` : "No launcher profile selected yet."}</small>
                <div className="bootstrap-actions">
                  <button onClick={() => void prepareBootstrap("curseforge")} disabled={Boolean(preparingLauncher)}>
                    {preparingLauncher === "curseforge" ? "Preparing…" : "Prepare CurseForge import"}
                  </button>
                  <button onClick={() => void prepareBootstrap("modrinth")} disabled={Boolean(preparingLauncher)}>
                    {preparingLauncher === "modrinth" ? "Preparing…" : "Prepare Modrinth import"}
                  </button>
                </div>
                {bootstrapArtifact && (
                  <div className="bootstrap-result">
                    <strong>{bootstrapArtifact.fileName}</strong>
                    <span>{formatBytes(bootstrapArtifact.bytes)} · SHA-256 {bootstrapArtifact.sha256}</span>
                    <p>{bootstrapArtifact.launcher === "curseforge" ? "In CurseForge choose Import, then select this ZIP." : "Open this .mrpack with Modrinth to create the empty managed profile."} After import, detect the profile here and run Sync, update &amp; repair.</p>
                    <div>
                      <button onClick={() => void openBootstrap(bootstrapArtifact)}>Open import file</button>
                      <button onClick={() => void revealBootstrap(bootstrapArtifact)}>Show in Explorer</button>
                    </div>
                  </div>
                )}
              </div>
            </div>
          )}
          <div className="form-stack">
            <Field label="Game or launcher executable" value={draft.gameExePath} placeholder="C:\Path\To\Game.exe" onChange={(value) => update("gameExePath", value)} />
            <Field label="Game directory" value={draft.gameDir} placeholder="Optional separate game data directory" onChange={(value) => update("gameDir", value)} />
            <Field label="Modpack base folder" value={draft.installDir} placeholder="Folder managed by Mythic Loot" onChange={(value) => update("installDir", value)} />
            <div className="form-grid">
              <Field label="Installed modpack version" value={draft.localModpackVersion} placeholder="Not verified" onChange={(value) => update("localModpackVersion", value)} />
              <Field label="Required modpack version" value={draft.requiredModpackVersion} onChange={(value) => update("requiredModpackVersion", value)} />
            </div>
            <Field label="Launch arguments" value={draft.launchArgs} placeholder="Optional Windows command arguments" onChange={(value) => update("launchArgs", value)} />
          </div>

          {candidates.length > 0 && (
            <div className="detection-results">
              <div className="results-title"><Radar size={16} /> Detected installations <span>{candidates.length}</span></div>
              {candidates.map((candidate) => {
                const modpackDir = detectedModpackBase(candidate.installDir, draft.deploymentSubdir);
                const selected = pathsEqual(draft.installDir, modpackDir) && (!candidate.exePath || pathsEqual(draft.gameExePath, candidate.exePath));
                const syncTarget = draft.game === "minecraft" && isMinecraftSyncTarget(candidate.source);
                return (
                  <button
                    key={`${candidate.source}-${candidate.installDir}`}
                    className={selected ? "selected" : ""}
                    onClick={() => setDraft((current) => ({
                      ...current,
                      installDir: modpackDir,
                      gameDir: candidate.installDir,
                      gameExePath: candidate.exePath ?? current.gameExePath,
                      minecraftLauncher: current.game === "minecraft" && syncTarget ? candidate.source : "",
                    }))}
                  >
                    <span>
                      <strong>{candidate.label}</strong>
                      <small>{draft.deploymentSubdir ? `${candidate.installDir} · manages ${modpackDir}` : candidate.installDir}</small>
                    </span>
                    {selected ? <Check size={17} /> : <span className="use-label">{syncTarget ? "Use as sync target" : "Use"}</span>}
                  </button>
                );
              })}
            </div>
          )}
          {!busy && candidates.length === 0 && (
            <p className="detection-note">Run detection to search supported launcher and Steam locations. Manual paths always remain available.</p>
          )}
        </section>

        <section className="settings-section panel-card native-data-card">
          <div className="section-title"><HardDrive /><div><h2>Native data location</h2><p>{dataDir}</p></div></div>
          <div className="safety-note"><X size={15} /> Paths and settings are handled by Rust and are not written into bundled application assets.</div>
        </section>
      </div>
    </main>
  );
}

export function detectedModpackBase(gameDir: string, deploymentSubdir: string): string {
  const root = gameDir.trim().replace(/[\\/]+$/, "");
  const subdir = deploymentSubdir.trim().replace(/^[\\/]+|[\\/]+$/g, "");
  if (!root || !subdir) return root;
  const separator = root.includes("\\") ? "\\" : "/";
  return `${root}${separator}${subdir}`;
}

export function isMinecraftSyncTarget(source: string): boolean {
  return source === "curseforge" || source === "modrinth";
}

function launcherLabel(value: string): string {
  return value === "curseforge" ? "CurseForge" : value === "modrinth" ? "Modrinth" : value;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  return `${(bytes / 1024).toFixed(1)} KiB`;
}

function pathsEqual(left: string, right: string): boolean {
  return left.replace(/\//g, "\\").toLowerCase() === right.replace(/\//g, "\\").toLowerCase();
}

function Field({
  label,
  value,
  placeholder,
  type = "text",
  onChange,
}: {
  label: string;
  value: string;
  placeholder?: string;
  type?: "text" | "number";
  onChange: (value: string) => void;
}) {
  return (
    <label className="field">
      <span>{label}</span>
      <input type={type} value={value} placeholder={placeholder} onChange={(event) => onChange(event.target.value)} />
    </label>
  );
}
