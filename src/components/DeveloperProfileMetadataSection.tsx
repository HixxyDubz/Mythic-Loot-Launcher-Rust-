import { GitBranch, PackageOpen } from "lucide-react";
import type { GameDefinition, GameProfile } from "../types";

interface DeveloperProfileMetadataSectionProps {
  draft: GameProfile;
  games: GameDefinition[];
  onUpdate: <K extends keyof GameProfile>(key: K, value: GameProfile[K]) => void;
}

export function DeveloperProfileMetadataSection({ draft, games, onUpdate }: DeveloperProfileMetadataSectionProps) {
  return (
    <>
      <section className="settings-section panel-card">
        <div className="section-title"><PackageOpen /><div><h2>Public modpack identity</h2><p>Developer-owned metadata that is published to Player editions.</p></div></div>
        <div className="form-grid">
          <Field label="Display name" value={draft.displayName} onChange={(value) => onUpdate("displayName", value)} />
          <label className="field">
            <span>Game adapter</span>
            <select value={draft.game} onChange={(event) => onUpdate("game", event.target.value)}>
              {games.map((game) => <option key={game.id} value={game.id}>{game.displayName}</option>)}
            </select>
          </label>
          <Field label="Required game version" value={draft.requiredGameVersion} onChange={(value) => onUpdate("requiredGameVersion", value)} />
          <Field label="Required modpack version" value={draft.requiredModpackVersion} onChange={(value) => onUpdate("requiredModpackVersion", value)} />
          <Field label="Deployment subfolder" value={draft.deploymentSubdir} placeholder="For example Mods" onChange={(value) => onUpdate("deploymentSubdir", value)} />
          <Field label="Public artwork path or URL" value={draft.logoPath} placeholder="/assets/my-modpack.png" onChange={(value) => onUpdate("logoPath", value)} />
        </div>
      </section>

      <section className="settings-section panel-card">
        <div className="section-title"><GitBranch /><div><h2>Distribution channel</h2><p>Developer-owned GitHub locations used to discover and download modpack updates.</p></div></div>
        <div className="form-stack">
          <Field label="Manifest URL" value={draft.manifestUrl} placeholder="https://github.com/owner/repo/releases/latest/download/manifest.json" onChange={(value) => onUpdate("manifestUrl", value)} />
          <Field label="Package URL override" value={draft.updateSource} placeholder="Normally supplied by the published manifest" onChange={(value) => onUpdate("updateSource", value)} />
          <Field label="Local manifest path" value={draft.manifestPath} onChange={(value) => onUpdate("manifestPath", value)} />
          <Field label="Discord invitation" value={draft.discordInvite} placeholder="Optional public community link" onChange={(value) => onUpdate("discordInvite", value)} />
        </div>
      </section>
    </>
  );
}

function Field({ label, value, placeholder, onChange }: { label: string; value: string; placeholder?: string; onChange: (value: string) => void }) {
  return (
    <label className="field">
      <span>{label}</span>
      <input value={value} placeholder={placeholder} onChange={(event) => onChange(event.target.value)} />
    </label>
  );
}
