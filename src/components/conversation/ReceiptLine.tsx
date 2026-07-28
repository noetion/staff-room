export function ReceiptLine({ model, latency, usage, details }: { model?: string; latency?: string; usage?: string; details?: Array<[string, string | undefined]> }) {
  const summary = `${model ?? "Model not reported"} · ${latency ?? "Latency not reported"} · ${usage ?? "Usage not reported"}`;
  const fullDetails = details ?? [["Model", model], ["Latency", latency], ["Usage", usage]];

  return (
    <details className="receipt-line">
      <summary>{summary}</summary>
      <dl>
        {fullDetails.map(([name, value]) => <div key={name}><dt>{name}</dt><dd>{value ?? "not reported"}</dd></div>)}
      </dl>
    </details>
  );
}
