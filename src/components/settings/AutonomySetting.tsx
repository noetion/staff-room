export function AutonomySetting({ enabled, disabled, onChange }: { enabled: boolean; disabled: boolean; onChange: (enabled: boolean) => void }) {
  return <section className="autonomy-setting" aria-label="Autonomous Ship">
    <div><strong>Hands-free autonomous Ship</strong><p>When armed, an agent can apply the autonomous-ship skill and start the isolated Ship workflow without another approval. Verification, review, bounded recovery, and promotion gates remain coordinator-owned.</p></div>
    <div className="autonomy-control"><span className="status-chip">{enabled ? "Armed" : "Disarmed"}</span><label className="switch-control"><input type="checkbox" checked={enabled} disabled={disabled} onChange={(event) => onChange(event.target.checked)} /><span>{enabled ? "Armed" : "Off"}</span></label></div>
  </section>;
}
