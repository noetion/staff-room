import { createElement, forwardRef, type ElementType, type HTMLAttributes } from "react";

type SurfaceVariant = "glass" | "glass-clear" | "raised" | "sunken" | "plain";
type SurfaceRadius = "sm" | "md" | "lg" | "xl" | "capsule";

export type SurfaceProps = HTMLAttributes<HTMLElement> & {
  variant?: SurfaceVariant;
  refract?: boolean;
  as?: ElementType;
  radius?: SurfaceRadius;
};

export const Surface = forwardRef<HTMLElement, SurfaceProps>(function Surface(
  { as: Component = "div", className, variant = "plain", refract = false, radius = "md", ...props },
  ref,
) {
  const classes = [
    "primitive-surface",
    `primitive-surface--${variant}`,
    `primitive-surface--radius-${radius}`,
    variant === "glass" || variant === "glass-clear" ? "glass" : "",
    variant === "glass-clear" ? "glass--clear" : "",
    refract && variant.startsWith("glass") ? "glass--refract" : "",
    className,
  ]
    .filter(Boolean)
    .join(" ");

  return createElement(Component, { ...props, className: classes, ref: ref as never });
});
