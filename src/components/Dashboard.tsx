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
  RefreshCw,
  Server,
  Settings2,
  ShieldCheck,
  Sparkles,
  Wrench,
} from "lucide-react";
import type { GameProfile, ProfileHealth, ReadinessStatus } from "../types";

type DetailTab = "overview" | "news" | "changelog" | "rules";

interface DashboardProps {
  profile: GameProfile;
  health: ProfileHealth;
  busy: boolean;
  onOpenSettings: () => void;
  onPlay: () => void;
}

const labels: Record<ReadinessStatus, string> = {
  ready: "READY",
  updateRequired: "UPDATE REQUIRED",
  repairNeeded: "REPAIR NEEDED",
  serverOffline: "SERVER OFFLINE",
  gamePathMissing: "PATH MISSING",
  setupRequired: "SETUP REQUIRED",
  checking: "CHECKING",
  failed: "CHECK FAILED",
};

export function Dashboard({ profile, health, busy, onOpenSettings, onPlay }: DashboardProps) {
  const [tab, setTab] = useState<DetailTab>("overview");
  const ready = health.status === "ready";
  const serverAddress = profile.serverIp
    ? `${profile.serverIp}:${profile.serverPort}`
    : "Not configured";

  return (
    <main className="dashboard">
      <div className="dashboard-topline">
        <div>
          <span className="eyebrow">SERVER OVERVIEW</span>
          <h1>{profile.serverName || profile.displayName}</h1>
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
                  ? "The configured client and modpack version passed the current Rust readiness gates."
                  : "Finish the highlighted setup before the launcher will start this game."}
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
                <h3>Can I play right now?</h3>
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
                label="Modpack version"
                value={profile.localModpackVersion || "Not verified"}
                complete={
                  Boolean(profile.localModpackVersion) &&
                  profile.localModpackVersion === profile.requiredModpackVersion
                }
              />
              <ReadinessRow
                label="Live server check"
                value={profile.serverIp ? "Protocol port pending" : "Not configured"}
                complete={false}
                pending
              />
            </div>
          </section>

          <section className="server-card panel-card">
            <div className="panel-heading">
              <div>
                <span className="eyebrow">CONNECTION</span>
                <h3>Server details</h3>
              </div>
              <Server />
            </div>
            <dl className="server-facts">
              <div><dt>Address</dt><dd>{serverAddress}</dd></div>
              <div><dt>Game version</dt><dd>{profile.requiredGameVersion || "Not specified"}</dd></div>
              <div><dt>Required pack</dt><dd>{profile.requiredModpackVersion || "Not specified"}</dd></div>
              <div><dt>Direct join</dt><dd>{["seven_days", "factorio"].includes(profile.game) ? "Supported" : "In-game assist"}</dd></div>
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
              <button disabled><RefreshCw /> Repair <small>Port scheduled</small></button>
              <button disabled><FolderOpen /> Open files <small>Native scope next</small></button>
              <button onClick={onOpenSettings}><Settings2 /> Settings <small>Paths & server</small></button>
            </div>
          </section>
        </div>
      ) : (
        <section className="migration-placeholder panel-card">
          <div className="placeholder-icon">
            {tab === "news" ? <Globe2 /> : tab === "rules" ? <ShieldCheck /> : <Clock3 />}
          </div>
          <span className="eyebrow">MLLP PARITY MIGRATION</span>
          <h2>{tab[0].toUpperCase() + tab.slice(1)} is queued for the manifest phase</h2>
          <p>
            This navigation is in place, but no placeholder production content is being presented as a finished feature.
            The Rust manifest validator and trusted content pipeline come first.
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
