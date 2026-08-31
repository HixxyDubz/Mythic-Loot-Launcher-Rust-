import { useState } from "react";
import type { ReactNode } from "react";
import {
  ArrowRight,
  Check,
  CircleAlert,
  Clock3,
  Download,
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
  onOpenSmartLaunch: () => void;
  onVerifyFiles: () => void;
  onOpenUpdates: () => void;
  onOpenSafeLaunch: () => void;
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

export function Dashboard({ profile, health, manifest, verification, busy, onOpenSettings, onOpenSmartLaunch, onVerifyFiles, onOpenUpdates, onOpenSafeLaunch }: DashboardProps) {
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
                <button className="primary-action" onClick={ready ? onOpenSmartLaunch : onOpenSettings} disabled={busy}>
                  {busy ? <RefreshCw className="spin" size={18} /> : ready ? <Gamepad2 size={19} /> : <Settings2 size={19} />}
                  {busy ? "Working…" : ready ? "Smart Launch" : "Complete setup"}
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
              <button onClick={onOpenSmartLaunch} disabled={busy || !profile.installDir || !manifest.valid || manifest.requiredFileCount === 0}>
                <Gamepad2 /> Smart Launch <small>Check, maintain, recheck, open</small>
              </button>
              <button onClick={onOpenUpdates} disabled={busy || !profile.installDir || !manifest.valid}>
                <Download /> {profile.game === "minecraft" ? "Sync, update & repair" : "Update & repair"} <small>Staged native transaction</small>
              </button>
              <button onClick={onVerifyFiles} disabled={busy || !profile.installDir || !manifest.valid || manifest.requiredFileCount === 0}>
                <ShieldCheck /> Verify files <small>SHA-256 manifest check</small>
              </button>
              <button onClick={onOpenSafeLaunch} disabled={busy || !profile.installDir || !manifest.valid}>
                <Gamepad2 /> Safe Launch <small>Optional extras isolated</small>
              </button>
              <button onClick={onOpenSettings}><Settings2 /> Settings <small>Paths & updates</small></button>
            </div>
          </section>
        </div>
      ) : tab === "news" ? <NewsContent manifest={manifest} />
        : tab === "changelog" ? <ChangelogContent manifest={manifest} />
          : <RulesContent manifest={manifest} />}
    </main>
  );
}

function NewsContent({ manifest }: { manifest: ManifestSummary }) {
  return (
    <section className="content-page panel-card">
      {manifest.newsBannerUrl && <img className="news-banner" src={manifest.newsBannerUrl} alt="Modpack news" />}
      <div className="content-heading"><Globe2 /><div><span className="eyebrow">LATEST NEWS</span><h2>Announcements</h2></div></div>
      {manifest.announcement.trim()
        ? <p className="announcement-copy">{manifest.announcement}</p>
        : <EmptyContent icon={<Globe2 />} title="No announcements right now" detail="This modpack has not published a current announcement." />}
    </section>
  );
}

function ChangelogContent({ manifest }: { manifest: ManifestSummary }) {
  return (
    <section className="content-page panel-card">
      <div className="content-heading"><Clock3 /><div><span className="eyebrow">VERSION HISTORY</span><h2>Changelog</h2></div></div>
      {manifest.changelog.length ? <div className="changelog-list">{manifest.changelog.map((entry, index) => (
        <article className="changelog-entry" key={`${entry.version}-${entry.date}-${index}`}>
          <header><strong>v{entry.version}</strong><span>{entry.date || "Date not specified"}</span></header>
          {entry.notes && <p>{entry.notes}</p>}
          <ChangeGroup label="Added" items={entry.added} />
          <ChangeGroup label="Changed" items={entry.changed} />
          <ChangeGroup label="Fixed" items={entry.fixed} />
        </article>
      ))}</div> : <EmptyContent icon={<Clock3 />} title="No changelog available" detail="Release history will appear here when it is published in the trusted manifest." />}
    </section>
  );
}

function ChangeGroup({ label, items }: { label: string; items: string[] }) {
  if (!items.length) return null;
  return <div className="change-group"><b>{label}</b><ul>{items.map((item, index) => <li key={`${item}-${index}`}>{item}</li>)}</ul></div>;
}

function RulesContent({ manifest }: { manifest: ManifestSummary }) {
  const guide = manifest.rulesGuide;
  const empty = !guide.howToJoin.trim() && !guide.rules.length && !guide.commonFixes.length;
  return (
    <section className="content-page panel-card">
      <div className="content-heading"><ShieldCheck /><div><span className="eyebrow">PLAYER GUIDE</span><h2>Rules & common fixes</h2></div></div>
      {empty ? <EmptyContent icon={<ShieldCheck />} title="No guide is configured" detail="This modpack has not published rules or common fixes." /> : <div className="rules-grid">
        {guide.howToJoin && <article><h3>How to get started</h3><p>{guide.howToJoin}</p></article>}
        {guide.rules.length > 0 && <article><h3>Rules</h3><ol>{guide.rules.map((rule, index) => <li key={`${rule}-${index}`}>{rule}</li>)}</ol></article>}
        {guide.commonFixes.length > 0 && <article><h3>Common fixes</h3><ul>{guide.commonFixes.map((fix, index) => <li key={`${fix}-${index}`}>{fix}</li>)}</ul></article>}
      </div>}
    </section>
  );
}

function EmptyContent({ icon, title, detail }: { icon: ReactNode; title: string; detail: string }) {
  return <div className="content-empty"><div className="placeholder-icon">{icon}</div><h3>{title}</h3><p>{detail}</p></div>;
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
