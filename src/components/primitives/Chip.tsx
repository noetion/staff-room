import { forwardRef, type HTMLAttributes } from "react";

export type ChipProps = HTMLAttributes<HTMLSpanElement> & {
  tone?: "neutral" | "working" | "good" | "warn" | "bad";
  dot?: boolean;
  mono?: boolean;
  size?: "sm" | "md";
};

export const Chip = forwardRef<HTMLSpanElement, ChipProps>(function Chip(
  { className, tone = "neutral", dot = false, mono = true, size = "md", children, ...props },
  ref,
) {
  return (
    <span {...props} ref={ref} className={["primitive-chip", `primitive-chip--${tone}`, `primitive-chip--${size}`, mono && "primitive-chip--mono", className].filter(Boolean).join(" ")}>
      {dot && <span className="primitive-chip__dot" aria-hidden="true" />}
      {children}
    </span>
  );
});
