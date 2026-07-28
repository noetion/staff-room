import { Check, CircleStop, History, X } from "lucide-react";
import type { Run, RunState } from "../../model";
import { initials } from "../../lib/format";
import { Chip } from "../primitives";
import { LiveActivityStrip, type LiveActivityItem } from "./LiveActivityStrip";

const activeStates: RunState[] = [
  "selecting",
  "working",
  "verifying",
  "reviewing",
  "revising",
  "promoting",
];

export function RunProgressCard({
  run,
  activity,
  limits,
  onStop,
  onResume,
  onAbandon,
}: {
  run: Run;
  activity: LiveActivityItem[];
  limits: string;
  onStop: () => void;
  onResume: () => void;
  onAbandon: () => void;
}) {
  const running = activeStates.includes(run.state);
  const recoverable =
    Boolean(run.worktreePath) &&
    ["waiting", "failed", "stopped"].includes(run.state) &&
    (run.recoveryCount ?? 0) < 2;

  return (
    <article
      className="run-progress-card"
      data-active={running}
      data-run-state={run.state}
      id="run-progress-card"
      aria-label="Current Ship run"
      aria-live="polite"
    >
      <div className="run-progress-card__header">
        <Chip className="run-progress-card__state">
          <span className="run-state-dot" aria-hidden="true" />
          {run.state.replace("-", " ")}
        </Chip>
        <span className="run-progress-card__objective" title={run.stopReason ?? run.objective}>
          {run.stopReason ?? run.objective}
        </span>
      </div>

      <div className="run-stage-rail" aria-label="Ship stages">
        {run.route.map((step, index) => (
          <div className="run-stage" data-state={step.state} key={`${step.label}-${index}`}>
            <span className="run-stage__node">
              {step.state === "complete" ? <Check aria-hidden="true" /> : step.agent ? initials(step.agent) : index + 1}
            </span>
            <span className="run-stage__label">{step.label}</span>
            {index < run.route.length - 1 && <span className="run-stage__connector" aria-hidden="true" />}
          </div>
        ))}
      </div>

      <LiveActivityStrip activity={activity} />

      <footer className="run-progress-card__footer">
        <span className="run-progress-card__limits">{limits}</span>
        <div className="run-progress-card__actions">
          {running ? (
            <button type="button" className="run-progress-card__action" onClick={onStop}>
              <CircleStop aria-hidden="true" />
              Stop
            </button>
          ) : recoverable ? (
            <>
              <button type="button" className="run-progress-card__action" onClick={onResume}>
                <History aria-hidden="true" />
                Resume
              </button>
              <button type="button" className="run-progress-card__secondary-action" onClick={onAbandon}>
                <X aria-hidden="true" />
                Abandon
              </button>
            </>
          ) : Boolean(run.worktreePath) ? (
            <button type="button" className="run-progress-card__action" onClick={onAbandon}>
              <X aria-hidden="true" />
              Abandon
            </button>
          ) : null}
        </div>
      </footer>
    </article>
  );
}
