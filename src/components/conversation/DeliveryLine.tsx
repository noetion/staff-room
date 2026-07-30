import { useEffect, useState, type ReactNode } from "react";
import { elapsedTime, latencyLabel } from "../../lib/format";
import type { ExecutionReceipt } from "../../model";

export type DeliveryState = "sent" | "working" | "answered" | "failed";

export interface DeliveryLineProps {
  state: DeliveryState;
  startedAt?: string;
  receipt?: ExecutionReceipt;
  cause?: string;
  recoveryAction?: ReactNode;
}

export function DeliveryLine({
  state,
  startedAt,
  receipt,
  cause,
  recoveryAction,
}: DeliveryLineProps) {
  const [, setTick] = useState(0);

  useEffect(() => {
    if (state !== "working") return;
    const interval = window.setInterval(() => setTick((value) => value + 1), 1000);
    return () => window.clearInterval(interval);
  }, [state]);

  if (state === "working") {
    return <span className="delivery-line">Working · {elapsedTime(startedAt ?? "")}</span>;
  }

  if (state === "answered" && receipt?.firstOutputMs != null) {
    return (
      <span className="delivery-line">
        Answered · {latencyLabel(receipt)} first token
      </span>
    );
  }

  if (state === "failed") {
    return (
      <span className="delivery-line delivery-line-failed">
        Failed — {cause ?? "Provider run failed"}
        {recoveryAction}
      </span>
    );
  }

  return <span className="delivery-line">Sent</span>;
}
