import { forwardRef, useEffect, useId, useRef, type HTMLAttributes, type ReactNode } from "react";

export type SheetProps = Omit<HTMLAttributes<HTMLElement>, "title"> & {
  open: boolean;
  onClose: () => void;
  title: ReactNode;
  labelledBy?: string;
};

const focusableSelector = 'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

export const Sheet = forwardRef<HTMLElement, SheetProps>(function Sheet(
  { open, onClose, title, labelledBy, className, children, ...props },
  forwardedRef,
) {
  const internalRef = useRef<HTMLElement>(null);
  const previousFocus = useRef<HTMLElement | null>(null);
  const generatedTitleId = useId();
  const titleId = labelledBy ?? generatedTitleId;
  const setRef = (element: HTMLElement | null) => { internalRef.current = element; if (typeof forwardedRef === "function") forwardedRef(element); else if (forwardedRef) forwardedRef.current = element; };

  useEffect(() => {
    if (!open) return;
    previousFocus.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const first = internalRef.current?.querySelector<HTMLElement>(focusableSelector);
    first?.focus();
    return () => previousFocus.current?.focus();
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") { onClose(); return; }
      if (event.key !== "Tab") return;
      const nodes = Array.from(internalRef.current?.querySelectorAll<HTMLElement>(focusableSelector) ?? []);
      if (!nodes.length) { event.preventDefault(); return; }
      const current = document.activeElement;
      const index = nodes.indexOf(current as HTMLElement);
      if (event.shiftKey && (index <= 0 || current === internalRef.current)) { event.preventDefault(); nodes[nodes.length - 1]?.focus(); }
      if (!event.shiftKey && index === nodes.length - 1) { event.preventDefault(); nodes[0]?.focus(); }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [open, onClose]);

  return (
    <div className={["primitive-sheet-layer", open && "primitive-sheet-layer--open"].filter(Boolean).join(" ")} aria-hidden={!open}>
      <div className="primitive-sheet__scrim" onClick={onClose} aria-hidden="true" />
      <aside {...props} ref={setRef} className={["primitive-sheet", "glass", className].filter(Boolean).join(" ")} role="dialog" aria-modal="true" aria-labelledby={labelledBy ?? titleId} tabIndex={-1}>
        <header className="primitive-sheet__header"><h2 id={titleId} className="primitive-sheet__title">{title}</h2><button type="button" className="primitive-sheet__close" onClick={onClose} aria-label="Close">×</button></header>
        <div className="primitive-sheet__content">{children}</div>
      </aside>
    </div>
  );
});
