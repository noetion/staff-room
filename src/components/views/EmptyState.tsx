import type { ReactNode } from "react";

type Suggestion = {
  label: string;
  value: string;
};

type EmptyStateProps = {
  wordmark?: boolean;
  title: string;
  body?: ReactNode;
  suggestions?: Suggestion[];
  onSuggestion?: (value: string) => void;
};

export function EmptyState({ wordmark = false, title, body, suggestions, onSuggestion }: EmptyStateProps) {
  return (
    <section className="empty-state">
      {wordmark && <span className="ghost-wordmark" aria-hidden="true">THE STAFF ROOM</span>}
      <div className="empty-state-content">
        <h2>{title}</h2>
        {body && <p>{body}</p>}
        {suggestions?.length ? (
          <div className="empty-state-suggestions" aria-label="Suggested prompts">
            {suggestions.slice(0, 3).map((suggestion) => (
              <button key={suggestion.value} type="button" className="empty-state-chip" onClick={() => onSuggestion?.(suggestion.value)}>
                {suggestion.label}
              </button>
            ))}
          </div>
        ) : null}
      </div>
    </section>
  );
}
