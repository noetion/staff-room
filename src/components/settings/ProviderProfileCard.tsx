import { RefreshCw } from "lucide-react";
import { useEffect, useState } from "react";
import type { AgentKind, Participant, ProviderProfile } from "../../model";
import { connectionLabel, capabilityChips } from "../../lib/participants";
import { connectionTestDraft, providerModelOptions, type ProviderDraft } from "../../lib/provider-profiles";
import { initials } from "../../lib/format";
import { Monogram } from "../primitives";

type ProviderRoute = "chat" | "build" | "review";

export function ProviderProfileCard({
  participant,
  profiles,
  saving,
  onSave,
  testing,
  onTest,
  models,
  hasAuthoritativeCatalog,
  discoveringModels,
  modelDiscoveryDetail,
  onRefreshModels,
}: {
  participant: Participant;
  profiles: ProviderProfile[];
  saving: boolean;
  onSave: (profiles: ProviderProfile[]) => Promise<void>;
  testing: boolean;
  onTest: (kind: AgentKind, draft: ProviderDraft) => void;
  models: string[];
  hasAuthoritativeCatalog: boolean;
  discoveringModels: boolean;
  modelDiscoveryDetail?: string;
  onRefreshModels: (kind: AgentKind) => void;
}) {
  const routes: ProviderRoute[] = ["chat", "build", "review"];
  const [drafts, setDrafts] = useState<Record<ProviderRoute, ProviderDraft>>({
    chat: { model: "", effort: "" },
    build: { model: "", effort: "" },
    review: { model: "", effort: "" },
  });

  useEffect(() => {
    setDrafts(Object.fromEntries(routes.map((route) => {
      const profile = profiles.find((value) => value.route === route);
      return [route, { model: profile?.model ?? "", effort: profile?.effort ?? "" }];
    })) as typeof drafts);
  }, [profiles]);

  const modelOptions = providerModelOptions(models, profiles, hasAuthoritativeCatalog);

  return (
    <article className="provider-profile-card">
      <header className="provider-profile-header">
        <Monogram label={initials(participant.kind)} aria-hidden="true">{initials(participant.kind)}</Monogram>
        <span className="provider-profile-identity">
          <strong>{participant.name}</strong>
          <small>{participant.installed ? participant.version ?? "Installed" : "Not installed"}</small>
        </span>
        <span className={`status-chip connection-${participant.connectionStatus}`}>{connectionLabel(participant.connectionStatus)}</span>
      </header>
      {/* One datalist per card. Rendering it inside the route loop emitted the
          same element id three times, which is invalid and leaves two of the
          three inputs pointing at a duplicate the browser ignores. */}
      <datalist id={`models-${participant.kind}`}>{modelOptions.map((option) => <option key={option.value} value={option.value} label={option.label} />)}</datalist>
      <div className="route-profile-list">
        {routes.map((route) => <section className="route-profile-row" key={route}>
          <strong>{route === "chat" ? "Chat" : route === "build" ? "Build" : "Review"}</strong>
          <label htmlFor={`model-${participant.kind}-${route}`}>
            <span>Model</span>
            <input id={`model-${participant.kind}-${route}`} list={`models-${participant.kind}`} value={drafts[route].model} onChange={(event) => setDrafts((current) => ({ ...current, [route]: { ...current[route], model: event.target.value } }))} disabled={!participant.installed || saving} placeholder="Default" aria-invalid={hasAuthoritativeCatalog && Boolean(drafts[route].model.trim()) && !models.includes(drafts[route].model.trim())} title={hasAuthoritativeCatalog && Boolean(drafts[route].model.trim()) && !models.includes(drafts[route].model.trim()) ? "Choose an exact model from the refreshed catalogue before saving." : undefined} />
          </label>
          <label htmlFor={`effort-${participant.kind}-${route}`}>
            <span>Effort</span>
            <select id={`effort-${participant.kind}-${route}`} value={drafts[route].effort} onChange={(event) => setDrafts((current) => ({ ...current, [route]: { ...current[route], effort: event.target.value } }))} disabled={!participant.installed || saving || !participant.supportsEffort || participant.kind === "cursor"} title={participant.kind === "cursor" ? "Cursor model identifiers already encode effort." : undefined}>
              <option value="">Default</option>
              {participant.effortOptions.filter((option) => participant.kind !== "codex" || drafts[route].model === "gpt-5.6-sol" || !["max", "ultra"].includes(option)).map((option) => <option key={option} value={option}>{option[0].toUpperCase() + option.slice(1)}</option>)}
            </select>
          </label>
        </section>)}
      </div>
      {participant.kind === "cursor" && <small className="model-discovery-note">Cursor model identifiers already include effort, thinking, and speed. Refresh Models and select an exact identifier.</small>}
      <small className="model-discovery-note" title={modelDiscoveryDetail ?? participant.modelDiscoveryNote}>{modelDiscoveryDetail ?? participant.modelDiscoveryNote}</small>
      <details className="provider-details"><summary>Runtime capability</summary><p className="connection-detail" role={participant.connectionStatus === "connected" ? undefined : "status"}>{participant.connectionDetail}</p>{capabilityChips(participant).length > 0 && <div className="capability-chips" aria-label={`${participant.name} capability limits`}>{capabilityChips(participant).map((chip) => <span key={chip}>{chip}</span>)}</div>}<p>{participant.capabilities.autonomyNote}</p></details>
      <div className="profile-actions">
        <button type="button" className="secondary-button" aria-label={`Refresh models for ${participant.name}`} title={`Refresh models for ${participant.name}`} disabled={!participant.installed || discoveringModels} onClick={() => onRefreshModels(participant.kind)}><RefreshCw size={14} className={discoveringModels ? "spinning" : undefined} />{discoveringModels ? "Refreshing" : "Refresh"}</button>
        <button type="button" className="secondary-button" disabled={!participant.installed || saving} onClick={() => onSave(routes.map((route) => ({ participantKind: participant.kind, route, model: drafts[route].model.trim() || undefined, effort: drafts[route].effort || undefined })))}>{saving ? "Saving" : "Save"}</button>
        <button type="button" className="secondary-button" aria-label={`Test ${participant.name} connection`} disabled={!participant.installed || testing} onClick={() => onTest(participant.kind, connectionTestDraft(drafts.chat))}>{testing ? "Testing" : "Test"}</button>
      </div>
    </article>
  );
}
