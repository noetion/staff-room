import type { AgentKind, ExecutionReceipt } from "../model";

export function initials(kind?: AgentKind): string { if (!kind) return "AR"; return { codex: "CX", claude: "CL", cursor: "CU", antigravity: "AG" }[kind]; }

/**
 * SQLite writes `CURRENT_TIMESTAMP` as `YYYY-MM-DD HH:MM:SS` in UTC with no zone
 * marker, and `new Date(...)` reads that shape as *local* time. Every timestamp
 * that reaches the renderer must go through here so a run started one minute ago
 * never renders as an hour in the future — or, once clamped, as a frozen `0s`.
 */
export function parseTimestamp(timestamp: string): number {
  if (!timestamp) return Number.NaN;
  const normalized = /^\d{4}-\d{2}-\d{2}[ T]\d{2}:\d{2}:\d{2}/.test(timestamp) && !/[zZ]|[+-]\d{2}:?\d{2}$/.test(timestamp)
    ? `${timestamp.replace(" ", "T")}Z`
    : timestamp;
  return new Date(normalized).getTime();
}

/** Stable `YYYY-M-D` key in the viewer's local zone, used for day grouping. */
export function calendarDay(timestamp: string): string {
  const time = parseTimestamp(timestamp);
  if (Number.isNaN(time)) return "unknown";
  const date = new Date(time);
  return `${date.getFullYear()}-${date.getMonth()}-${date.getDate()}`;
}

export function relativeTime(timestamp: string): string { const time = parseTimestamp(timestamp); if (Number.isNaN(time)) return "now"; const minutes = Math.max(0, Math.round((Date.now() - time) / 60_000)); if (minutes < 1) return "now"; if (minutes < 60) return `${minutes}m`; return `${Math.floor(minutes / 60)}h`; }
export function elapsedTime(timestamp: string): string { const started = parseTimestamp(timestamp); if (!started || Number.isNaN(started)) return "just started"; const seconds = Math.max(0, Math.floor((Date.now() - started) / 1000)); return seconds < 60 ? `${seconds}s elapsed` : `${Math.floor(seconds / 60)}m ${seconds % 60}s elapsed`; }
export function contextLabel(bytes = 0, budgetBytes = 0): string { if (!bytes) return "Packet not assembled"; const usage = `${Math.max(1, Math.round(bytes / 1024))} KiB`; return budgetBytes ? `${usage} / ${Math.round(budgetBytes / 1024)} KiB` : usage; }
function isFiniteNumber(value: number | null | undefined): value is number { return typeof value === "number" && Number.isFinite(value); }
export function usageLabel(receipt: ExecutionReceipt): string { const usage = receipt.usage ?? {}; const { inputTokens, cachedInputTokens, outputTokens, totalCostUsd, numTurns } = usage; const parts = [isFiniteNumber(inputTokens) ? `${inputTokens.toLocaleString()} in` : undefined, isFiniteNumber(cachedInputTokens) ? `${cachedInputTokens.toLocaleString()} cached` : undefined, isFiniteNumber(outputTokens) ? `${outputTokens.toLocaleString()} out` : undefined, isFiniteNumber(totalCostUsd) ? `$${totalCostUsd.toFixed(4)}` : undefined, isFiniteNumber(numTurns) ? `${numTurns} turns` : undefined].filter(Boolean); return parts.length ? parts.join(" · ") : "Not reported"; }
export function latencyLabel(receipt: ExecutionReceipt): string | undefined { if (!isFiniteNumber(receipt.totalMs)) return undefined; const total = (receipt.totalMs / 1000).toFixed(2); const first = !isFiniteNumber(receipt.firstOutputMs) ? "first output unavailable" : `${(receipt.firstOutputMs / 1000).toFixed(2)}s to first output`; const preflight = !isFiniteNumber(receipt.preflightMs) ? undefined : `${(receipt.preflightMs / 1000).toFixed(2)}s preflight`; return [`${total}s total`, first, preflight].filter(Boolean).join(" · "); }
export function millisecondsLabel(value: number | null | undefined): string { return !isFiniteNumber(value) ? "—" : `${value.toLocaleString()} ms`; }
