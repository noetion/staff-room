import { useState } from "react";

export function CodeBlock({ code, language }: { code: string; language: string }) {
  const [copied, setCopied] = useState(false);

  async function copyCode() {
    try {
      await navigator.clipboard.writeText(code);
      setCopied(true);
    } catch {
      setCopied(false);
    }
  }

  return (
    <pre className="code-block">
      <div className="code-toolbar">
        <span>{language}</span>
        <button type="button" className="code-copy" onClick={() => void copyCode()} aria-label={`Copy ${language} code`}>
          <span aria-hidden="true">⧉</span>
          <span>{copied ? "Copied" : "Copy"}</span>
        </button>
      </div>
      <code>{code}</code>
      {copied && <span className="sr-only code-copy-announcement" aria-live="polite" onAnimationEnd={() => setCopied(false)}>Code copied to clipboard</span>}
    </pre>
  );
}
