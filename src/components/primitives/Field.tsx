import { forwardRef, useId, type InputHTMLAttributes, type Ref, type SelectHTMLAttributes, type TextareaHTMLAttributes } from "react";

type FieldBase = { label: string; hint?: string; error?: string; mono?: boolean; className?: string };
export type InputFieldProps = FieldBase & InputHTMLAttributes<HTMLInputElement> & { as?: "input" };
export type TextareaFieldProps = FieldBase & TextareaHTMLAttributes<HTMLTextAreaElement> & { as: "textarea" };
export type SelectFieldProps = FieldBase & SelectHTMLAttributes<HTMLSelectElement> & { as: "select" };
export type FieldProps = InputFieldProps | TextareaFieldProps | SelectFieldProps;

export const Field = forwardRef<HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement, FieldProps>(function Field(
  { as = "input", label, hint, error, mono = false, className, id: suppliedId, children, ...props },
  ref,
) {
  const generatedId = useId();
  const id = suppliedId ?? generatedId;
  const hintId = hint ? `${id}-hint` : undefined;
  const errorId = error ? `${id}-error` : undefined;
  const describedBy = [hintId, errorId].filter(Boolean).join(" ") || undefined;
  const controlProps = { ...props, id, "aria-describedby": describedBy, "aria-invalid": error ? true : undefined, className: ["primitive-field__control", mono && "primitive-field__control--mono"].filter(Boolean).join(" ") };

  return (
    <label className={["primitive-field", className].filter(Boolean).join(" ")} htmlFor={id}>
      <span className="primitive-field__label">{label}</span>
      {as === "textarea" ? <textarea {...(controlProps as TextareaHTMLAttributes<HTMLTextAreaElement>)} ref={ref as Ref<HTMLTextAreaElement>}>{children}</textarea> : as === "select" ? <select {...(controlProps as SelectHTMLAttributes<HTMLSelectElement>)} ref={ref as Ref<HTMLSelectElement>}>{children}</select> : <input {...(controlProps as InputHTMLAttributes<HTMLInputElement>)} ref={ref as Ref<HTMLInputElement>} />}
      {hint && <span id={hintId} className="primitive-field__hint">{hint}</span>}
      {error && <span id={errorId} className="primitive-field__error">{error}</span>}
    </label>
  );
});
