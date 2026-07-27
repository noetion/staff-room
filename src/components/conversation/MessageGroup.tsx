import { Fragment, memo, type ReactNode } from "react";
import { agentNames, type AgentKind, type ExecutionReceipt, type RoomMessage, type Run } from "../../model";
import { initials, relativeTime } from "../../lib/format";
import type { MessageGroup as Group } from "../../lib/grouping";
import { Monogram } from "../primitives";
import { DeliveryLine } from "./DeliveryLine";
import { MessageBubble, type MessagePosition, type MessageSide } from "./MessageBubble";
import { SystemNote } from "./SystemNote";

export interface MessageGroupProps {
  group: Group;
  freshIds: ReadonlySet<string>;
  streamingRunId?: string;
  run: Run;
  receipts: ExecutionReceipt[];
  renderMessage: (message: RoomMessage) => ReactNode;
  onResume: () => void;
}

function position(index: number, count: number): MessagePosition {
  if (count === 1) return "only";
  if (index === 0) return "first";
  if (index === count - 1) return "last";
  return "middle";
}

function senderName(message: RoomMessage) {
  if (message.sender === "human") return "You";
  if (message.sender === "system") return "Agent Room";
  return agentNames[message.sender];
}

function receiptAfter(message: RoomMessage, receipts: ExecutionReceipt[]) {
  const createdAt = new Date(message.createdAt).getTime();
  return [...receipts]
    .reverse()
    .find((receipt) => new Date(receipt.createdAt).getTime() >= createdAt);
}

function MessageGroupComponent({
  group,
  freshIds,
  streamingRunId,
  run,
  receipts,
  renderMessage,
  onResume,
}: MessageGroupProps) {
  const first = group.messages[0];
  const last = group.messages[group.messages.length - 1];
  const side: MessageSide = group.kind === "human" ? "out" : "in";
  const labelId = `message-sender-${first.id}`;
  const name = senderName(first);
  const isAgent = group.sender !== "human" && group.sender !== "system";

  if (group.kind === "system") {
    return (
      <article className="message-group message-group-system" aria-labelledby={labelId}>
        <span className="visually-hidden" id={labelId}>{name}</span>
        {group.messages.map((message) => (
          <SystemNote key={message.id} emphasized={message.kind === "status"}>
            {renderMessage(message)}
          </SystemNote>
        ))}
      </article>
    );
  }

  const activeForGroup = Boolean(streamingRunId && last.runId === streamingRunId);
  const receipt = receiptAfter(last, receipts);
  const deliveryState =
    run.id === last.runId && run.state === "failed"
      ? "failed"
      : activeForGroup
        ? "working"
        : receipt?.firstOutputMs !== undefined
          ? "answered"
          : "sent";
  const recoverable =
    run.id === last.runId &&
    Boolean(run.worktreePath) &&
    (run.recoveryCount ?? 0) < 2;

  return (
    <article className="message-group" data-side={side} aria-labelledby={labelId}>
      {group.messages.map((message, index) => {
        const isFirst = index === 0;
        const isLast = index === group.messages.length - 1;
        const isStreaming = Boolean(
          streamingRunId &&
          message.runId === streamingRunId &&
          message.kind === "agent",
        );

        return (
          <Fragment key={message.id}>
            <div className="message-row">
              {side === "in" && (
                <span className="message-avatar-slot">
                  {isLast && isAgent && (
                    <Monogram
                      label={name}
                      className={`conversation-avatar conversation-avatar-${group.sender}`}
                      aria-hidden="true"
                    >
                      {initials(group.sender as AgentKind)}
                    </Monogram>
                  )}
                </span>
              )}
              <div className="message-stack">
                {isFirst && (
                  <span
                    className={side === "out" ? "visually-hidden" : "message-sender"}
                    id={labelId}
                  >
                    {name}
                  </span>
                )}
                <MessageBubble
                  side={side}
                  pos={position(index, group.messages.length)}
                  fresh={freshIds.has(message.id) && !isStreaming}
                  streaming={isStreaming}
                >
                  {renderMessage(message)}
                </MessageBubble>
              </div>
            </div>
            {isLast && (
              <div className="message-meta">
                <time dateTime={message.createdAt}>{relativeTime(message.createdAt)}</time>
                {side === "out" && (
                  <DeliveryLine
                    state={deliveryState}
                    startedAt={run.id === last.runId ? run.startedAt : last.createdAt}
                    receipt={receipt}
                    cause={run.stopReason}
                    recoveryAction={recoverable ? (
                      <button type="button" className="delivery-recovery" onClick={onResume}>
                        Resume recovery
                      </button>
                    ) : undefined}
                  />
                )}
              </div>
            )}
          </Fragment>
        );
      })}
    </article>
  );
}

export const MessageGroup = memo(MessageGroupComponent);
