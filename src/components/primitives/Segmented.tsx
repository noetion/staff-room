import { forwardRef, useId, type ForwardedRef, type HTMLAttributes, type KeyboardEvent, type ReactElement } from "react";

export type SegmentedOption<T extends string> = { value: T; label: string; disabled?: boolean };
export type SegmentedProps<T extends string> = Omit<HTMLAttributes<HTMLDivElement>, "onChange"> & {
  value: T;
  onChange: (value: T) => void;
  options: readonly SegmentedOption<T>[];
};

function SegmentedInner<T extends string>({ value, onChange, options, className, ...props }: SegmentedProps<T>, ref: ForwardedRef<HTMLDivElement>) {
  const labelId = useId();
  const selectedIndex = Math.max(0, options.findIndex((option) => option.value === value));
  const onKeyDown = (event: KeyboardEvent<HTMLButtonElement>, index: number) => {
    if (!(["ArrowRight", "ArrowLeft", "ArrowDown", "ArrowUp"] as string[]).includes(event.key)) return;
    event.preventDefault();
    const direction = event.key === "ArrowRight" || event.key === "ArrowDown" ? 1 : -1;
    let nextIndex = index;
    do { nextIndex = (nextIndex + direction + options.length) % options.length; } while (options[nextIndex]?.disabled && nextIndex !== index);
    const next = options[nextIndex];
    if (next && !next.disabled) {
      onChange(next.value);
      document.getElementById(`${labelId}-${nextIndex}`)?.focus();
    }
  };

  return (
    <div {...props} ref={ref} role="group" className={["primitive-segmented", className].filter(Boolean).join(" ")}>
      <span className="primitive-segmented__indicator" style={{ transform: `translateX(${selectedIndex * 100}%)`, width: `${100 / options.length}%` }} aria-hidden="true" />
      {options.map((option, index) => (
        <button key={option.value} id={`${labelId}-${index}`} type="button" className="primitive-segmented__option" aria-pressed={option.value === value} disabled={option.disabled} onClick={() => onChange(option.value)} onKeyDown={(event) => onKeyDown(event, index)}>
          {option.label}
        </button>
      ))}
    </div>
  );
}

export const Segmented = forwardRef<HTMLDivElement, SegmentedProps<string>>(SegmentedInner) as <T extends string>(props: SegmentedProps<T> & { ref?: ForwardedRef<HTMLDivElement> }) => ReactElement;
