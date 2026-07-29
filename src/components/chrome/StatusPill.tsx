import { useEffect, useState } from "react";
import { elapsedTime } from "../../lib/format";

const runningStates = [
  "selecting",
  "working",
  "verifying",
  "reviewing",
  "revising",
  "promoting",
];

export function StatusPill({
  state,
  stage,
  startedAt,
}: {
  state: string;
  stage: string;
  startedAt: string;
}) {
  const running = runningStates.includes(state);
  const [elapsed, setElapsed] = useState(() => elapsedTime(startedAt));

  // A run can sit in one phase for minutes without emitting an event, so the
  // pill drives its own clock instead of waiting for the next incidental render.
  useEffect(() => {
    setElapsed(elapsedTime(startedAt));
    if (!running) return;
    const interval = window.setInterval(() => setElapsed(elapsedTime(startedAt)), 1000);
    return () => window.clearInterval(interval);
  }, [running, startedAt]);

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
