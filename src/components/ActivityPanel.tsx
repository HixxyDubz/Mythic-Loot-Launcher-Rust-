import { useEffect, useMemo, useState } from "react";
import {
  Activity,
  ArrowLeft,
  CheckCircle2,
  CircleX,
  CloudUpload,
  Download,
  Gamepad2,
  HardDriveDownload,
  LoaderCircle,
  RefreshCw,
  RotateCcw,
  SearchCheck,
  HardDrive,
  Trash2,
  Wrench,
} from "lucide-react";
import { clearFinishedActivity, listActivity } from "../api";
import type { ActivityItem, ActivityKind } from "../types";

interface ActivityPanelProps {
  onBack: () => void;
  onNotice: (message: string) => void;
}

export function ActivityPanel({ onBack, onNotice }: ActivityPanelProps) {
  const [items, setItems] = useState<ActivityItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [clearing, setClearing] = useState(false);
  const finished = useMemo(() => items.filter((item) => item.done).length, [items]);

  useEffect(() => {
    let active = true;
    async function refresh(silent: boolean) {
      try {
        const result = await listActivity();
        if (active) setItems(result);
      } catch (error) {
        if (active && !silent) onNotice(errorMessage(error));
      } finally {
        if (active) setLoading(false);
      }
    }
    void refresh(false);
    const interval = window.setInterval(() => void refresh(true), 1_500);
    return () => {
      active = false;
      window.clearInterval(interval);
    };
  }, [onNotice]);

  async function clearFinished() {
    setClearing(true);
    try {
      const remaining = await clearFinishedActivity();
      setItems(remaining);
      onNotice("Finished activity was cleared. Active operations were preserved.");
    } catch (error) {
      onNotice(errorMessage(error));
    } finally {
      setClearing(false);
    }
  }

  async function refreshNow() {
    setLoading(true);
    try {
      setItems(await listActivity());
    } catch (error) {
      onNotice(errorMessage(error));
    } finally {
      setLoading(false);
    }
  }

  return (
    <main className="settings-page activity-page">
      <div className="settings-header">
        <button className="back-button" onClick={onBack}><ArrowLeft size={18} /> Back</button>
        <div><span className="eyebrow">LAUNCHER OPERATIONS</span><h1>Activity Centre</h1></div>
        <button className="detect-button" onClick={() => void refreshNow()} disabled={loading}>
          <RefreshCw className={loading ? "spin" : ""} size={16} /> Refresh
        </button>
      </div>

      <section className="settings-section panel-card activity-summary">
        <div className="section-title">
          <Activity />
          <div><h2>Recent activity</h2><p>The newest 12 real native operations are shown. Running work refreshes automatically.</p></div>
        </div>
        <button className="secondary-action" onClick={() => void clearFinished()} disabled={finished === 0 || clearing}>
          {clearing ? <LoaderCircle className="spin" size={16} /> : <Trash2 size={16} />} Clear finished
        </button>
      </section>

      {loading && items.length === 0 ? (
        <section className="panel-card activity-empty"><LoaderCircle className="spin" /><h2>Loading activity…</h2></section>
      ) : items.length === 0 ? (
        <section className="panel-card activity-empty"><CheckCircle2 /><h2>No recent activity</h2><p>Everything is idle.</p></section>
      ) : (
        <section className="activity-list" aria-label="Recent launcher activity">
          {items.map((item) => <ActivityRow key={item.id} item={item} />)}
        </section>
      )}
    </main>
  );
}

function ActivityRow({ item }: { item: ActivityItem }) {
  const Icon = kindIcon(item.kind);
  const state = !item.done ? "active" : item.success ? "success" : "failed";
  const status = !item.done ? "Active" : item.success ? "Completed" : "Failed";
  const percent = item.progress === null ? null : Math.round(Math.max(0, Math.min(1, item.progress)) * 100);
  return (
    <article className={`panel-card activity-row ${state}`}>
      <span className="activity-kind-icon"><Icon /></span>
      <div className="activity-copy">
        <header><strong>{item.title}</strong><span>{formatTime(item.updatedAt)}</span></header>
        <div className="activity-meta"><b>{kindLabel(item.kind)}</b><i>·</i><em>{status}</em>{percent !== null && <><i>·</i><em>{percent}%</em></>}</div>
        <p>{item.message || "No additional details were reported."}</p>
        {!item.done && <div className="activity-progress" aria-label="Operation in progress"><span /></div>}
      </div>
      <span className="activity-state-icon">
        {!item.done ? <LoaderCircle className="spin" /> : item.success ? <CheckCircle2 /> : <CircleX />}
      </span>
    </article>
  );
}

function kindIcon(kind: ActivityKind) {
  switch (kind) {
    case "catalogue": return Download;
    case "storage": return HardDrive;
    case "support": return Wrench;
    case "verifying": return SearchCheck;
    case "updating": return HardDriveDownload;
    case "repairing": return Wrench;
    case "restoring": return RotateCcw;
    case "publishing": return CloudUpload;
    case "launching": return Gamepad2;
    case "setup": return Download;
  }
}

function kindLabel(kind: ActivityKind): string {
  switch (kind) {
    case "catalogue": return "Catalogue";
    case "storage": return "Storage";
    case "support": return "Support";
    case "verifying": return "Verifying";
    case "updating": return "Updating";
    case "repairing": return "Repairing";
    case "restoring": return "Restoring";
    case "publishing": return "Publishing";
    case "launching": return "Launching";
    case "setup": return "Setup";
  }
}

function formatTime(timestamp: number): string {
  if (!Number.isFinite(timestamp) || timestamp <= 0) return "Unknown time";
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(timestamp));
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
