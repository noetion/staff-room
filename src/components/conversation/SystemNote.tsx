import type { ReactNode } from "react";

export interface SystemNoteProps {
  emphasized?: boolean;
  children: ReactNode;
}

export function SystemNote({ emphasized = false, children }: SystemNoteProps) {
  return (
    <div className="system-note" data-emphasized={emphasized || undefined}>
      {children}
    </div>
  );
}
