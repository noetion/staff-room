export function AttentionCard({ title, detail, onDismiss }: { title: string; detail: string; onDismiss: () => void }) {
  return (
    <aside className="attention-card" role="alert">
      <span className="attention-mark" aria-hidden="true">!</span>
      <span><strong>{title}</strong><span>{detail}</span><small>Review the room status or dismiss this notice.</small></span>
      <button type="button" onClick={onDismiss} aria-label="Dismiss attention notice">×</button>
    </aside>
  );
}
