import { Activity, ChevronRight, CloudUpload, Gamepad2, HardDrive, LifeBuoy, Plus, RefreshCw, Settings } from "lucide-react";
import type { GameProfile, ProfileHealth } from "../types";

interface SidebarProps {
  profiles: GameProfile[];
  health: ProfileHealth[];
  selectedId: string;
  edition: "player" | "developer";
  publisherAvailable: boolean;
  onSelect: (id: string) => void;
  onSettings: () => void;
  onActivity: () => void;
  onStorage: () => void;
  onSupport: () => void;
  onAppUpdate: () => void;
  onPublisher: () => void;
  onAddModpack: () => void;
}

export function Sidebar({ profiles, health, selectedId, edition, publisherAvailable, onSelect, onSettings, onActivity, onStorage, onSupport, onAppUpdate, onPublisher, onAddModpack }: SidebarProps) {
  return (
    <aside className="sidebar">
      <div className="brand-lockup">
        <img src="/assets/mythic-loot-wordmark.jpg" alt="Mythic Loot" />
        <p>MODPACK LAUNCHER</p>
      </div>
      <div className="sidebar-heading">
        <span>Your modpacks</span>
        <span>{profiles.length}</span>
      </div>
      <nav className="profile-list" aria-label="Modpack profiles">
        {profiles.map((profile) => {
          const state = health.find((item) => item.profileId === profile.id);
          const selected = profile.id === selectedId;
          return (
            <button
              className={`profile-button ${selected ? "selected" : ""}`}
              key={profile.id}
              onClick={() => onSelect(profile.id)}
            >
              <span className="profile-art">
                <img src={profile.logoPath || "/assets/mythic-loot-logo.jpg"} alt="" />
              </span>
              <span className="profile-copy">
                <strong>{profile.displayName}</strong>
                <small>
                  <i className={`status-dot ${state?.status ?? "checking"}`} />
                  {state?.headline ?? "Checking"}
                </small>
              </span>
              <ChevronRight size={16} />
            </button>
          );
        })}
      </nav>
      {publisherAvailable && (
        <button className="add-modpack" onClick={onAddModpack}>
          <Plus size={16} /> Add modpack
        </button>
      )}
      <div className="sidebar-footer">
        <button onClick={onActivity}>
          <Activity size={17} /> Activity
        </button>
        <button onClick={onStorage}>
          <HardDrive size={17} /> Storage
        </button>
        <button onClick={onSupport}>
          <LifeBuoy size={17} /> Support
        </button>
        <button onClick={onAppUpdate}>
          <RefreshCw size={17} /> App update
        </button>
        {publisherAvailable && (
          <button onClick={onPublisher}>
            <CloudUpload size={17} /> Publisher
          </button>
        )}
        <button onClick={onSettings}>
          <Settings size={17} /> Settings
        </button>
        <span><Gamepad2 size={15} /> {edition === "developer" ? "Developer edition" : "Player edition"}</span>
      </div>
    </aside>
  );
}
