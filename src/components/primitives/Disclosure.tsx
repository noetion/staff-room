import { forwardRef, type DetailsHTMLAttributes } from "react";

export type DisclosureProps = DetailsHTMLAttributes<HTMLDetailsElement> & { defaultOpen?: boolean };

export const Disclosure = forwardRef<HTMLDetailsElement, DisclosureProps>(function Disclosure(
  { className, children, defaultOpen, ...props },
  ref,
) {
  return <details {...props} ref={ref} open={props.open ?? defaultOpen} className={["primitive-disclosure", className].filter(Boolean).join(" ")}>{children}</details>;
});
