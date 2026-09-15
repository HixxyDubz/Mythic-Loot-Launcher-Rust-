import { useEffect, useRef, useState } from "react";
import { Settings } from "lucide-react";
import type { LauncherPreferences } from "../types";

export function PreferencesSection({ preferences, busy, onSave, onRefreshCatalogue, onNotice }: {
  preferences: LauncherPreferences;
  busy: boolean;
  onSave: (preferences: LauncherPreferences) => Promise<void>;
  onRefreshCatalogue: () => void;
  onNotice: (message: string) => void;
}) {
  const [draft, setDraft] = useState(preferences);
  const previousPreferences = useRef(preferences);
  const [saving, setSaving] = useState(false);
  useEffect(() => {
    const previous = previousPreferences.current;
    previousPreferences.current = preferences;
    setDraft((current) => JSON.stringify(current) === JSON.stringify(previous) || JSON.stringify(current) === JSON.stringify(preferences) ? preferences : current);
  }, [preferences]);
  const disabled = busy || saving;
  async function save() {
    if (disabled) return;
    setSaving(true);
    try {
      await onSave(draft);
      onNotice("Launcher preferences saved. Startup update-check changes apply the next time you open the app.");
    } catch (error) { onNotice(error instanceof Error ? error.message : String(error)); }
    finally { setSaving(false); }
  }
  return <section className="settings-section panel-card preferences-section">
    <div className="section-title"><Settings /><div><h2>Launcher preferences</h2><p>Local to this edition, across all your modpacks. Saved separately from modpack settings.</p></div></div>
    <fieldset disabled={disabled}>
      <label className="preference-toggle"><input type="checkbox" checked={draft.autoCheckUpdates} onChange={(event) => setDraft({ ...draft, autoCheckUpdates: event.target.checked })} /><span>Check for app and modpack updates at startup<small>Checks only; installs still require your confirmation. When off, use Refresh catalogue here and App update in the sidebar.</small></span></label>
      <label className="preference-toggle"><input type="checkbox" checked={draft.closeAfterLaunch} onChange={(event) => setDraft({ ...draft, closeAfterLaunch: event.target.checked })} /><span>Close Mythic Loot after a normal launch<small>Also closes after opening your chosen Minecraft launcher. Safe Launch stays open to restore optional files; active work can block closing.</small></span></label>
      <label className="preference-toggle"><input type="checkbox" checked={draft.reduceMotion} onChange={(event) => setDraft({ ...draft, reduceMotion: event.target.checked })} /><span>Reduce motion<small>Stops decorative animations and transitions. Windows reduced-motion preference is also respected.</small></span></label>
      <label className="preference-toggle"><input type="checkbox" checked={draft.decorativeBackground} onChange={(event) => setDraft({ ...draft, decorativeBackground: event.target.checked })} /><span>Show decorative background<small>Turn off for a plain background with less visual distraction.</small></span></label>
      <div className="form-grid">
        <label className="field"><span>Colour theme</span><select value={draft.theme} onChange={(event) => setDraft({ ...draft, theme: event.target.value as LauncherPreferences["theme"] })}><option value="amethyst">Amethyst</option><option value="slate">Slate</option></select></label>
        <label className="field"><span>Interface font</span><select value={draft.font} onChange={(event) => setDraft({ ...draft, font: event.target.value as LauncherPreferences["font"] })}><option value="system">Windows system font</option><option value="verdana">Verdana</option></select></label>
      </div>
    </fieldset>
    <div className="bootstrap-actions">
      <button disabled={disabled} onClick={() => void save()}>{saving ? "Saving preferences…" : "Save launcher preferences"}</button>
      <button disabled={disabled} onClick={onRefreshCatalogue}>Refresh catalogue now</button>
    </div>
  </section>;
}
