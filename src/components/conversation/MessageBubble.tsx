import type { ReactNode } from "react";

export type MessageSide = "in" | "out";
export type MessagePosition = "first" | "middle" | "last" | "only";

export interface MessageBubbleProps {
  side: MessageSide;
  pos: MessagePosition;
  fresh?: boolean;
  streaming?: boolean;
  children: ReactNode;
}

export function MessageBubble({
  side,
  pos,
  fresh = false,
  streaming = false,
  children,
}: MessageBubbleProps) {
  return (
    <div
      className="message-bubble"
      data-side={side}
      data-pos={pos}
      data-fresh={fresh || undefined}
      data-streaming={streaming || undefined}
      aria-live={streaming ? "polite" : undefined}
    >
      {children}
    </div>
  );
}
