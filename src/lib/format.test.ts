import { describe, expect, it } from "vitest";
import { calendarDay, elapsedTime, millisecondsLabel, parseTimestamp, relativeTime, usageLabel } from "./format";
import type { ExecutionReceipt } from "../model";

/** Render a UTC instant the way SQLite's CURRENT_TIMESTAMP writes it. */
function sqliteTimestamp(date: Date): string {
  return date.toISOString().replace("T", " ").slice(0, 19);
}

describe("parseTimestamp", () => {
  it("reads a SQLite CURRENT_TIMESTAMP value as UTC, not local time", () => {
    expect(parseTimestamp("2026-07-29 12:00:00")).toBe(Date.UTC(2026, 6, 29, 12, 0, 0));
  });

  it("reads an ISO instant unchanged", () => {
    expect(parseTimestamp("2026-07-29T12:00:00Z")).toBe(Date.UTC(2026, 6, 29, 12, 0, 0));
  });

  it("respects an explicit offset instead of forcing UTC", () => {
    expect(parseTimestamp("2026-07-29T12:00:00+01:00")).toBe(Date.UTC(2026, 6, 29, 11, 0, 0));
  });

  it("reports an unparseable value as NaN rather than throwing", () => {
    expect(Number.isNaN(parseTimestamp(""))).toBe(true);
  });
});

describe("elapsedTime", () => {
  it("counts up from a SQLite timestamp instead of clamping to zero", () => {
    const started = sqliteTimestamp(new Date(Date.now() - 90_000));
    expect(elapsedTime(started)).toBe("1m 30s elapsed");
  });

  it("agrees with the equivalent ISO timestamp", () => {
    const ninetySecondsAgo = new Date(Date.now() - 90_000);
    expect(elapsedTime(sqliteTimestamp(ninetySecondsAgo)))
      .toBe(elapsedTime(ninetySecondsAgo.toISOString()));
  });

  it("falls back to a neutral label when the run has no start time", () => {
    expect(elapsedTime("")).toBe("just started");
  });
});

describe("relativeTime", () => {
  it("reads both timestamp shapes the same way", () => {
    const twoHoursAgo = new Date(Date.now() - 2 * 60 * 60 * 1000);
    expect(relativeTime(sqliteTimestamp(twoHoursAgo))).toBe("2h");
    expect(relativeTime(twoHoursAgo.toISOString())).toBe("2h");
  });
});

describe("calendarDay", () => {
  it("buckets both timestamp shapes into the same local day", () => {
    const now = new Date();
    expect(calendarDay(sqliteTimestamp(now))).toBe(calendarDay(now.toISOString()));
  });
});

describe("nullable receipt formatting", () => {
  it("keeps JSON null timing and usage fields readable", () => {
    const receipt = {
      id: "receipt",
      phase: "chat",
      participant: "codex",
      contextBytes: 0,
      usage: {
        inputTokens: null,
        cachedInputTokens: null,
        outputTokens: null,
        totalCostUsd: null,
        numTurns: null,
      },
      usageNote: "Provider did not report usage.",
      createdAt: "2026-07-30T00:00:00Z",
      preflightMs: null,
      totalMs: null,
    } as unknown as ExecutionReceipt;

    expect(millisecondsLabel(receipt.preflightMs)).toBe("—");
    expect(usageLabel(receipt)).toBe("Not reported");
  });
});
