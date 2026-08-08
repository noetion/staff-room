import type { CSSProperties } from "react";

type WordmarkCorner = "bottom-left" | "bottom-right";

type WordmarkStyle = CSSProperties & {
  "--wordmark-size": CSSProperties["fontSize"];
};

export type WordmarkProps = {
  corner: WordmarkCorner;
  size: CSSProperties["fontSize"];
};

export function Wordmark({ corner, size }: WordmarkProps) {
  return (
    <div
      className={`wordmark wordmark--${corner}`}
      style={{ "--wordmark-size": size } as WordmarkStyle}
      aria-hidden="true"
    >
      THE STAFF ROOM
    </div>
  );
}
