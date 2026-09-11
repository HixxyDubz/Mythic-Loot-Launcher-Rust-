import { Activity, ChevronRight, CloudUpload, Gamepad2, HardDrive, LifeBuoy, Plus, RefreshCw, Settings } from "lucide-react";
import type { GameProfile, ProfileHealth } from "../types";
import { useState } from "react";

interface SidebarProps {
  profiles: GameProfile[];
  health: ProfileHealth[];
  selectedId: string;
  edition: "player" | "developer";
  publisherAvailable: boolean;
  disabled?: boolean;
  onSelect: (id: string) => void;
  onSettings: () => void;
  onActivity: () => void;
  onStorage: () => void;
  onSupport: () => void;
  onAppUpdate: () => void;
  onPublisher: () => void;
  onAddModpack: () => void;
}

export function Sidebar({ profiles, health, selectedId, edition, publisherAvailable, disabled, onSelect, onSettings, onActivity, onStorage, onSupport, onAppUpdate, onPublisher, onAddModpack }: SidebarProps) {
  const [showArchived, setShowArchived] = useState(false);
  const archived = edition === "player" ? profiles.filter((p) => !p.catalogVisible) : [];
  const shown = profiles.filter((p) => edition === "developer" || p.catalogVisible || showArchived);
  return (
    <aside className="sidebar">
      <div className="brand-lockup">
        <img src="/assets/mythic-loot-wordmark.jpg" alt="Mythic Loot" />
        <p>MODPACK LAUNCHER</p>
      </div>
      <div className="sidebar-heading">
        <span>Your modpacks</span>
        <span>{shown.length}</span>
      </div>
      <nav className="profile-list" aria-label="Modpack profiles">
        {shown.map((profile) => {
          const state = health.find((item) => item.profileId === profile.id);
          const selected = profile.id === selectedId;
          return (
            <button
              className={`profile-button ${selected ? "selected" : ""}`}
              key={profile.id}
              disabled={disabled}
              onClick={() => onSelect(profile.id)}
            >
              <span className="profile-art">
                <img src={profile.logoPath || "/assets/mythic-loot-logo.jpg"} referrerPolicy="no-referrer" alt="" />
              </span>
              <span className="profile-copy">
                <strong>{profile.displayName}</strong>
                <small>
                  <i className={`status-dot ${state?.status ?? "checking"}`} />
                  {!profile.catalogVisible && edition === "player" ? "Archived · local files kept" : state?.headline ?? "Checking"}
                </small>
              </span>
              <ChevronRight size={16} />
            </button>
          );
        })}
      </nav>
      {archived.length > 0 && <button className="add-modpack" disabled={disabled} onClick={() => setShowArchived(!showArchived)}>{showArchived ? "Hide" : "Show"} archived modpacks ({archived.length})</button>}
      {publisherAvailable && (
        <button className="add-modpack" onClick={onAddModpack} disabled={disabled}>
          <Plus size={16} /> Add modpack
        </button>
      )}
      <div className="sidebar-footer">
        <button onClick={onActivity} disabled={disabled}>
          <Activity size={17} /> Activity
        </button>
        <button onClick={onStorage} disabled={disabled}>
          <HardDrive size={17} /> Storage
        </button>
        <button onClick={onSupport} disabled={disabled}>
          <LifeBuoy size={17} /> Support
        </button>
        <button onClick={onAppUpdate} disabled={disabled}>
          <RefreshCw size={17} /> App update
        </button>
        {publisherAvailable && (
          <button onClick={onPublisher} disabled={disabled}>
            <CloudUpload size={17} /> Publisher
          </button>
        )}
        <button onClick={onSettings} disabled={disabled}>
          <Settings size={17} /> Settings
        </button>
        <span><Gamepad2 size={15} /> {edition === "developer" ? "Developer edition" : "Player edition"}</span>
      </div>
    </aside>
  );
}
