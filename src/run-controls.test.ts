import { describe, expect, it } from "vitest";
import type { StopRunResult } from "./model";
import { shouldShowRunProgress } from "./lib/runs";

describe("run control contract", () => {
  it("keeps a failed cancellation distinct from a cancellation request", () => {
    const unavailable: StopRunResult = {
      cancelled: false,
      reason: "Cancellation is not available yet or the run has already finished.",
    };
    const requested: StopRunResult = {
      cancelled: true,
      reason: "Cancellation requested. Agent Room will preserve recoverable work.",
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
});
