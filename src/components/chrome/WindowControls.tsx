import { Minus, Square, X } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";

export function WindowControls({ native }: { native: boolean }) {
  return (
    <div className="chrome-window-controls" role="group" aria-label="Window controls">
      <button
        type="button"
        className="chrome-window-control"
        aria-label="Minimize window"
        disabled={!native}
        title={native ? "Minimize window" : "Inert in browser preview mode"}
        onClick={() => {
          if (native) void getCurrentWindow().minimize();
        }}
      >
        <Minus size={14} strokeWidth={1.7} />
      </button>
      <button
        type="button"
        className="chrome-window-control"
        aria-label="Maximize or restore window"
        disabled={!native}
        title={native ? "Maximize or restore window" : "Inert in browser preview mode"}
        onClick={() => {
          if (native) void getCurrentWindow().toggleMaximize();
        }}
      >
        <Square size={11} strokeWidth={1.7} />
      </button>
      <button
        type="button"
        className="chrome-window-control chrome-window-close"
        aria-label="Close Agent Room"
        disabled={!native}
        title={native ? "Close Agent Room" : "Inert in browser preview mode"}
        onClick={() => {
          if (native) void getCurrentWindow().close();
        }}
      >
        <X size={14} strokeWidth={1.7} />
      </button>
    </div>
  );
}
