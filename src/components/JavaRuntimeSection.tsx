import { useRef, useState } from "react";
import { Coffee } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { detectJavaRuntimes, prepareJavaArguments } from "../api";
import { useOperationScope } from "../useOperationScope";
import type { GameProfile, JavaDiscovery } from "../types";

export function JavaRuntimeSection({ profile, busy, onChange, onNotice }: {
  profile: GameProfile;
  busy: boolean;
  onChange: (profile: GameProfile) => void;
  onNotice: (message: string) => void;
}) {
  const [scan, setScan] = useState<JavaDiscovery | null>(null);
  const [working, setWorking] = useState(false);
  const [preset, setPreset] = useState("4096");
  const [custom, setCustom] = useState("6144");
  const latest = useRef(profile);
  const scope = useOperationScope();
  latest.current = profile;
  const external = Boolean(profile.minecraftLauncher) || (Boolean(profile.gameExePath) && !/[/\\]javaw?\.exe$/i.test(profile.gameExePath.trim()));
  const direct = !external && /[/\\]javaw?\.exe$/i.test(profile.gameExePath.trim());
  async function discover() {
    const current = scope();
    setWorking(true);
    try { const found = await detectJavaRuntimes(); if (current()) setScan(found); }
    catch (error) { if (current()) onNotice(message(error)); }
    finally { if (current()) setWorking(false); }
  }
  async function memory() {
    const input = profile;
    const current = scope();
    const mb = preset === "automatic" ? null : Number(preset === "custom" ? custom : preset);
    if (mb !== null && (!Number.isInteger(mb) || mb < 512 || mb > 65536)) {
      onNotice("Choose a whole number between 512 and 65536 MiB, leaving memory for Windows.");
      return;
    }
    setWorking(true);
    try {
      const launchArgs = await prepareJavaArguments(input, mb);
      if (!current()) return;
      if (latest.current !== input) { onNotice("The modpack settings changed. Apply memory again to the current draft."); return; }
      onChange({ ...input, launchArgs });
      onNotice("Java memory arguments updated in the draft. Review Launch arguments, then Save settings to keep them.");
    } catch (error) { if (current()) onNotice(message(error)); }
    finally { if (current()) setWorking(false); }
  }
  async function guide(url: string) {
    try { await openUrl(url); } catch (error) { onNotice(message(error)); }
  }
  return <section className="settings-section panel-card java-section">
    <div className="section-title"><Coffee /><div><h2>Minecraft Java and memory</h2><p>Use the Java version required by your modpack. No automatic runtime download or account changes.</p></div></div>
    <p className="detection-note">CurseForge and Modrinth manage Java and memory themselves. Settings here do not rewrite those apps' configuration or change other Java programs.</p>
    <div className="bootstrap-actions">
      <button onClick={() => void guide("https://support.curseforge.com/support/solutions/articles/9000218572-getting-started")}>CurseForge Java / memory guide</button>
      <button onClick={() => void guide("https://support.modrinth.com/en/articles/8797659-java-installations")}>Modrinth Java guide</button>
      <button disabled={busy || working} onClick={() => void discover()}>{working ? "Working…" : "Find installed Java runtimes"}</button>
    </div>
    {scan && <div className="runtime-results">
      <p>{scan.runtimes.length ? `Found ${scan.runtimes.length} runtime(s). Versions below are declared in each runtime's release file, not verified by executing Java.` : "No Java runtimes found in the searched locations. Your launcher can install a compatible runtime; you can also browse for an existing executable above."}</p>
      {scan.limited && <p role="status">The search reached its time or entry limit. Additional runtimes may exist outside these results.</p>}
      {scan.runtimes.map((runtime) => <div className="runtime-result" key={runtime.executable}>
        <strong>Java {runtime.version || "version unknown"} {runtime.vendor} {runtime.architecture}</strong>
        <small>{runtime.source}</small>
        <input aria-label={`Java path ${runtime.version || runtime.executable}`} value={runtime.executable} readOnly />
        {!external && <button className="secondary-action" disabled={busy || working} onClick={() => onChange({ ...profile, gameExePath: runtime.executable, minecraftLauncher: "" })}>Use for advanced direct Java launch</button>}
      </div>)}
    </div>}
    {external ? <>
      <p className="safety-note">Your selected executable opens another launcher. Set memory in that launcher's Minecraft or profile settings; Java flags will not be added to its command line.</p>
      <details className="detection-note"><summary>Advanced: switch to direct Java</summary>
        <p>This clears the launcher executable and arguments in the unsaved draft. You must supply a Java executable and a complete Minecraft command yourself. Changes take effect only after Save settings.</p>
        <button className="secondary-action" disabled={busy || working} onClick={() => onChange({ ...profile, gameExePath: "", launchArgs: "", minecraftLauncher: "" })}>Clear launch target for direct Java setup</button>
      </details>
    </> : <>
      <p className="detection-note">Advanced direct Java only: choose java.exe or javaw.exe above and supply a complete main-class or JAR command. This does not construct Minecraft authentication or library arguments. Relative files use your Game directory (or Modpack base folder when blank).</p>
      <div className="form-grid">
        <label className="field"><span>Memory preset to apply</span><select disabled={busy || working || !direct} value={preset} onChange={(event) => setPreset(event.target.value)}>
          <option value="automatic">Automatic — remove explicit heap flags</option><option value="4096">4 GiB (4096 MiB)</option><option value="6144">6 GiB (6144 MiB)</option><option value="8192">8 GiB (8192 MiB)</option><option value="custom">Custom</option>
        </select></label>
        {preset === "custom" && <label className="field"><span>Maximum Java heap (MiB)</span><input type="number" min="512" max="65536" step="1" value={custom} disabled={busy || working || !direct} onChange={(event) => setCustom(event.target.value)} /></label>}
      </div>
      <p className="detection-note">This editor replaces explicit JVM heap flags before the main class/JAR, not application arguments. Java @argument files are not expanded. Leave enough RAM for Windows and other programs.</p>
      <button className="secondary-action" disabled={busy || working || !direct} onClick={() => void memory()}>Update launch arguments</button>
    </>}
  </section>;
}

function message(error: unknown) { return error instanceof Error ? error.message : String(error); }
