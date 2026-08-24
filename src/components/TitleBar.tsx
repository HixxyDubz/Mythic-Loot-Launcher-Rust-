import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauri } from "@tauri-apps/api/core";
import { Minus, Square, X } from "lucide-react";

const nativeWindow = isTauri() ? getCurrentWindow() : null;

export function TitleBar() {
  return (
    <header
      className="titlebar"
      onDoubleClick={() => void nativeWindow?.toggleMaximize()}
      onMouseDown={(event) => {
        if (event.button === 0) void nativeWindow?.startDragging();
      }}
    >
      <div className="titlebar-brand">
        <img src="/assets/mythic-loot-logo.jpg" alt="" />
        <span>Mythic Loot Launcher</span>
        <span className="version-pill">TAURI PREVIEW 0.1</span>
      </div>
      <div className="window-controls" onMouseDown={(event) => event.stopPropagation()}>
        <button aria-label="Minimize" onClick={() => void nativeWindow?.minimize()}>
          <Minus size={16} />
        </button>
        <button aria-label="Maximize" onClick={() => void nativeWindow?.toggleMaximize()}>
          <Square size={13} />
        </button>
        <button className="window-close" aria-label="Close" onClick={() => void nativeWindow?.close()}>
          <X size={17} />
        </button>
      </div>
    </header>
  );
}
