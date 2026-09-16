import { useState } from "react";
import { inspectMinecraftMetadata } from "../api";
import { useOperationScope } from "../useOperationScope";
import type { MinecraftMetadataInspection } from "../types";

interface InspectorProps {
  profileId: string;
  directory: string;
  disabled: boolean;
  onNotice: (message: string) => void;
  onUse?: (gameVersion: string, modLoader: string) => void;
}

export function MinecraftMetadataInspector(props: InspectorProps) {
  // Changing profiles or folders discards both the old evidence and any pending
  // response. A result can never be applied to a different source selection.
  return <InspectorSession key={`${props.profileId}|${props.directory}`} {...props} />;
}

function InspectorSession({ profileId, directory, disabled, onNotice, onUse }: InspectorProps) {
  const [result, setResult] = useState<MinecraftMetadataInspection | null>(null);
  const [checking, setChecking] = useState(false);
  const [error, setError] = useState("");
  const scope = useOperationScope();
  async function inspect() {
    if (disabled || checking || !directory.trim()) return;
    const current = scope();
    setChecking(true); setError(""); setResult(null);
    try {
      const found = await inspectMinecraftMetadata(profileId, directory);
      if (!current()) return;
      if (found.profileId !== profileId) throw new Error("The metadata result belongs to another profile. Inspect again.");
      setResult(found);
    } catch (error) { if (current()) setError(error instanceof Error ? error.message : String(error)); }
    finally { if (current()) setChecking(false); }
  }
  function useDetected() {
    if (!result?.canUse || !result.gameVersion || !result.modLoader || !onUse || disabled || checking) return;
    onUse(result.gameVersion, result.modLoader);
    onNotice("Detected Minecraft version and loader copied into the draft. Review them before saving or preparing a release. Nothing was uploaded.");
  }
  return <div className="minecraft-metadata-inspector">
    <h3>Minecraft version and loader check</h3>
    <p>Reads only recognised metadata in the selected folder. It does not change Minecraft, launchers, files or accounts.</p>
    <button type="button" className="secondary-action" disabled={disabled || checking || !directory.trim()} onClick={() => void inspect()}>{checking ? "Inspecting metadata…" : "Inspect Minecraft metadata"}</button>
    {!directory.trim() && <p>Choose a Minecraft folder first.</p>}
    {error && <p role="alert">{error}</p>}
    {result && <div className="metadata-result" aria-live="polite">
      <dl className="pack-facts">
        <div><dt>Detected Minecraft</dt><dd>{result.gameVersion ?? "Cannot determine"}</dd></div>
        <div><dt>Detected loader</dt><dd>{result.modLoader ?? "Cannot determine"}</dd></div>
        <div><dt>Published requirement</dt><dd>{result.expectedGameVersion ?? "No verified requirement"}{result.expectedModLoader && ` · ${result.expectedModLoader}`}</dd></div>
      </dl>
      <strong>{result.comparison === "matches" ? "Instance metadata matches the published requirement" : result.comparison === "mismatch" ? "Metadata differs from the published requirement" : "Installed compatibility cannot be confirmed"}</strong>
      <p>{result.comparison === "mismatch" ? "For an installed pack, choose the matching Minecraft version and loader in your launcher. For a new release, review whether this difference is intentional." : "This checks declared metadata, not actual game binaries, installed mod compatibility or a successful game launch."}</p>
      <ul>{result.sources.map((source) => <li key={source.fileName}>{source.fileName} — {source.instanceMetadata ? "instance declaration" : "export snapshot (may be outdated)"}: {source.gameVersion} · {source.modLoader}</li>)}</ul>
      {result.issues.length > 0 && <ul className="metadata-issues">{result.issues.map((issue, index) => <li key={index}>{issue}</li>)}</ul>}
      {onUse && <button type="button" className="secondary-action" disabled={disabled || checking || !result.canUse} onClick={useDetected}>Use detected version and loader</button>}
    </div>}
    <p className="detection-note">Supported: minecraftinstance.json, CurseForge export manifest.json and extracted modrinth.index.json. ZIP/.mrpack archives and Modrinth's private database are not read. Missing or ambiguous metadata is never treated as a match.</p>
  </div>;
}
