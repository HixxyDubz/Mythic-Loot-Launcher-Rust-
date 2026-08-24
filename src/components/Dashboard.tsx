import { useState } from "react";
import {
  ArrowRight,
  Check,
  CircleAlert,
  Clock3,
  Download,
  FolderOpen,
  Gamepad2,
  Globe2,
  PackageOpen,
  RefreshCw,
  Settings2,
  ShieldCheck,
  Sparkles,
  Wrench,
} from "lucide-react";
import type { FileVerification, GameProfile, ManifestSummary, ProfileHealth, ReadinessStatus } from "../types";

type DetailTab = "overview" | "news" | "changelog" | "rules";

interface DashboardProps {
  profile: GameProfile;
  health: ProfileHealth;
  manifest: ManifestSummary;
  verification?: FileVerification;
  busy: boolean;
  onOpenSettings: () => void;
  onPlay: () => void;
  onVerifyFiles: () => void;
}

const labels: Record<ReadinessStatus, string> = {
  ready: "READY",
  updateRequired: "UPDATE REQUIRED",
  repairNeeded: "REPAIR NEEDED",
  gamePathMissing: "PATH MISSING",
  setupRequired: "SETUP REQUIRED",
  checking: "CHECKING",
  failed: "CHECK FAILED",
};

export function Dashboard({ profile, health, manifest, verification, busy, onOpenSettings, onPlay, onVerifyFiles }: DashboardProps) {
  const [tab, setTab] = useState<DetailTab>("overview");
  const ready = health.status === "ready";
  const verificationValue = verification
    ? `${verification.current}/${verification.checked} current`
    : `${manifest.requiredFileCount.toLocaleString()} tracked`;

  return (
    <main className="dashboard">
      <div className="dashboard-topline">
        <div>
          <span className="eyebrow">MODPACK OVERVIEW</span>
          <h1>{profile.displayName}</h1>
        </div>
        <div className={`readiness-pill ${health.status}`}>
          <i /> {labels[health.status]}
        </div>
      </div>

      <div className="detail-tabs" role="tablist">
        {(["overview", "news", "changelog", "rules"] as DetailTab[]).map((name) => (
          <button
            key={name}
            className={tab === name ? "active" : ""}
            onClick={() => setTab(name)}
          >
            {name[0].toUpperCase() + name.slice(1)}
          </button>
        ))}
      </div>

      {tab === "overview" ? (
        <div className="overview-grid">
          <section className="hero-card">
            <div className="hero-glow" />
            <div className="hero-copy">
              <span className="eyebrow"><Sparkles size={13} /> YOUR NEXT ADVENTURE</span>
              <h2>{health.headline}</h2>
              <p>
                {ready
                  ? "This local installation matches the trusted modpack version and is ready to launch."
                  : "Finish the highlighted local setup before launching the game."}
              </p>
              <div className="hero-actions">
                <button className="primary-action" onClick={ready ? onPlay : onOpenSettings} disabled={busy}>
                  {busy ? <RefreshCw className="spin" size={18} /> : ready ? <Gamepad2 size={19} /> : <Settings2 size={19} />}
                  {busy ? "Working…" : ready ? "Play now" : "Complete setup"}
                  <ArrowRight size={18} />
                </button>
                <button className="secondary-action" onClick={onOpenSettings}>
                  <Wrench size={17} /> Configure
                </button>
              </div>
            </div>
            <img className="hero-art" src={profile.logoPath || "/assets/mythic-loot-logo.jpg"} alt="" />
          </section>

          <section className="readiness-card panel-card">
            <div className="panel-heading">
              <div>
                <span className="eyebrow">READINESS</span>
                <h3>Is this installation current?</h3>
              </div>
              {ready ? <ShieldCheck className="good" /> : <CircleAlert className="warn" />}
            </div>
            <div className="check-list">
              <ReadinessRow
                label="Game client"
                value={profile.gameExePath ? "Configured" : "Needs setup"}
                complete={Boolean(profile.gameExePath)}
              />
              <ReadinessRow
                label="Modpack folder"
                value={profile.installDir ? "Configured" : "Needs setup"}
                complete={Boolean(profile.installDir)}
              />
              <ReadinessRow
                label="Trusted manifest"
                value={manifest.valid ? `v${manifest.manifestVersion} · ${manifest.requiredFileCount.toLocaleString()} files` : "Validation failed"}
                complete={manifest.valid}
              />
              <ReadinessRow
                label="Modpack version"
                value={profile.localModpackVersion || "Not verified"}
                complete={
                  Boolean(profile.localModpackVersion) &&
                  profile.localModpackVersion === manifest.modpackVersion
                }
              />
            </div>
          </section>

          <section className="pack-card panel-card">
            <div className="panel-heading">
              <div>
                <span className="eyebrow">PACKAGE</span>
                <h3>Modpack details</h3>
              </div>
              <PackageOpen />
            </div>
            <dl className="pack-facts">
              <div><dt>Game adapter</dt><dd>{profile.game}</dd></div>
              <div><dt>Game version</dt><dd>{profile.requiredGameVersion || "Not specified"}</dd></div>
              <div><dt>Pack version</dt><dd>{manifest.modpackVersion || profile.requiredModpackVersion || "Not specified"}</dd></div>
              <div><dt>Manifest files</dt><dd>{verificationValue}</dd></div>
              <div><dt>Released</dt><dd>{manifest.releaseDate || "Not specified"}</dd></div>
              <div><dt>Update channel</dt><dd>{profile.manifestUrl.includes("github.com") ? "GitHub Releases" : "Not configured"}</dd></div>
            </dl>
          </section>

          <section className="quick-card panel-card">
            <div className="panel-heading">
              <div>
                <span className="eyebrow">TOOLS</span>
                <h3>Quick actions</h3>
              </div>
            </div>
            <div className="quick-actions">
              <button disabled><Download /> Update <small>Port scheduled</small></button>
              <button onClick={onVerifyFiles} disabled={busy || !profile.installDir || !manifest.valid || manifest.requiredFileCount === 0}>
                <ShieldCheck /> Verify files <small>SHA-256 manifest check</small>
              </button>
              <button disabled><FolderOpen /> Open files <small>Native scope next</small></button>
              <button onClick={onOpenSettings}><Settings2 /> Settings <small>Paths & updates</small></button>
            </div>
          </section>
        </div>
      ) : (
        <section className="migration-placeholder panel-card">
          <div className="placeholder-icon">
            {tab === "news" ? <Globe2 /> : tab === "rules" ? <ShieldCheck /> : <Clock3 />}
          </div>
          <span className="eyebrow">TRUSTED MANIFEST READY</span>
          <h2>{tab[0].toUpperCase() + tab.slice(1)} content is the next migration slice</h2>
          <p>
            This navigation is in place, but no placeholder production content is being presented as a finished feature.
            Manifest v{manifest.manifestVersion} passed validation; structured modpack {tab} rendering is not wired into this screen yet.
          </p>
        </section>
      )}
    </main>
  );
}

function ReadinessRow({
  label,
  value,
  complete,
  pending = false,
}: {
  label: string;
  value: string;
  complete: boolean;
  pending?: boolean;
}) {
  return (
    <div className="readiness-row">
      <span className={`check-icon ${complete ? "complete" : pending ? "pending" : "missing"}`}>
        {complete ? <Check size={14} /> : pending ? <Clock3 size={14} /> : <CircleAlert size={14} />}
      </span>
      <span><strong>{label}</strong><small>{value}</small></span>
    </div>
  );
}
