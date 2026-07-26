import { describe, expect, it } from "vitest";
import type { StopRunResult } from "./model";

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
});
