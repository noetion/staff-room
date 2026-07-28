import type { ReactNode } from "react";
import type { RoomMessage } from "../../model";
import { agentNames } from "../../model";
import { relativeTime } from "../../lib/format";
import { EmptyState } from "./EmptyState";

type ActivityViewProps = {
  messages: RoomMessage[];
  query: string;
  renderMessage: (message: RoomMessage) => ReactNode;
};

function HighlightedText({ value, query }: { value: string; query: string }) {
  if (!query.trim()) return <>{value}</>;

  const parts = value.split(new RegExp(`(${query.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")})`, "gi"));
  return <>{parts.map((part, index) => part.toLowerCase() === query.toLowerCase()
    ? <mark key={`${part}-${index}`}>{part}</mark>
    : part)}</>;
}

export function ActivityView({ messages, query, renderMessage }: ActivityViewProps) {
  return (
    <section className="utility-screen activity-view" aria-labelledby="activity-title">
      <header className="utility-header activity-header">
        <div>
          <h1 id="activity-title">Activity</h1>
          <p>{query ? `Results for “${query}”` : "A durable record of objectives, evidence, and recovery states."}</p>
        </div>
        <code className="activity-result-count">{messages.length} {messages.length === 1 ? "result" : "results"}</code>
      </header>
      <div className="utility-timeline activity-list" role="feed" aria-label="Room activity">
        {messages.length ? messages.map((message) => {
          const senderName = message.sender === "human"
            ? "You"
            : message.sender === "system"
              ? "Agent Room"
              : agentNames[message.sender];

          return (
            <article className="activity-message" key={message.id} aria-label={`${senderName}, ${relativeTime(message.createdAt)}`}>
              <header>
                <strong>{senderName}</strong>
                <span>{relativeTime(message.createdAt)}</span>
                <span>{message.kind}</span>
              </header>
              {query ? <div className="activity-message-body"><HighlightedText value={message.body} query={query} /></div> : renderMessage(message)}
            </article>
          );
        }) : (
          <EmptyState title="No room activity matches this search." body={query ? "Try a different search term." : "Activity will appear as work happens in this room."} />
        )}
      </div>
    </section>
  );
}
