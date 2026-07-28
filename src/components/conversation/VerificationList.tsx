import type { VerificationResult } from "../../model";

export function VerificationList({ verification }: { verification: VerificationResult[] }) {
  if (!verification.length) return null;
  return (
    <div className="verification-list">
      {verification.map((result) => (
        <div className={`verification-row verification-${result.status}`} key={result.label}>
          <code>{result.label}</code>
          <span className="verification-chip">{result.status === "passed" ? "Passed" : result.status === "failed" ? "Failed" : "Not run"}</span>
        </div>
      ))}
    </div>
  );
}
