import { describe, expect, it } from "vitest";
import type { StopRunResult } from "./model";
import { isRunRecoverable, shouldShowRunProgress } from "./lib/runs";

describe("run control contract", () => {
  it("keeps a failed cancellation distinct from a cancellation request", () => {
    const unavailable: StopRunResult = {
      cancelled: false,
      reason: "Cancellation is not available yet or the run has already finished.",
    };
    const requested: StopRunResult = {
      cancelled: true,
      reason: "Cancellation requested. The Staff Room will preserve recoverable work.",
    };

    expect(unavailable.cancelled).toBe(false);
    expect(requested.cancelled).toBe(true);
  });

  it("does not place completed or abandoned Ship runs in the chat timeline", () => {
    expect(shouldShowRunProgress({ id: "run-complete", state: "complete" })).toBe(false);
    expect(shouldShowRunProgress({ id: "run-abandoned", state: "abandoned" })).toBe(false);
    expect(shouldShowRunProgress({ id: "run-active", state: "working" })).toBe(true);
    expect(shouldShowRunProgress({ id: "run-recoverable", state: "failed" })).toBe(true);
  });

  it("never offers recovery when an interrupted transition has ambiguous repository state", () => {
    const base = {
      state: "waiting" as const,
      worktreePath: "C:\\worktree",
      recoveryCount: 0,
    };

    expect(isRunRecoverable({ ...base, stopReason: "Verification failed." })).toBe(true);
    expect(isRunRecoverable({ ...base, stopReason: "Promotion state unknown — inspect the repository before continuing." })).toBe(false);
    expect(isRunRecoverable({ ...base, stopReason: "Abandonment state unknown — confirm Abandon again to finish cleanup." })).toBe(false);
  });
});
