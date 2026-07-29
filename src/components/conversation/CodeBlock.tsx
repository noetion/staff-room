import { useEffect, useState } from "react";

/** How long the confirmation stays up before the button returns to "Copy". */
const copiedResetMs = 1600;

export function CodeBlock({ code, language }: { code: string; language: string }) {
  const [copied, setCopied] = useState(false);
  const [failed, setFailed] = useState(false);

  // The reset used to hang off an animationend event. Reduced-motion users have
  // animations disabled globally, so that event never fired and the button was
  // stuck reading "Copied" for the rest of the session.
  useEffect(() => {
    if (!copied) return;
    const timeout = window.setTimeout(() => setCopied(false), copiedResetMs);
    return () => window.clearTimeout(timeout);
  }, [copied]);

  useEffect(() => {
    if (!failed) return;
    const timeout = window.setTimeout(() => setFailed(false), copiedResetMs);
    return () => window.clearTimeout(timeout);
  }, [failed]);

  async function copyCode() {
    try {
      await navigator.clipboard.writeText(code);
      setFailed(false);
      setCopied(true);
    } catch {
      setCopied(false);
      setFailed(true);
    }
  }

  const label = failed ? "Copy failed" : copied ? "Copied" : "Copy";

  return (
    <pre className="code-block">
      <div className="code-toolbar">
        <span>{language}</span>
        <button type="button" className="code-copy" onClick={() => void copyCode()} aria-label={`Copy ${language} code`}>
          <span aria-hidden="true">⧉</span>
          <span>{label}</span>
        </button>
      </div>
      <code>{code}</code>
      <span className="sr-only" role="status" aria-live="polite">
        {failed ? "Copying to the clipboard failed." : copied ? "Code copied to clipboard" : ""}
      </span>
    </pre>
  );
}
