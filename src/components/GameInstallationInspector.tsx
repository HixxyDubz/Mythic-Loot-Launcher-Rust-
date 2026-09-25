import { useState } from "react";
import { inspectGameInstallation } from "../api";
import type { GameInstallationCheck, GameProfile } from "../types";
import { useOperationScope } from "../useOperationScope";

export function GameInstallationInspector({ profile, disabled }: { profile: GameProfile; disabled: boolean }) {
  return <InspectionSession key={JSON.stringify([profile.id, profile.game, profile.gameDir, profile.installDir, profile.gameExePath])} profile={profile} disabled={disabled} />;
}
function InspectionSession({ profile, disabled }: { profile: GameProfile; disabled: boolean }) {
  const [result, setResult] = useState<GameInstallationCheck | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const scope = useOperationScope();
  async function inspect() {
    if (disabled || busy) return;
    const current = scope(); setBusy(true); setResult(null); setError("");
    try {
      const found = await inspectGameInstallation(profile);
      if (!current()) return;
      if (found.profileId !== profile.id || found.game !== profile.game) throw new Error("The check returned a different profile/game; inspect again.");
      setResult(found);
    } catch (error) { if (current()) setError(error instanceof Error ? error.message : String(error)); }
    finally { if (current()) setBusy(false); }
  }
  return <section className="settings-section panel-card local-game-inspector">
    <h2>Local game folder and version check</h2>
    <p>Checks the paths shown above without saving settings, launching anything or changing files. Steam build IDs are not game version numbers.</p>
    <button type="button" className="secondary-action" disabled={disabled || busy} onClick={() => void inspect()}>{busy ? "Checking local installation…" : "Check local game installation"}</button>
    {error && <p role="alert">{error}</p>}
    {result && <div aria-live="polite">
      <ul>{result.paths.map((entry) => <li key={entry.label}><strong>{entry.label}: {entry.status === "unsafe" ? "Unsafe or unreadable path" : entry.status}</strong><span>{entry.path || "Not selected"}</span></li>)}</ul>
      <dl className="pack-facts">
        <div><dt>Known client layout</dt><dd>{result.clientFound === null ? "Manual check required" : result.clientFound ? "Recognised executable found" : "Recognised executable not found"}</dd></div>
        <div><dt>Declared game version</dt><dd>{result.gameVersion ?? "Cannot determine"}</dd></div>
        <div><dt>Published requirement</dt><dd>{result.requiredGameVersion ?? "No verified requirement"}</dd></div>
        <div><dt>Steam installed build ID</dt><dd>{result.steamBuildId ?? "Unavailable"}</dd></div>
      </dl>
      <strong>{result.comparison === "matches" ? "Declared game version matches the published requirement" : result.comparison === "mismatch" ? "Declared game version differs from the published requirement" : "Game version compatibility cannot be confirmed"}</strong>
      <ul>{result.notes.map((note, index) => <li key={index}>{note}</li>)}</ul>
    </div>}
  </section>;
}
