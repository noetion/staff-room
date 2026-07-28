import type { VerificationResult } from "../../model";
import { VerificationList } from "./VerificationList";

export function EvidenceCard({ changedFiles, label, verification }: { changedFiles: string[]; label: string; verification: VerificationResult[] }) {
  const visibleFiles = changedFiles.slice(0, 8);
  const remainingFiles = changedFiles.slice(8);

  return (
    <section className="evidence-card" aria-label={`Evidence for ${label}`}>
      <header><span className="evidence-chip">Evidence</span><span>{label}</span></header>
      <div className="evidence-files">
        {visibleFiles.map((file) => <div className="evidence-file" key={file}><span aria-hidden="true">+</span><code title={file}>{file}</code></div>)}
        {remainingFiles.length > 0 && (
          <details className="evidence-more">
            <summary>Show {remainingFiles.length} more files</summary>
            {remainingFiles.map((file) => <div className="evidence-file" key={file}><span aria-hidden="true">+</span><code title={file}>{file}</code></div>)}
          </details>
        )}
      </div>
      <VerificationList verification={verification} />
    </section>
  );
}
