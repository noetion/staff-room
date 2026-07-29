import type { VerificationConfig } from "../../model";

export function VerificationSettings({ config, disabled, saving = false, onChange, onSave }: { config: VerificationConfig; disabled: boolean; saving?: boolean; onChange: (config: VerificationConfig) => void; onSave: () => void }) {
  return <section className="verification-setting" aria-labelledby="verification-settings-title">
    <div><strong id="verification-settings-title">Verification</strong><p>Commands are detected once when a repository is attached. Edit them here; they will not be replaced automatically.</p></div>
    <label className="switch-control"><input type="checkbox" checked={config.enabled} disabled={disabled} onChange={(event) => onChange({ ...config, enabled: event.target.checked })} /><span>{config.enabled ? "Enabled" : "Disabled"}</span></label>
    <label className="verification-field"><span>Prepare command</span><input value={config.prepare ?? ""} disabled={disabled} placeholder="Optional command before checks" onChange={(event) => onChange({ ...config, prepare: event.target.value || undefined })} /></label>
    <div className="verification-commands">{config.commands.map((command, index) => <div className="verification-command" key={`${command.label}-${index}`}>
      <input value={command.label} aria-label={`Verification label ${index + 1}`} disabled={disabled} onChange={(event) => onChange({ ...config, commands: config.commands.map((value, valueIndex) => valueIndex === index ? { ...value, label: event.target.value } : value) })} />
      <input value={command.command} aria-label={`Verification command ${index + 1}`} disabled={disabled} onChange={(event) => onChange({ ...config, commands: config.commands.map((value, valueIndex) => valueIndex === index ? { ...value, command: event.target.value } : value) })} />
      <label className="switch-control"><input type="checkbox" checked={command.enabled} disabled={disabled} onChange={(event) => onChange({ ...config, commands: config.commands.map((value, valueIndex) => valueIndex === index ? { ...value, enabled: event.target.checked } : value) })} /><span>Run</span></label>
      <button type="button" className="secondary-button" aria-label={`Remove verification command ${index + 1}`} disabled={disabled} onClick={() => onChange({ ...config, commands: config.commands.filter((_, valueIndex) => valueIndex !== index) })}>Remove</button>
    </div>)}</div>
    <div className="verification-actions"><button type="button" className="secondary-button" disabled={disabled || config.commands.length >= 4} onClick={() => onChange({ ...config, commands: [...config.commands, { label: "Project check", command: "", enabled: true }] })}>Add check</button><button type="button" className="primary-button" disabled={disabled} aria-busy={saving || undefined} onClick={onSave}>{saving ? "Saving…" : "Save verification"}</button></div>
  </section>;
}
