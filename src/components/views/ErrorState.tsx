import type { ReactNode } from "react";

type ErrorStateProps = {
  cause: string;
  action: ReactNode;
};

export function ErrorState({ cause, action }: ErrorStateProps) {
  return (
    <section className="error-state" role="alert">
      <p><strong>Cause:</strong> {cause}</p>
      <div className="error-state-action">{action}</div>
    </section>
  );
}
