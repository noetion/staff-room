import { CircleStop, Play, Sparkles } from "lucide-react";
import type { ComposerRoute } from "./RouteSwitch";

type SendButtonProps = {
  busy: boolean;
  disabled: boolean;
  mode: ComposerRoute;
  onStop: () => void;
  sideChatAvailable: boolean;
  promoting: boolean;
};

function labelFor(mode: ComposerRoute, sideChatAvailable: boolean, promoting: boolean) {
  if (mode === "ask") return sideChatAvailable ? "Ask active run" : "Ask";
  if (mode === "quick-edit") return "Preview edit";
  if (promoting) return "Promoting";
  return sideChatAvailable ? "Ask active run" : "Run autonomously";
}

export function SendButton({ busy, disabled, mode, onStop, sideChatAvailable, promoting }: SendButtonProps) {
  const label = labelFor(mode, sideChatAvailable, promoting);
  return (
    <button
      type={busy ? "button" : "submit"}
      className={`composer-send${busy ? " is-stopping" : ""}`}
      disabled={busy ? false : disabled}
      aria-busy={mode === "ask" || mode === "quick-edit" ? busy : false}
      onClick={busy ? onStop : undefined}
    >
      <span className="composer-send-fill" aria-hidden="true" />
      <span className="composer-send-content">
        <span className="composer-send-icon" aria-hidden="true">
          {busy ? <CircleStop size={15} /> : mode === "ask" || mode === "quick-edit" ? <Sparkles size={15} /> : <Play size={15} fill="currentColor" />}
        </span>
        <span className="composer-send-label">{busy ? "Stop response" : label}</span>
        {!busy && <kbd>Enter</kbd>}
      </span>
    </button>
  );
}
