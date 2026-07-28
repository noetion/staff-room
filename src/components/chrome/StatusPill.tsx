export function StatusPill({
  state,
  stage,
  elapsed,
}: {
  state: string;
  stage: string;
  elapsed: string;
}) {
  return (
    <button
      type="button"
      className="status-pill glass--clear"
      data-run-state={state}
      onClick={() => document.getElementById("run-progress-card")?.scrollIntoView({ block: "end", behavior: "smooth" })}
      aria-label={`View ${state.replace("-", " ")} Ship run`}
    >
      <span className="status-pill__dot" aria-hidden="true" />
      <span className="status-pill__stage">{stage}</span>
      <span className="status-pill__elapsed">{elapsed}</span>
    </button>
  );
}
