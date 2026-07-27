import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from "react";

export type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: "primary" | "secondary" | "ghost" | "danger";
  size?: "sm" | "md";
  icon?: ReactNode;
  iconEnd?: ReactNode;
  busy?: boolean;
};

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { className, variant = "primary", size = "md", icon, iconEnd, busy = false, children, ...props },
  ref,
) {
  const classes = ["primitive-button", `primitive-button--${variant}`, `primitive-button--${size}`, className]
    .filter(Boolean)
    .join(" ");

  return (
    <button {...props} ref={ref} className={classes} aria-busy={busy || undefined}>
      <span className="primitive-button__icon">{busy ? <span className="primitive-spinner" aria-hidden="true" /> : icon}</span>
      <span className="primitive-button__label">{children}</span>
      {iconEnd && <span className="primitive-button__icon">{iconEnd}</span>}
    </button>
  );
});
