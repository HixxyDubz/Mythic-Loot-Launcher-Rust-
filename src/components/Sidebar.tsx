import { ChevronRight, CloudUpload, Gamepad2, Plus, Settings } from "lucide-react";
import type { GameProfile, ProfileHealth } from "../types";

interface SidebarProps {
  profiles: GameProfile[];
  health: ProfileHealth[];
  selectedId: string;
  onSelect: (id: string) => void;
  onSettings: () => void;
  onPublisher: () => void;
}

export function Sidebar({ profiles, health, selectedId, onSelect, onSettings, onPublisher }: SidebarProps) {
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
      <button className="add-modpack" disabled title="Modpack creation is scheduled for the publishing workspace">
        <Plus size={16} /> Add modpack
      </button>
      <div className="sidebar-footer">
        <button onClick={onPublisher}>
          <CloudUpload size={17} /> Publisher
        </button>
        <button onClick={onSettings}>
          <Settings size={17} /> Settings
        </button>
        <span><Gamepad2 size={15} /> Rust core connected</span>
      </div>
    </aside>
  );
}
