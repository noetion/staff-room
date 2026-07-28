export function AutonomySetting({ enabled, disabled, onChange }: { enabled: boolean; disabled: boolean; onChange: (enabled: boolean) => void }) {
  return <section className="autonomy-setting" aria-label="Autonomous Ship">
    <div><strong>Hands-free autonomous Ship</strong><p>Locked until the autonomous acceptance contract passes. Human-gated Ship remains available and always requires explicit promotion.</p></div>
    <div className="autonomy-control"><span className="status-chip">{enabled ? "Armed" : "Disarmed"}</span><label className="switch-control"><input type="checkbox" checked={enabled} disabled={disabled} onChange={(event) => onChange(event.target.checked)} /><span>{enabled ? "Armed" : "Off"}</span></label></div>
  </section>;
}
