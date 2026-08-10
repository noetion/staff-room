import type { Run, RunState, StoredRun } from "../model";

const validStates: RunState[] = ["ready", "selecting", "working", "verifying", "reviewing", "revising", "awaiting-promotion", "promoting", "waiting", "complete", "failed", "stopped", "abandoned"];
const visibleProgressStates: RunState[] = ["selecting", "working", "verifying", "reviewing", "revising", "awaiting-promotion", "promoting", "waiting", "failed", "stopped"];
const ambiguousStatePrefixes = ["Promotion state unknown", "Abandonment state unknown"];
export function asRunState(value: string): RunState { return validStates.includes(value as RunState) ? (value as RunState) : "working"; }
export function hydrateRun(stored: StoredRun): Run { return { id: stored.id, objective: stored.objective, state: stored.state, currentOwner: stored.currentOwner, route: stored.route, reviewCount: stored.reviewCount, revisionCount: stored.revisionCount, startedAt: stored.startedAt, stopReason: stored.stopReason, nativeSessionId: stored.nativeSessionId, writer: stored.writer, reviewer: stored.reviewer, degradedReview: stored.degradedReview, worktreePath: stored.worktreePath, branch: stored.branch, contextBytes: stored.contextBytes, artifactPath: stored.artifactPath, instructionFiles: stored.instructionFiles, skillFiles: stored.skillFiles, recoveryCount: stored.recoveryCount }; }
export function shouldShowRunProgress(run: Pick<Run, "id" | "state">): boolean { return Boolean(run.id) && visibleProgressStates.includes(run.state); }
export function isRunRecoverable(run: Pick<Run, "state" | "worktreePath" | "recoveryCount" | "stopReason">): boolean {
  const ambiguousState = run.state === "waiting"
    && ambiguousStatePrefixes.some((prefix) => run.stopReason?.startsWith(prefix));
  return Boolean(run.worktreePath)
    && ["waiting", "failed", "stopped"].includes(run.state)
    && (run.recoveryCount ?? 0) < 2
    && !ambiguousState;
}
