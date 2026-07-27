import { forwardRef, type ButtonHTMLAttributes } from "react";

export type IconButtonProps = Omit<ButtonHTMLAttributes<HTMLButtonElement>, "aria-label"> & {
  "aria-label": string;
  size?: "sm" | "md";
};

export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(function IconButton(
  { className, size = "md", type = "button", ...props },
  ref,
) {
  return <button {...props} ref={ref} type={type} className={["primitive-icon-button", `primitive-icon-button--${size}`, className].filter(Boolean).join(" ")} />;
});
