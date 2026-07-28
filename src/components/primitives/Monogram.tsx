import { forwardRef, type CSSProperties, type HTMLAttributes } from "react";

export type MonogramProps = HTMLAttributes<HTMLSpanElement> & {
  label: string;
  size?: "sm" | "md";
  ring?: string;
  tone?: "neutral" | "working" | "good" | "warn" | "bad";
  shape?: "square" | "circle";
};

export const Monogram = forwardRef<HTMLSpanElement, MonogramProps>(function Monogram(
  { className, label, size = "md", ring, tone = "neutral", shape = "square", style, ...props },
  ref,
) {
  return <span {...props} ref={ref} style={{ ...style, "--monogram-ring": ring ? `var(${ring})` : undefined } as CSSProperties} className={["primitive-monogram", `primitive-monogram--${size}`, `primitive-monogram--${tone}`, `primitive-monogram--${shape}`, className].filter(Boolean).join(" ")}>{label}</span>;
});
