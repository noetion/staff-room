import { Mic, PackageOpen } from "lucide-react";
import type { VoiceStatus } from "../../native";

export function VoiceSettings({
  status,
  disabled,
  onPickEngine,
  onPickModel,
}: {
  status: VoiceStatus;
  disabled: boolean;
  onPickEngine: () => void;
  onPickModel: () => void;
}) {
  return (
    <section className="voice-setting" aria-label="Local voice dictation">
      <div>
        <strong>Local push-to-talk</strong>
        <p>
          Hold Dictate to capture up to {status.maxSeconds} seconds. The transcript is inserted
          into the composer for editing and is never sent automatically.
        </p>
        <p className="voice-status" role="status">{status.detail}</p>
      </div>
      <div className="voice-setting-actions">
        <span className={`status-chip ${status.available ? "connection-connected" : "connection-unverified"}`}>
          {status.available ? "Ready" : "Setup required"}
        </span>
        <button type="button" className="secondary-button" disabled={disabled} onClick={onPickEngine}>
          <PackageOpen aria-hidden="true" size={15} />
          Choose whisper CLI
        </button>
        <button type="button" className="secondary-button" disabled={disabled} onClick={onPickModel}>
          <Mic aria-hidden="true" size={15} />
          Choose ggml model
        </button>
      </div>
    </section>
  );
}
