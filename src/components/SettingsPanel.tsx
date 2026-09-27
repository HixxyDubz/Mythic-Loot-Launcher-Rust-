import { useEffect, useRef, useState } from "react";
import { ArrowLeft, Check, HardDrive, Radar, RefreshCw, Save, X } from "lucide-react";
import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
import { EditionProfileMetadataSection, launcherEdition } from "@launcher-edition";
import { PathField } from "./PathField";
import { PreferencesSection } from "./PreferencesSection";
import { JavaRuntimeSection } from "./JavaRuntimeSection";
import { MinecraftMetadataInspector } from "./MinecraftMetadataInspector";
import { GameInstallationInspector } from "./GameInstallationInspector";
import type { LauncherPreferences } from "../types";
import type { DetectedInstall, GameDefinition, GameProfile, MinecraftBootstrapArtifact, MinecraftBootstrapRequest, MinecraftLauncher } from "../types";

interface SettingsPanelProps {
  profile: GameProfile;
  games: GameDefinition[];
  dataDir: string;
  busy: boolean;
  candidates: DetectedInstall[];
  onBack: () => void;
  onDetect: (profile: GameProfile) => void;
  onSave: (profile: GameProfile) => void;
  onPrepareMinecraftBootstrap: (request: MinecraftBootstrapRequest) => Promise<MinecraftBootstrapArtifact>;
  onNotice: (message: string) => void;
  preferences: LauncherPreferences;
  onSavePreferences: (preferences: LauncherPreferences) => Promise<void>;
  onRefreshCatalogue: () => void;
}

export function SettingsPanel({
  profile,
  games,
  dataDir,
  busy,
  candidates,
  onBack,
  onDetect,
  onSave,
  onPrepareMinecraftBootstrap,
  onNotice,
  preferences,
  onSavePreferences,
  onRefreshCatalogue,
}: SettingsPanelProps) {
  const [draft, setDraft] = useState(profile);
  const previousProfile = useRef(profile);
  const [bootstrapArtifact, setBootstrapArtifact] = useState<MinecraftBootstrapArtifact | null>(null);
  const [preparingLauncher, setPreparingLauncher] = useState<MinecraftLauncher | null>(null);
  const [detectionStale, setDetectionStale] = useState(false);
  useEffect(() => {
    const previous = previousProfile.current;
    previousProfile.current = profile;
    // Explicit catalogue refresh may return fresh profile objects while the
    // user is editing paths. Keep an edited draft; adopt refreshes when clean.
    setDraft((current) => current === previous ? profile : current);
    setBootstrapArtifact(null);
  }, [profile]);

  const update = <K extends keyof GameProfile>(key: K, value: GameProfile[K]) => {
    if (["game", "gameDir", "gameExePath", "launchArgs", "installDir", "deploymentSubdir"].includes(key)) setDetectionStale(true);
    setDraft((current) => ({ ...current, [key]: value }));
  };

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
          <span className="eyebrow">{launcherEdition === "developer" ? "DEVELOPER MODPACK SETTINGS" : "LOCAL PLAYER SETTINGS"}</span>
          <h1>{profile.displayName}</h1>
        </div>
        <button className="primary-action save-button" onClick={() => onSave(draft)} disabled={busy}>
          <Save size={17} /> {busy ? "Saving…" : "Save settings"}
        </button>
      </div>

      <div className="settings-layout">
        <PreferencesSection preferences={preferences} busy={busy} onSave={onSavePreferences} onRefreshCatalogue={onRefreshCatalogue} onNotice={onNotice} />
        <EditionProfileMetadataSection draft={draft} games={games} onUpdate={update} />

        <section className="settings-section panel-card">
          <div className="section-title">
            <HardDrive />
            <div><h2>{draft.game === "minecraft" ? "Launcher sync target" : "Game and modpack"}</h2><p>Detected paths stay local to this computer.</p></div>
            <button className="detect-button" onClick={() => { setDetectionStale(false); onDetect(draft); }} disabled={busy}><Radar size={16} /> Detect installs</button>
          </div>
          {draft.game === "minecraft" && (
            <div className="minecraft-sync-note">
              <RefreshCw size={17} />
              <div>
                <strong>CurseForge and Modrinth are supported sync targets</strong>
                <p>Create or import a profile using the Minecraft version and loader declared by this modpack, run detection, then select that profile below. The import buttons use the trusted release metadata. Update &amp; Repair syncs only trusted manifest files and leaves saves, logs, screenshots, options and launcher account data alone.</p>
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
            <PathField label="Game or launcher executable" kind="executable" value={draft.gameExePath} placeholder="C:\Path\To\Game.exe" disabled={busy} onNotice={onNotice} onChange={(value) => update("gameExePath", value)} />
            <PathField label="Game directory" value={draft.gameDir} placeholder="Optional separate game data directory" disabled={busy} onNotice={onNotice} onChange={(value) => update("gameDir", value)} />
            <PathField label="Modpack base folder" value={draft.installDir} placeholder="Folder managed by Mythic Loot" disabled={busy} onNotice={onNotice} onChange={(value) => update("installDir", value)} />
            <label className="field"><span>Installed modpack version</span><input value={draft.localModpackVersion || "Not verified"} readOnly /></label>
            <label className="field"><span>Launch arguments</span><input value={draft.launchArgs} placeholder="Optional Windows command arguments" disabled={busy} onChange={(event) => update("launchArgs", event.target.value)} /></label>
          </div>

          {candidates.length > 0 && !detectionStale && (
            <div className="detection-results">
              <div className="results-title"><Radar size={16} /> Detected installations <span>{candidates.length}</span></div>
              {candidates.map((candidate) => {
                const modpackDir = candidate.modpackDir ?? detectedModpackBase(candidate.installDir, draft.deploymentSubdir);
                const selected = pathsEqual(draft.installDir, modpackDir) && pathsEqual(draft.gameExePath, candidate.exePath ?? "");
                const syncTarget = draft.game === "minecraft" && isMinecraftSyncTarget(candidate.source);
                return (
                  <button
                    key={`${candidate.source}-${candidate.installDir}-${modpackDir}-${candidate.exePath ?? ""}`}
                    className={selected ? "selected" : ""}
                    disabled={busy}
                    onClick={() => setDraft((current) => ({
                      ...current,
                      installDir: modpackDir,
                      gameDir: candidate.installDir,
                      gameExePath: candidate.exePath ?? "",
                      minecraftLauncher: current.game === "minecraft" && syncTarget ? candidate.source : "",
                    }))}
                  >
                    <span>
                      <strong>{candidate.label}</strong>
                      <small>{!modpackDir ? `${candidate.installDir} · Modpack folder unresolved — choose manually` : !pathsEqual(candidate.installDir, modpackDir) ? `${candidate.installDir} · manages ${modpackDir}` : candidate.installDir}</small>
                      {candidate.targetNote && <small>{candidate.targetNote}</small>}
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
          {detectionStale && <p className="detection-note">Settings changed. Run detection again before choosing an installation.</p>}
          {draft.game === "factorio" && <p className="detection-note">Factorio detection reads the active config-path.cfg/config.ini or absolute --config/-c and --mod-directory overrides in these launch arguments. It does not inspect Steam launch options. Missing or unsupported configuration stays manual; no game files are changed.</p>}
          {draft.game === "hytale" && <p className="detection-note">Select the matching patchline in Hytale Launcher. For custom installations, use its Settings → Open Directory → User Data, then choose the Mods folder here. Detection does not switch patchlines or configure servers.</p>}
        </section>

        {draft.game === "minecraft" && <section className="settings-section panel-card"><MinecraftMetadataInspector profileId={profile.id} directory={draft.installDir} disabled={busy} onNotice={onNotice} /></section>}
        {draft.game === "minecraft" && <JavaRuntimeSection profile={draft} busy={busy} onChange={setDraft} onNotice={onNotice} />}
        {draft.game !== "minecraft" && <GameInstallationInspector profile={draft} disabled={busy} />}

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
