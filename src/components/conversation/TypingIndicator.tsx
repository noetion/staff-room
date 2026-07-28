import { MessageBubble } from "./MessageBubble";

export interface TypingIndicatorProps {
  senderName: string;
}

export function TypingIndicator({ senderName }: TypingIndicatorProps) {
  return (
    <MessageBubble side="in" pos="last" streaming>
      <span className="typing-indicator" role="status" aria-label={`${senderName} is typing`}>
        <span />
        <span />
        <span />
      </span>
    </MessageBubble>
  );
}
