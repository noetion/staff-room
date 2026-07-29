import {
  forwardRef,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { groupMessages, type MessageGroup as Group } from "../../lib/grouping";
import { agentNames, type AgentKind, type ExecutionReceipt, type RoomMessage, type Run } from "../../model";
import { calendarDay, initials } from "../../lib/format";
import { Monogram } from "../primitives";
import { DayDivider } from "./DayDivider";
import { MessageGroup } from "./MessageGroup";
import { TypingIndicator } from "./TypingIndicator";
import "./conversation.css";

export interface ConversationStreamingState {
  active: boolean;
  participant?: AgentKind;
  runId?: string;
}

export interface ConversationProps {
  messages: RoomMessage[];
  streaming: ConversationStreamingState;
  hasMore: boolean;
  loadingOlder: boolean;
  onLoadOlder: () => void;
  query: string;
  run: Run;
  receipts: ExecutionReceipt[];
  renderMessage: (message: RoomMessage) => ReactNode;
  onResume: () => void;
  children?: ReactNode;
}

function splitAtDayBoundaries(groups: Group[]) {
  const split: Group[] = [];
  for (const group of groups) {
    let current: Group | undefined;
    for (const message of group.messages) {
      const previousMessage = current?.messages.at(-1);
      if (
        current &&
        previousMessage &&
        calendarDay(previousMessage.createdAt) === calendarDay(message.createdAt)
      ) {
        current.messages.push(message);
      } else {
        current = { sender: group.sender, kind: group.kind, messages: [message] };
        split.push(current);
      }
    }
  }
  return split;
}

export const Conversation = forwardRef<HTMLDivElement, ConversationProps>(
  function Conversation(
    {
      messages,
      streaming,
      hasMore,
      loadingOlder,
      onLoadOlder,
      query,
      run,
      receipts,
      renderMessage,
      onResume,
      children,
    },
    ref,
  ) {
    const normalizedQuery = query.trim().toLocaleLowerCase();
    const visibleMessages = useMemo(() => {
      if (!normalizedQuery) return messages;
      return messages.filter((message) =>
        [message.body, message.reason, message.sender, message.kind]
          .filter(Boolean)
          .some((value) => value?.toLocaleLowerCase().includes(normalizedQuery)),
      );
    }, [messages, normalizedQuery]);
    const groups = useMemo(
      () => splitAtDayBoundaries(groupMessages(visibleMessages)),
      [visibleMessages],
    );
    const previousIdsRef = useRef(messages.map((message) => message.id));
    const [freshIds, setFreshIds] = useState<ReadonlySet<string>>(new Set());

    useLayoutEffect(() => {
      const previousIds = previousIdsRef.current;
      const currentIds = messages.map((message) => message.id);
      const appended =
        !normalizedQuery &&
        currentIds.length > previousIds.length &&
        previousIds.every((id, index) => currentIds[index] === id);
      const nextFresh = appended
        ? new Set(currentIds.slice(previousIds.length))
        : new Set<string>();
      previousIdsRef.current = currentIds;
      setFreshIds(nextFresh);

      if (!nextFresh.size) return;
      const freshDuration = Number.parseFloat(
        window.getComputedStyle(document.documentElement).getPropertyValue("--dur-2"),
      );
      const timeout = window.setTimeout(() => setFreshIds(new Set()), freshDuration);
      return () => window.clearTimeout(timeout);
    }, [messages, normalizedQuery]);

    const hasStreamingMessage = Boolean(
      streaming.runId &&
      visibleMessages.some(
        (message) =>
          message.runId === streaming.runId &&
          message.kind === "agent" &&
          message.body.trim(),
      ),
    );

    return (
      <div className="conversation scroll-edge" ref={ref} role="feed" aria-label="Room timeline">
        <div className="conversation-column">
          {hasMore && !normalizedQuery && (
            <button
              type="button"
              className="load-older-messages"
              onClick={onLoadOlder}
              disabled={loadingOlder}
            >
              {loadingOlder ? "Loading earlier messages…" : "Load earlier messages"}
            </button>
          )}
          {groups.map((group, index) => {
            const first = group.messages[0];
            const previous = groups[index - 1]?.messages.at(-1);
            const showDay = !previous ||
              calendarDay(previous.createdAt) !== calendarDay(first.createdAt);
            return (
              <div className="conversation-segment" key={group.messages.map((message) => message.id).join(":")}>
                {showDay && <DayDivider date={first.createdAt} />}
                <MessageGroup
                  group={group}
                  freshIds={freshIds}
                  streamingRunId={streaming.active ? streaming.runId : undefined}
                  run={run}
                  receipts={receipts}
                  renderMessage={renderMessage}
                  onResume={onResume}
                />
              </div>
            );
          })}
          {streaming.active && streaming.participant && !hasStreamingMessage && !normalizedQuery && (
            <article
              className="message-group typing-group"
              aria-labelledby="typing-sender"
              data-side="in"
            >
              <div className="message-row">
                <span className="message-avatar-slot">
                  <Monogram
                    label={initials(streaming.participant)}
                    className={`conversation-avatar conversation-avatar-${streaming.participant}`}
                    aria-hidden="true"
                  >
                    {initials(streaming.participant)}
                  </Monogram>
                </span>
                <div className="message-stack">
                  <span className="message-sender" id="typing-sender">
                    {agentNames[streaming.participant]}
                  </span>
                  <TypingIndicator
                    senderName={agentNames[streaming.participant]}
                  />
                </div>
              </div>
            </article>
          )}
          {normalizedQuery && !visibleMessages.length && (
            <p className="empty-state">No room activity matches this search.</p>
          )}
          {children}
        </div>
      </div>
    );
  },
);
