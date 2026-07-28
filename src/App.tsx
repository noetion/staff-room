import {
  Bot,
  Braces,
  Check,
  ChevronRight,
  CircleAlert,
  GitBranch,
  HardDrive,
  History,
  RefreshCw,
  ShieldCheck,
  Sparkles,
  X,
} from "lucide-react";
import {
  FormEvent,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type {
  AgentKind,
  ExecutionReceipt,
  NativeEnvironment,
  Participant,
  Project,
  ProjectSettings,
  ProviderProfile,
  QuickEditResult,
  RoomMessage,
  Run,
  RunState,
  VerificationConfig,
} from "./model";
import { AttentionCard, EvidenceCard, MarkdownBody, VerificationList } from "./components/conversation";
import { agentNames } from "./model";
import {
  connectionTestDraft,
  providerModelOptions,
  type ProviderDraft,
} from "./lib/provider-profiles";
import {
  contextLabel,
  elapsedTime,
  initials,
  latencyLabel,
  millisecondsLabel,
  relativeTime,
  usageLabel,
} from "./lib/format";
import {
  autonomyLabel,
  canUseChat,
  capabilityChips,
  chatAgentFor,
  connectionLabel,
  isRunnableParticipant,
} from "./lib/participants";
import { asRunState, hydrateRun } from "./lib/runs";
import {
  getEnvironment,
  discoverProviderModels,
  isNativeApp,
  loadProviderProfiles,
  loadProjectSettings,
  loadVerificationConfig,
  loadRoom,
  onRunEvent,
  projectActive,
  projectAttach,
  projectList,
  projectPick,
  projectSelect,
  quickEditApply,
  abandonRun,
  quickEditDiscard,
  quickEditStart,
  saveProviderProfile,
  saveProjectSettings,
  saveVerificationConfig,
  startRoomChat,
  startRoomRun,
  stopRun,
  testProviderConnection,
  type RunEvent,
} from "./native";
import { NavRail, TitleBar } from "./components/chrome";
import { StatusPill } from "./components/chrome/StatusPill";
import { Conversation } from "./components/conversation";
import { RunProgressCard } from "./components/conversation/RunProgressCard";
import { Monogram } from "./components/primitives";
import { Composer } from "./components/composer";

type InspectorTab = "Repository" | "Participants" | "Evidence" | "Memory";
type PrimaryView = "rooms" | "activity" | "settings";
type ProviderRoute = "chat" | "build" | "review";

const detachedProject: Project = {
  id: "",
  name: "No repository selected",
  goal: "",
  repositoryPath: "",
  branch: "",
};

const detachedEnvironment: NativeEnvironment = {
  native: false,
  attached: false,
  repositoryPath: "",
  branch: "",
  participants: [],
  contextBudgetBytes: 0,
};

const emptyRun: Run = {
  id: "",
  objective: "",
  state: "ready",
  route: [],
  reviewCount: 0,
  revisionCount: 0,
  startedAt: "",
};
type ComposerMode = "ask" | "quick-edit" | "ship";
type LiveActivityItem = { title: string; detail: string };

const activeStates: RunState[] = [
  "selecting",
  "working",
  "verifying",
  "reviewing",
  "revising",
  "promoting",
];

function ParticipantMark({ participant }: { participant: Participant }) {
  return (
    <Monogram label={participant.name} aria-hidden="true">
      {initials(participant.kind)}
    </Monogram>
  );
}

function TimelineMessageContent({ message }: { message: RoomMessage }) {
  const reason = message.reason;

  return (
    <>
      <MarkdownBody body={message.body} />
      {reason && (
        <div className="reason-line">
          <Braces size={14} />
          <span>{reason}</span>
        </div>
      )}
      {(message.changedFiles?.length || message.verification?.length) ? <EvidenceCard changedFiles={message.changedFiles ?? []} label="Run evidence" verification={message.verification ?? []} /> : null}
    </>
  );
}

function TimelineEntry({ message }: { message: RoomMessage }) {
  const isHuman = message.kind === "human";
  const senderName =
    message.sender === "human"
      ? "You"
      : message.sender === "system"
        ? "Agent Room"
        : agentNames[message.sender];

  return (
    <article className={`timeline-entry entry-${message.kind}`}>
      <div className="entry-rail">
        <span className="entry-dot">
          {isHuman ? (
            <Sparkles size={13} />
          ) : message.kind === "evidence" ? (
            <ShieldCheck size={13} />
          ) : (
            <Bot size={13} />
          )}
        </span>
        <span className="entry-line" />
      </div>
      <div className="entry-content">
        <header>
          <strong>{senderName}</strong>
          <span>{relativeTime(message.createdAt)}</span>
          <span className="entry-kind">{message.kind}</span>
        </header>
        <TimelineMessageContent message={message} />
      </div>
    </article>
  );
}

function AttachProjectView({
  projects,
  native,
  onAttach,
  onSelect,
}: {
  projects: Project[];
  native: boolean;
  onAttach: () => void;
  onSelect: (id: string) => void;
}) {
  return (
    <section className="utility-screen attach-project" aria-labelledby="attach-project-title">
      <header className="utility-header">
        <span className="eyebrow">Local workspace</span>
        <h1 id="attach-project-title">Choose a repository</h1>
        <p>Attach a Git repository to create an independently scoped Agent Room.</p>
        <button type="button" className="primary-button" onClick={onAttach} disabled={!native}>
          Choose a repository
        </button>
        {!native && <p className="empty-state">Repository attachment is available in the Tauri desktop app.</p>}
      </header>
      {projects.length > 0 && (
        <div className="recent-projects">
          <span className="eyebrow">Recent projects</span>
          {projects.map((recentProject) => (
            <button key={recentProject.id} type="button" className="project-switch" onClick={() => onSelect(recentProject.id)}>
              <span className="project-monogram small">{recentProject.name.slice(0, 2).toUpperCase()}</span>
              <span>
                <strong>{recentProject.name}</strong>
                <small>{recentProject.repositoryPath}</small>
              </span>
              <ChevronRight size={15} />
            </button>
          ))}
        </div>
      )}
    </section>
  );
}

function ActivityView({ messages, query }: { messages: RoomMessage[]; query: string }) {
  return (
    <section className="utility-screen" aria-labelledby="activity-title">
      <header className="utility-header">
        <span className="eyebrow">Room record</span>
        <h1 id="activity-title">Activity</h1>
        <p>{query ? `Results for “${query}”` : "A durable timeline of objectives, evidence, and recovery states."}</p>
      </header>
      <div className="utility-timeline" role="feed" aria-label="Room activity">
        {messages.length ? (
          messages.map((message) => <TimelineEntry key={message.id} message={message} />)
        ) : (
          <p className="empty-state">No room activity matches this search.</p>
        )}
      </div>
    </section>
  );
}

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
      return [route, {
        model: profile?.model ?? "",
        effort: profile?.effort ?? "",
      }];
    })) as typeof drafts);
  }, [profiles]);

  const modelOptions = providerModelOptions(models, profiles, hasAuthoritativeCatalog);

  return (
    <article className="participant-card provider-profile-card">
      <div className="participant-row">
        <ParticipantMark participant={participant} />
        <span className="participant-copy">
          <strong>{participant.name}</strong>
          <small>{participant.installed ? participant.version ?? "Installed" : "Not installed"}</small>
        </span>
        <span className={`status-chip connection-${participant.connectionStatus}`}>
          {connectionLabel(participant.connectionStatus)}
        </span>
      </div>
      <p className="connection-detail" role={participant.connectionStatus === "connected" ? undefined : "status"}>
        {participant.connectionDetail}
      </p>
      {capabilityChips(participant).length > 0 && (
        <div className="capability-chips" aria-label={`${participant.name} capability limits`}>
          {capabilityChips(participant).map((chip) => <span key={chip}>{chip}</span>)}
        </div>
      )}
      <div className="route-profile-list">
        {routes.map((route) => (
          <div className="route-profile-row" key={route}>
            <strong>{route === "chat" ? "Chat" : route === "build" ? "Builder" : "Reviewer"}</strong>
            <label htmlFor={`model-${participant.kind}-${route}`}>
              <span>Model</span>
              <input
                id={`model-${participant.kind}-${route}`}
                list={`models-${participant.kind}`}
                value={drafts[route].model}
                onChange={(event) => setDrafts((current) => ({
                  ...current,
                  [route]: { ...current[route], model: event.target.value },
                }))}
                disabled={!participant.installed || saving}
                placeholder="Default"
              />
              <datalist id={`models-${participant.kind}`}>
                {modelOptions.map((option) => (
                  <option key={option.value} value={option.value} label={option.label} />
                ))}
              </datalist>
            </label>
            <label htmlFor={`effort-${participant.kind}-${route}`}>
              <span>Effort</span>
              <select
                id={`effort-${participant.kind}-${route}`}
                value={drafts[route].effort}
                onChange={(event) => setDrafts((current) => ({
                  ...current,
                  [route]: { ...current[route], effort: event.target.value },
                }))}
                disabled={
                  !participant.installed
                  || saving
                  || !participant.supportsEffort
                  || participant.kind === "cursor"
                }
                title={
                  participant.kind === "cursor"
                    ? "Cursor model identifiers already encode effort."
                    : undefined
                }
              >
                <option value="">Default</option>
                {participant.effortOptions.filter((option) => (
                  participant.kind !== "codex"
                  || drafts[route].model === "gpt-5.6-sol"
                  || !["max", "ultra"].includes(option)
                )).map((option) => (
                  <option key={option} value={option}>
                    {option[0].toUpperCase() + option.slice(1)}
                  </option>
                ))}
              </select>
            </label>
          </div>
        ))}
      </div>
      {participant.kind === "cursor" && (
        <small className="model-discovery-note">
          Cursor model identifiers already include effort, thinking, and speed. Refresh Models and select an exact identifier.
        </small>
      )}
      <small className="model-discovery-note" title={modelDiscoveryDetail ?? participant.modelDiscoveryNote}>
        {modelDiscoveryDetail ?? participant.modelDiscoveryNote}
      </small>
      <details className="provider-details">
        <summary>Runtime capability</summary>
        <p>{participant.capabilities.autonomyNote}</p>
      </details>
      <div className="profile-actions">
        <button
          type="button"
          className="secondary-button"
          disabled={!participant.installed || discoveringModels}
          onClick={() => onRefreshModels(participant.kind)}
        >
          <RefreshCw size={14} className={discoveringModels ? "spinning" : undefined} />
          {discoveringModels ? "Refreshing" : "Models"}
        </button>
        <button
          type="button"
          className="secondary-button"
          disabled={!participant.installed || saving}
          onClick={() => onSave(routes.map((route) => ({
            participantKind: participant.kind,
            route,
            model: drafts[route].model.trim() || undefined,
            effort: drafts[route].effort || undefined,
          })))}
        >
          {saving ? "Saving" : "Save"}
        </button>
        <button
          type="button"
          className="secondary-button"
          aria-label={`Test ${participant.name} connection`}
          disabled={!participant.installed || testing}
          onClick={() => onTest(participant.kind, connectionTestDraft(drafts.chat))}
        >
          {testing ? "Testing" : "Test"}
        </button>
      </div>
    </article>
  );
}

function SettingsView({
  environment,
  profiles,
  projectSettings,
  verificationConfig,
  savingProjectSettings,
  savingKind,
  refreshing,
  native,
  error,
  onRefresh,
  onSaveProfile,
  testingKind,
  onTestConnection,
  modelCatalog,
  discoveringModelsKind,
  modelDiscoveryDetails,
  onRefreshModels,
  onAutonomousShipChange,
  onVerificationConfigChange,
  onSaveVerificationConfig,
}: {
  environment: NativeEnvironment;
  profiles: ProviderProfile[];
  projectSettings: ProjectSettings;
  verificationConfig: VerificationConfig;
  savingProjectSettings: boolean;
  savingKind?: AgentKind;
  refreshing: boolean;
  native: boolean;
  error: string;
  onRefresh: () => void;
  onSaveProfile: (profiles: ProviderProfile[]) => Promise<void>;
  testingKind?: AgentKind;
  onTestConnection: (kind: AgentKind, draft: ProviderDraft) => void;
  modelCatalog: Partial<Record<AgentKind, string[]>>;
  discoveringModelsKind?: AgentKind;
  modelDiscoveryDetails: Partial<Record<AgentKind, string>>;
  onRefreshModels: (kind: AgentKind) => void;
  onAutonomousShipChange: (enabled: boolean) => void;
  onVerificationConfigChange: (config: VerificationConfig) => void;
  onSaveVerificationConfig: () => void;
}) {
  return (
    <section className="utility-screen" aria-labelledby="settings-title">
      <header className="utility-header utility-header-actions">
        <div>
          <span className="eyebrow">Local runtime</span>
          <h1 id="settings-title">Models and runtime</h1>
          <p>Choose exact provider models once per project. Agent Room stores the requested choice and records what each CLI reports for every phase.</p>
        </div>
        <button type="button" className="secondary-button" onClick={onRefresh} disabled={refreshing}>
          <RefreshCw size={15} className={refreshing ? "spinning" : undefined} />
          {refreshing ? "Checking providers" : "Recheck providers"}
        </button>
      </header>
      {!native && <p className="empty-state">Provider checks are available in the Tauri desktop app.</p>}
      {error && <p className="utility-error" role="alert">{error}</p>}
      <div className="autonomy-setting">
        <div>
          <strong>Hands-free autonomous Ship</strong>
          <p>When armed, an agent can apply the autonomous-ship skill and start the isolated Ship workflow without another approval. Verification, review, bounded recovery, and promotion gates remain coordinator-owned.</p>
        </div>
        <label className="switch-control">
          <input
            type="checkbox"
            checked={projectSettings.autonomousShipEnabled}
            disabled={!native || savingProjectSettings}
            onChange={(event) => onAutonomousShipChange(event.target.checked)}
          />
          <span>{projectSettings.autonomousShipEnabled ? "Armed" : "Off"}</span>
        </label>
      </div>
      <section className="verification-setting" aria-labelledby="verification-settings-title">
        <div>
          <strong id="verification-settings-title">Verification</strong>
          <p>Commands are detected once when a repository is attached. Edit them here; they will not be replaced automatically.</p>
        </div>
        <label className="switch-control">
          <input
            type="checkbox"
            checked={verificationConfig.enabled}
            disabled={!native || savingProjectSettings}
            onChange={(event) => onVerificationConfigChange({ ...verificationConfig, enabled: event.target.checked })}
          />
          <span>{verificationConfig.enabled ? "Enabled" : "Disabled"}</span>
        </label>
        <label className="verification-field">
          <span>Prepare command</span>
          <input
            value={verificationConfig.prepare ?? ""}
            disabled={!native || savingProjectSettings}
            placeholder="Optional command before checks"
            onChange={(event) => onVerificationConfigChange({ ...verificationConfig, prepare: event.target.value || undefined })}
          />
        </label>
        <div className="verification-commands">
          {verificationConfig.commands.map((command, index) => (
            <div className="verification-command" key={`${command.label}-${index}`}>
              <input
                value={command.label}
                aria-label={`Verification label ${index + 1}`}
                disabled={!native || savingProjectSettings}
                onChange={(event) => onVerificationConfigChange({
                  ...verificationConfig,
                  commands: verificationConfig.commands.map((value, valueIndex) => valueIndex === index ? { ...value, label: event.target.value } : value),
                })}
              />
              <input
                value={command.command}
                aria-label={`Verification command ${index + 1}`}
                disabled={!native || savingProjectSettings}
                onChange={(event) => onVerificationConfigChange({
                  ...verificationConfig,
                  commands: verificationConfig.commands.map((value, valueIndex) => valueIndex === index ? { ...value, command: event.target.value } : value),
                })}
              />
              <label className="switch-control">
                <input
                  type="checkbox"
                  checked={command.enabled}
                  disabled={!native || savingProjectSettings}
                  onChange={(event) => onVerificationConfigChange({
                    ...verificationConfig,
                    commands: verificationConfig.commands.map((value, valueIndex) => valueIndex === index ? { ...value, enabled: event.target.checked } : value),
                  })}
                />
                <span>Run</span>
              </label>
              <button
                type="button"
                className="secondary-button"
                disabled={!native || savingProjectSettings}
                onClick={() => onVerificationConfigChange({ ...verificationConfig, commands: verificationConfig.commands.filter((_, valueIndex) => valueIndex !== index) })}
              >
                Remove
              </button>
            </div>
          ))}
        </div>
        <div className="verification-actions">
          <button
            type="button"
            className="secondary-button"
            disabled={!native || savingProjectSettings || verificationConfig.commands.length >= 4}
            onClick={() => onVerificationConfigChange({
              ...verificationConfig,
              commands: [...verificationConfig.commands, { label: "Project check", command: "", enabled: true }],
            })}
          >
            Add check
          </button>
          <button type="button" className="primary-button" disabled={!native || savingProjectSettings} onClick={onSaveVerificationConfig}>
            {savingProjectSettings ? "Saving…" : "Save verification"}
          </button>
        </div>
      </section>
      <div className="settings-grid">
        {environment.participants.map((participant) => (
          <ProviderProfileCard
            key={participant.kind}
            participant={participant}
            profiles={profiles.filter((profile) => profile.participantKind === participant.kind)}
            saving={savingKind === participant.kind}
            onSave={onSaveProfile}
            testing={testingKind === participant.kind}
            onTest={onTestConnection}
            models={modelCatalog[participant.kind] ?? participant.models}
            hasAuthoritativeCatalog={modelCatalog[participant.kind] !== undefined}
            discoveringModels={discoveringModelsKind === participant.kind}
            modelDiscoveryDetail={modelDiscoveryDetails[participant.kind]}
            onRefreshModels={onRefreshModels}
          />
        ))}
      </div>
      <p className="settings-disclosure">Token totals appear only when a CLI emits them in its native run output. Provider account quotas and reset windows are not scraped or guessed.</p>
    </section>
  );
}

function Inspector({
  project,
  environment,
  activeTab,
  setActiveTab,
  run,
  messages,
  receipts,
}: {
  project: Project;
  environment: NativeEnvironment;
  activeTab: InspectorTab;
  setActiveTab: (tab: InspectorTab) => void;
  run: Run;
  messages: RoomMessage[];
  receipts: ExecutionReceipt[];
}) {
  const tabs: InspectorTab[] = ["Repository", "Participants", "Evidence", "Memory"];
  const latestEvidence = [...messages].reverse().find((message) => message.kind === "evidence");
  const verification = latestEvidence?.verification ?? [];
  return (
    <aside className="inspector">
      <div className="inspector-tabs" role="tablist">
        {tabs.map((tab) => (
          <button
            type="button"
            key={tab}
            id={`inspector-tab-${tab.toLowerCase()}`}
            className={activeTab === tab ? "active" : ""}
            onClick={() => setActiveTab(tab)}
            role="tab"
            aria-selected={activeTab === tab}
            aria-controls={`inspector-panel-${tab.toLowerCase()}`}
            tabIndex={activeTab === tab ? 0 : -1}
          >
            {tab}
          </button>
        ))}
      </div>

      {activeTab === "Repository" && (
        <div
          className="inspector-body"
          id="inspector-panel-repository"
          role="tabpanel"
          aria-labelledby="inspector-tab-repository"
        >
          <div className="inspector-heading">
            <GitBranch size={17} />
            <span>
              <small>Base checkout</small>
              <strong>{project.name}</strong>
            </span>
          </div>
          <div className="data-row">
            <span>Branch</span>
            <code>{environment.branch || "unavailable"}</code>
          </div>
          <div className="data-row">
            <span>Run branch</span>
            <code>{run.branch ?? "Created per objective"}</code>
          </div>
          <div className="path-block">{environment.repositoryPath}</div>
          {run.worktreePath && (
            <div className="recovery-block">
              <HardDrive size={15} />
              <span>
                <strong>Recoverable worktree</strong>
                <code>{run.worktreePath}</code>
              </span>
            </div>
          )}
          {run.artifactPath && (
            <div className="recovery-block log-block">
              <Braces size={15} />
              <span>
                <strong>Durable run artifacts</strong>
                <code>{run.artifactPath}</code>
              </span>
            </div>
          )}
          <div className="status-note">
            <Check size={15} />
            <span>
              <strong>Promotion is gated</strong>
              <small>The base HEAD must remain clean and unchanged before a fast-forward.</small>
            </span>
          </div>
        </div>
      )}

      {activeTab === "Participants" && (
        <div
          className="inspector-body participant-list"
          id="inspector-panel-participants"
          role="tabpanel"
          aria-labelledby="inspector-tab-participants"
        >
          {environment.participants.map((participant) => (
            <div className="participant-card" key={participant.kind}>
              <div className="participant-row">
                <ParticipantMark participant={participant} />
                <span className="participant-copy">
                  <strong>{participant.name}</strong>
                  <small>{participant.installed ? participant.version ?? "Installed" : "Not installed"}</small>
                </span>
                <span className={`status-chip mode-${participant.capabilities.autonomyMode}`}>
                  {participant.kind === "antigravity"
                    ? "Ship only — no read-only mode"
                    : autonomyLabel(participant.capabilities.autonomyMode)}
                </span>
              </div>
              <p>{participant.capabilities.autonomyNote}</p>
              <div className="capability-line">
                <span>{participant.capabilities.exactResume ? "Resume" : "Fresh session"}</span>
                <span>{participant.capabilities.warmSession ? "Warm session" : "Cold start per turn"}</span>
                <span>{participant.capabilities.structuredOutput ? "Structured" : "Text result"}</span>
                <span>{participant.capabilities.streaming ? "Streaming" : "Completion only"}</span>
              </div>
              <ul className="capability-proof">
                {participant.capabilities.capabilityProof.map((proof) => (
                  <li key={proof}>{proof}</li>
                ))}
              </ul>
            </div>
          ))}
          <p className="inspector-help">
            Missing capabilities are shown as downgrades. Agent Room never substitutes an unavailable CLI.
          </p>
        </div>
      )}

      {activeTab === "Evidence" && (
        <div
          className="inspector-body"
          id="inspector-panel-evidence"
          role="tabpanel"
          aria-labelledby="inspector-tab-evidence"
        >
          <div className="evidence-tile">
            <ShieldCheck size={18} />
            <span>
              <strong>Coordinator-owned evidence</strong>
              <small>Git and process results decide completion, not an agent claim.</small>
            </span>
          </div>
          <div className="data-row">
            <span>Changed files</span>
            <strong>{latestEvidence?.changedFiles?.length ?? 0}</strong>
          </div>
          <div className="data-row">
            <span>Context packet</span>
            <strong>{contextLabel(run.contextBytes, environment.contextBudgetBytes)}</strong>
          </div>
          <div className="data-row">
            <span>Review</span>
            <strong>{run.degradedReview ? "Same provider" : run.reviewer ? "Independent" : "Pending"}</strong>
          </div>
          <div className="data-row">
            <span>Handoff contract</span>
            <strong>Schema v1</strong>
          </div>
          <div className="run-limits">
            <strong>Agent Room limits</strong>
            <small>48 KiB packet, 20 minute phase, 5 minute idle timeout, one revision, two recovery attempts.</small>
          </div>
          <section className="receipt-list" aria-labelledby="receipt-title">
            <h2 id="receipt-title">Execution receipts</h2>
            {receipts.length ? (
              <div className="evidence-table-wrap">
                <table className="evidence-table">
                  <thead>
                    <tr>
                      <th>Phase</th><th>Provider</th><th>Model</th><th>Context bytes</th><th>Saved bytes</th>
                      <th>Preflight</th><th>Process start</th><th>First output</th><th>Total</th><th>Tokens</th><th>Cost</th>
                    </tr>
                  </thead>
                  <tbody>
                    {receipts.map((receipt) => (
                      <tr key={receipt.id}>
                        <td>{receipt.phase}</td>
                        <td>{agentNames[receipt.participant]}</td>
                        <td>{receipt.actualModel ?? receipt.requestedModel ?? "Provider default"}</td>
                        <td>{receipt.contextBytes.toLocaleString()}</td>
                        <td>{receipt.packetBytesSaved.toLocaleString()}</td>
                        <td>{millisecondsLabel(receipt.preflightMs)}</td>
                        <td>{millisecondsLabel(receipt.processStartMs)}</td>
                        <td>{millisecondsLabel(receipt.firstOutputMs)}</td>
                        <td>{millisecondsLabel(receipt.totalMs)}</td>
                        <td>{[receipt.usage.inputTokens, receipt.usage.outputTokens].every((value) => value === undefined)
                          ? "—"
                          : `${receipt.usage.inputTokens?.toLocaleString() ?? "—"} in / ${receipt.usage.outputTokens?.toLocaleString() ?? "—"} out`}</td>
                        <td>{receipt.usage.totalCostUsd === undefined ? "—" : `$${receipt.usage.totalCostUsd.toFixed(4)}`}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            ) : (
              <p className="empty-state">Receipts appear after the first provider phase. They report run telemetry, not provider account quotas.</p>
            )}
          </section>
          <VerificationList verification={verification} />
        </div>
      )}

      {activeTab === "Memory" && (
        <div
          className="inspector-body memory-body"
          id="inspector-panel-memory"
          role="tabpanel"
          aria-labelledby="inspector-tab-memory"
        >
          <span className="memory-label">Current direction</span>
          <p>
            Carry one objective, compact handoffs, repository evidence, and review findings between coding CLIs.
          </p>
          <span className="memory-label">Autonomy boundary</span>
          <p>
            One writer, managed worktree, deterministic routing, one revision, two reviews, then complete or notify.
          </p>
          <span className="memory-label">Context policy</span>
          <p>Native session history plus a measured 48 KiB delta packet. No full room replay.</p>
          <span className="memory-label">Instruction hierarchy</span>
          {run.instructionFiles?.length ? (
            <div className="context-file-list">
              {run.instructionFiles.map((file) => (
                <code key={file}>{file}</code>
              ))}
            </div>
          ) : (
            <p>No repository AGENTS.md or CLAUDE.md files were discovered for this run.</p>
          )}
          <span className="memory-label">Selected project skills</span>
          {run.skillFiles?.length ? (
            <div className="context-file-list">
              {run.skillFiles.map((file) => (
                <code key={file}>{file}</code>
              ))}
            </div>
          ) : (
            <p>No repository-local skill matched this objective. Provider-global skills remain provider-owned.</p>
          )}
          <p className="recovery-status">
            <History size={15} />
            {run.recoveryCount ?? 0} of 2 recovery attempts used
          </p>
        </div>
      )}
    </aside>
  );
}

export function App() {
  const [project, setProject] = useState<Project>(detachedProject);
  const [projects, setProjects] = useState<Project[]>([]);
  const [environment, setEnvironment] = useState(detachedEnvironment);
  const [messages, setMessages] = useState<RoomMessage[]>([]);
  const [hasMoreMessages, setHasMoreMessages] = useState(false);
  const [loadingOlderMessages, setLoadingOlderMessages] = useState(false);
  const [receipts, setReceipts] = useState<ExecutionReceipt[]>([]);
  const [profiles, setProfiles] = useState<ProviderProfile[]>([]);
  const [projectSettings, setProjectSettings] = useState<ProjectSettings>({
    autonomousShipEnabled: false,
  });
  const [verificationConfig, setVerificationConfig] = useState<VerificationConfig>({
    enabled: true,
    commands: [],
  });
  const [savingProjectSettings, setSavingProjectSettings] = useState(false);
  const [modelCatalog, setModelCatalog] = useState<Partial<Record<AgentKind, string[]>>>({});
  const [modelDiscoveryDetails, setModelDiscoveryDetails] = useState<Partial<Record<AgentKind, string>>>({});
  const [discoveringModelsKind, setDiscoveringModelsKind] = useState<AgentKind>();
  const [run, setRun] = useState<Run>(emptyRun);
  const [objective, setObjective] = useState("");
  const [composerMode, setComposerMode] = useState<ComposerMode>("ask");
  const [chatSending, setChatSending] = useState(false);
  const [quickEdit, setQuickEdit] = useState<QuickEditResult>();
  const [activity, setActivity] = useState<LiveActivityItem[]>([]);
  const [streamTitle, setStreamTitle] = useState("Provider events");
  const [activeView, setActiveView] = useState<PrimaryView>("rooms");
  const [activeTab, setActiveTab] = useState<InspectorTab>("Repository");
  const [inspectorOpen, setInspectorOpen] = useState(false);
  const [searchOpen, setSearchOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [refreshingProviders, setRefreshingProviders] = useState(false);
  const [savingProfileKind, setSavingProfileKind] = useState<AgentKind>();
  const [testingConnectionKind, setTestingConnectionKind] = useState<AgentKind>();
  const [uiError, setUiError] = useState("");
  const [attention, setAttention] = useState<LiveActivityItem>();
  const timelineRef = useRef<HTMLDivElement>(null);
  const loadingOlderMessagesRef = useRef(false);
  const runRef = useRef(run);
  const chatRunRef = useRef<string | undefined>(undefined);
  const streamTextBufferRef = useRef("");
  const streamFrameRef = useRef<number | undefined>(undefined);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const contextToggleRef = useRef<HTMLButtonElement>(null);
  const native = isNativeApp();

  useEffect(() => {
    runRef.current = run;
  }, [run]);

  async function refreshRoom(projectId = project.id) {
    if (!native) return;
    const snapshot = await loadRoom(projectId);
    setMessages(snapshot.messages);
    setHasMoreMessages(snapshot.hasMore);
    setRun(snapshot.latestRun ? hydrateRun(snapshot.latestRun) : emptyRun);
    setReceipts(snapshot.receipts);
  }

  async function activateProject(nextProject: Project, knownEnvironment?: NativeEnvironment) {
    const nextEnvironment = knownEnvironment ?? await getEnvironment();
    const [snapshot, nextProfiles, nextSettings, nextVerificationConfig, nextProjects] = await Promise.all([
      loadRoom(nextProject.id),
      loadProviderProfiles(nextProject.id),
      loadProjectSettings(nextProject.id),
      loadVerificationConfig(nextProject.id),
      projectList(),
    ]);
    setProject(nextProject);
    setProjects(nextProjects);
    setEnvironment(nextEnvironment);
    setMessages(snapshot.messages);
    setHasMoreMessages(snapshot.hasMore);
    setRun(snapshot.latestRun ? hydrateRun(snapshot.latestRun) : emptyRun);
    setReceipts(snapshot.receipts);
    setProfiles(nextProfiles);
    setProjectSettings(nextSettings);
    setVerificationConfig(nextVerificationConfig);
    setActiveView("rooms");
    setUiError("");
  }

  useEffect(() => {
    if (!native) {
      if (import.meta.env.DEV) {
        void import("./seed").then(({ previewEnvironment, seedMessages, seedProject, seedRun }) => {
          setProject(seedProject);
          setProjects([seedProject]);
          setEnvironment(previewEnvironment);
          setMessages(seedMessages);
          setRun(seedRun);
        });
      }
      return;
    }
    let disposed = false;
    let unlisten: (() => void) | undefined;

    getEnvironment()
      .then(async (nextEnvironment) => {
        if (disposed) return;
        const [nextProject, nextProjects] = await Promise.all([projectActive(), projectList()]);
        if (disposed) return;
        if (!nextEnvironment.attached || !nextProject) {
          setEnvironment(nextEnvironment);
          setProjects(nextProjects);
          return;
        }
        await activateProject(nextProject, nextEnvironment);
      })
      .catch((error) => console.error("Failed to inspect native environment", error));

    onRunEvent((event: RunEvent) => {
      if (event.eventType === "stream" || event.eventType === "text-delta") {
        if (
          (event.phase === "chat" && event.runId !== chatRunRef.current) ||
          (event.phase !== "chat" && event.runId !== runRef.current.id)
        ) {
          return;
        }
        setStreamTitle(event.agent ? `${agentNames[event.agent]} / ${event.phase}` : event.title);
        if (event.eventType === "text-delta" && event.agent) {
          const agent = event.agent;
          streamTextBufferRef.current += event.textDelta ?? "";
          if (streamFrameRef.current === undefined) {
            streamFrameRef.current = requestAnimationFrame(() => {
              streamFrameRef.current = undefined;
              const body = streamTextBufferRef.current;
              setActivity([]);
              setMessages((current) => {
                const existingIndex = current.findIndex(
                  (message) =>
                    message.runId === event.runId &&
                    message.kind === "agent" &&
                    message.sender === agent,
                );
                const message: RoomMessage = {
                  id: existingIndex >= 0 ? current[existingIndex].id : `live-${event.runId}`,
                  kind: "agent",
                  sender: agent,
                  body,
                  createdAt:
                    existingIndex >= 0
                      ? current[existingIndex].createdAt
                      : new Date().toISOString(),
                  runId: event.runId,
                };
                if (existingIndex < 0) return [...current, message];
                return current.map((currentMessage, index) =>
                  index === existingIndex ? message : currentMessage,
                );
              });
            });
          }
          return;
        }
        if (
          (event.title === "Chat response" ||
            event.title === "Native chat response") &&
          event.agent
        ) {
          const agent = event.agent;
          setActivity([]);
          setMessages((current) => {
            const existingIndex = current.findIndex(
              (message) =>
                message.runId === event.runId &&
                message.kind === "agent" &&
                message.sender === agent,
            );
            const message: RoomMessage = {
              id: existingIndex >= 0 ? current[existingIndex].id : `live-${event.runId}`,
              kind: "agent",
              sender: agent,
              body: event.detail,
              createdAt:
                existingIndex >= 0
                  ? current[existingIndex].createdAt
                  : new Date().toISOString(),
              runId: event.runId,
            };
            if (existingIndex < 0) return [...current, message];
            return current.map((currentMessage, index) =>
              index === existingIndex ? message : currentMessage,
            );
          });
        } else {
          setActivity((current) => {
            const next = { title: event.title, detail: event.detail };
            return event.title.startsWith("Native terminal")
              ? [next]
              : [...current, next].slice(-12);
          });
        }
        return;
      }
      if (event.runId === chatRunRef.current) {
        setStreamTitle(event.agent ? `${agentNames[event.agent]} / chat` : event.title);
        if (event.eventType === "attention") {
          setMessages((current) => [
            ...current,
            {
              id: crypto.randomUUID(),
              kind: "error",
              sender: "system",
              body: event.title,
              reason: event.detail,
              createdAt: new Date().toISOString(),
              runId: event.runId,
            },
          ]);
          chatRunRef.current = undefined;
          setChatSending(false);
        } else if (event.eventType === "complete") {
          setChatSending(false);
        }
        return;
      }
      if (
        event.runId === runRef.current.id &&
        (event.eventType === "phase" || event.eventType === "attention")
      ) {
        setStreamTitle(
          event.agent
            ? `${agentNames[event.agent]} / ${event.phase}`
            : `Agent Room / ${event.phase}`,
        );
        setActivity((current) => [
          ...current,
          { title: event.title, detail: event.detail },
        ].slice(-12));
      }
      if (event.runId === runRef.current.id && event.eventType === "attention") {
        setAttention({ title: event.title, detail: event.detail });
      }
      setRun((current) => {
        if (current.id !== event.runId) return current;
        return {
          ...current,
          state: asRunState(event.state),
          currentOwner: event.agent,
          contextBytes: Math.max(current.contextBytes ?? 0, event.contextBytes ?? 0),
          stopReason: event.eventType === "attention" ? event.detail : current.stopReason,
          route: current.route,
        };
      });
      if (event.runId === runRef.current.id) {
        void refreshRoom(project.id).catch(() => undefined);
      }
    }).then((dispose) => {
      if (disposed) dispose();
      else unlisten = dispose;
    });

    return () => {
      disposed = true;
      unlisten?.();
      if (streamFrameRef.current !== undefined) {
        cancelAnimationFrame(streamFrameRef.current);
      }
    };
  }, [native]);

  useEffect(() => {
    if (loadingOlderMessagesRef.current) {
      loadingOlderMessagesRef.current = false;
      return;
    }
    timelineRef.current?.scrollTo({
      top: timelineRef.current.scrollHeight,
      behavior: "smooth",
    });
  }, [messages, activity]);

  async function loadOlderMessages() {
    const oldest = messages[0];
    if (!native || !oldest || !hasMoreMessages || loadingOlderMessages) return;
    const timeline = timelineRef.current;
    const scrollHeight = timeline?.scrollHeight ?? 0;
    const scrollTop = timeline?.scrollTop ?? 0;
    loadingOlderMessagesRef.current = true;
    setLoadingOlderMessages(true);
    try {
      const snapshot = await loadRoom(project.id, { createdAt: oldest.createdAt, id: oldest.id });
      setMessages((current) => {
        const known = new Set(current.map((message) => message.id));
        return [...snapshot.messages.filter((message) => !known.has(message.id)), ...current];
      });
      setHasMoreMessages(snapshot.hasMore);
      requestAnimationFrame(() => {
        if (timeline) timeline.scrollTop = scrollTop + timeline.scrollHeight - scrollHeight;
      });
    } catch (error) {
      setUiError(error instanceof Error ? error.message : String(error));
    } finally {
      setLoadingOlderMessages(false);
    }
  }

  useEffect(() => {
    function handleShortcut(event: KeyboardEvent) {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setSearchOpen(true);
        requestAnimationFrame(() => searchInputRef.current?.focus());
      }
      if (event.key === "Escape") {
        if (inspectorOpen) {
          setInspectorOpen(false);
          requestAnimationFrame(() => contextToggleRef.current?.focus());
          return;
        }
        if (searchOpen) {
          setSearchOpen(false);
          setSearchQuery("");
        }
      }
    }
    window.addEventListener("keydown", handleShortcut);
    return () => window.removeEventListener("keydown", handleShortcut);
  }, [inspectorOpen, searchOpen]);

  const installedCount = useMemo(
    () => environment.participants.filter(isRunnableParticipant).length,
    [environment],
  );
  const activeShipAgent = run.currentOwner
    ?? (run.state === "reviewing" ? run.reviewer : run.writer)
    ?? run.reviewer;
  const sideChatAvailable = activeStates.includes(run.state)
    && run.state !== "promoting"
    && activeShipAgent !== "antigravity";

  const visibleMessages = useMemo(() => {
    const query = searchQuery.trim().toLocaleLowerCase();
    if (!query) return messages;
    return messages.filter((message) =>
      [message.body, message.reason, message.sender, message.kind]
        .filter(Boolean)
        .some((value) => value?.toLocaleLowerCase().includes(query)),
    );
  }, [messages, searchQuery]);

  function openInspector(tab: InspectorTab) {
    setActiveTab(tab);
    setInspectorOpen(true);
  }

  function showView(view: PrimaryView) {
    setActiveView(view);
    if (view === "rooms") setSearchQuery("");
  }

  async function handleAttachProject() {
    if (!native) {
      setUiError("Repository attachment is available in the Tauri desktop app.");
      return;
    }
    try {
      const path = await projectPick();
      if (!path) return;
      const nextProject = await projectAttach(path);
      await activateProject(nextProject);
    } catch (error) {
      setUiError(error instanceof Error ? error.message : String(error));
    }
  }

  async function handleSelectProject(id: string) {
    if (!native || id === project.id) return;
    try {
      await activateProject(await projectSelect(id));
    } catch (error) {
      setUiError(error instanceof Error ? error.message : String(error));
    }
  }

  async function refreshProviders() {
    if (!native) {
      setUiError("Provider checks are available in the Tauri desktop app.");
      return;
    }
    setRefreshingProviders(true);
    setUiError("");
    try {
      const nextEnvironment = await getEnvironment(true);
      setEnvironment(nextEnvironment);
      setProject((current) => ({ ...current, branch: nextEnvironment.branch }));
    } catch (error) {
      setUiError(error instanceof Error ? error.message : String(error));
    } finally {
      setRefreshingProviders(false);
    }
  }

  async function handleSaveProfiles(nextProfiles: ProviderProfile[]) {
    setUiError("");
    const participantKind = nextProfiles[0]?.participantKind;
    if (!participantKind) return;
    setSavingProfileKind(participantKind);
    try {
      if (native) {
        for (const profile of nextProfiles) {
          await saveProviderProfile(project.id, profile);
        }
      }
      setProfiles((current) => [
        ...current.filter((value) => value.participantKind !== participantKind),
        ...nextProfiles,
      ]);
    } catch (error) {
      setUiError(error instanceof Error ? error.message : String(error));
    } finally {
      setSavingProfileKind(undefined);
    }
  }

  async function handleAutonomousShipChange(enabled: boolean) {
    const nextSettings = { autonomousShipEnabled: enabled };
    setSavingProjectSettings(true);
    setUiError("");
    try {
      if (native) await saveProjectSettings(project.id, nextSettings);
      setProjectSettings(nextSettings);
    } catch (error) {
      setUiError(error instanceof Error ? error.message : String(error));
    } finally {
      setSavingProjectSettings(false);
    }
  }

  async function handleSaveVerificationConfig() {
    setSavingProjectSettings(true);
    setUiError("");
    try {
      if (native) await saveVerificationConfig(project.id, verificationConfig);
    } catch (error) {
      setUiError(error instanceof Error ? error.message : String(error));
    } finally {
      setSavingProjectSettings(false);
    }
  }

  async function handleTestConnection(kind: AgentKind, draft: ProviderDraft) {
    if (!native) {
      setUiError("Connection tests are available in the Tauri desktop app.");
      return;
    }
    setTestingConnectionKind(kind);
    setUiError("");
    try {
      const participant = await testProviderConnection({
        projectId: project.id,
        repositoryPath: project.repositoryPath,
        participantKind: kind,
        model: draft.model,
        effort: draft.effort,
      });
      setEnvironment((current) => ({
        ...current,
        participants: current.participants.map((value) => value.kind === kind ? participant : value),
      }));
    } catch (error) {
      setUiError(error instanceof Error ? error.message : String(error));
      await refreshProviders();
    } finally {
      setTestingConnectionKind(undefined);
    }
  }

  async function handleRefreshModels(kind: AgentKind) {
    if (!native) {
      setUiError("Model discovery is available in the Tauri desktop app.");
      return;
    }
    setDiscoveringModelsKind(kind);
    setUiError("");
    try {
      const result = await discoverProviderModels(kind);
      setModelCatalog((current) => ({ ...current, [kind]: result.models }));
      setModelDiscoveryDetails((current) => ({ ...current, [kind]: result.detail }));
      const staleProfiles = profiles.filter((profile) => (
        profile.participantKind === kind
        && Boolean(profile.model)
        && !result.models.includes(profile.model!)
      ));
      if (staleProfiles.length > 0) {
        setSavingProfileKind(kind);
        await Promise.all(staleProfiles.map((profile) => saveProviderProfile(project.id, {
          ...profile,
          model: undefined,
          effort: profile.participantKind === "cursor" ? undefined : profile.effort,
        })));
        const staleRoutes = new Set(staleProfiles.map((profile) => profile.route));
        setProfiles((current) => current.map((profile) => (
          profile.participantKind === kind && staleRoutes.has(profile.route)
            ? {
              ...profile,
              model: undefined,
              effort: profile.participantKind === "cursor" ? undefined : profile.effort,
            }
            : profile
        )));
      }
    } catch (error) {
      setUiError(error instanceof Error ? error.message : String(error));
    } finally {
      setSavingProfileKind(undefined);
      setDiscoveringModelsKind(undefined);
    }
  }

  async function runShipObjective(
    text: string,
    requestedWriter?: AgentKind,
    recordHumanMessage = true,
  ) {
    setUiError("");

    const writer = environment.participants.find(
      (participant) => participant.kind === requestedWriter && isRunnableParticipant(participant),
    )?.kind;
    const nextRun: Run = {
      id: crypto.randomUUID(),
      objective: text,
      state: "selecting",
      currentOwner: writer,
      writer,
      route: [],
      reviewCount: 0,
      revisionCount: 0,
      startedAt: new Date().toISOString(),
    };
    setRun(nextRun);
    setComposerMode("ask");
    setObjective("");
    setActivity([]);
    if (recordHumanMessage) {
      setMessages((current) => [
        ...current,
        {
          id: crypto.randomUUID(),
          kind: "human",
          sender: "human",
          body: text,
          createdAt: new Date().toISOString(),
          runId: nextRun.id,
        },
      ]);
    }

    if (!native) {
      setRun({
        ...nextRun,
        state: "waiting",
        stopReason: "Desktop runtime required to execute this objective.",
      });
      setMessages((current) => [
        ...current,
        {
          id: crypto.randomUUID(),
          kind: "status",
          sender: "system",
          body: "Preview mode shows the v1 interface but never starts provider CLIs.",
          reason: "Launch the Tauri desktop app to create a managed worktree and run the autonomous route.",
          createdAt: new Date().toISOString(),
          runId: nextRun.id,
        },
      ]);
      return;
    }

    try {
      await startRoomRun({
        runId: nextRun.id,
        projectId: project.id,
        objective: text,
        repositoryPath: project.repositoryPath,
        requestedAgent: writer,
      });
      await refreshRoom(project.id);
      const nextEnvironment = await getEnvironment();
      setEnvironment(nextEnvironment);
      setProject((current) => ({ ...current, branch: nextEnvironment.branch }));
    } catch (error) {
      const detail = error instanceof Error ? error.message : String(error);
      setUiError(detail);
      setRun((current) => ({ ...current, state: "failed", stopReason: detail }));
      setMessages((current) => [
        ...current,
        {
          id: crypto.randomUUID(),
          kind: "error",
          sender: "system",
          body: "The autonomous route could not start.",
          reason: detail,
          createdAt: new Date().toISOString(),
          runId: nextRun.id,
        },
      ]);
      await refreshRoom(project.id).catch(() => undefined);
    }
  }

  async function submitObjective() {
    const text = objective.trim();
    if (!text) return;
    if (run.state === "promoting") {
      setUiError("Promotion is finishing now. Side chat will reopen when this Ship run completes.");
      return;
    }
    if (sideChatAvailable) {
      await submitChat(true);
      return;
    }
    await runShipObjective(text);
  }

  async function submitChat(activeShip = false) {
    const text = objective.trim();
    if (!text || chatSending) return;
    const priorAgent = [...messages].reverse().find(
      (message) => message.kind === "agent" && message.sender !== "system" && message.sender !== "human",
    )?.sender as AgentKind | undefined;
    const participant = activeShip && activeShipAgent
      ? activeShipAgent
      : chatAgentFor(text, environment.participants, priorAgent);
    const chatRunId = crypto.randomUUID();
    setUiError("");
    setObjective("");
    setActivity([]);
    streamTextBufferRef.current = "";
    setStreamTitle(
      participant
        ? `${agentNames[participant]} / ${activeShip ? "active run" : "chat"}`
        : "Project chat",
    );
    setMessages((current) => [
      ...current,
      {
        id: crypto.randomUUID(),
        kind: "human",
        sender: "human",
        body: text,
        createdAt: new Date().toISOString(),
        runId: chatRunId,
      },
    ]);
    if (!participant) {
      setMessages((current) => [
        ...current,
        {
          id: crypto.randomUUID(),
          kind: "error",
          sender: "system",
          body: "No installed participant can answer this chat.",
          createdAt: new Date().toISOString(),
          runId: chatRunId,
        },
      ]);
      return;
    }
    if (!native) {
      setMessages((current) => [
        ...current,
        {
          id: crypto.randomUUID(),
          kind: "status",
          sender: "system",
          body: "Project chat is available in the Tauri desktop app.",
          reason: "The browser preview never starts provider CLIs.",
          createdAt: new Date().toISOString(),
          runId: chatRunId,
        },
      ]);
      return;
    }
    chatRunRef.current = chatRunId;
    setChatSending(true);
    try {
      const result = await startRoomChat({
        runId: chatRunId,
        projectId: project.id,
        message: text,
        repositoryPath: project.repositoryPath,
        requestedAgent: participant,
        activeRunId: activeShip ? run.id : undefined,
      });
      if (result.shipIntent && !activeShip) {
        setComposerMode("ship");
        setObjective(result.shipIntent.objective);
        await refreshRoom(project.id);
      } else {
        await refreshRoom(project.id);
      }
    } catch (error) {
      const detail = error instanceof Error ? error.message : String(error);
      setUiError(detail);
      await refreshRoom(project.id).catch(() => undefined);
    } finally {
      setChatSending(false);
      chatRunRef.current = undefined;
    }
  }

  async function submitQuickEdit() {
    const text = objective.trim();
    if (!text || chatSending) return;
    const priorAgent = [...messages].reverse().find(
      (message) => message.kind === "agent" && message.sender !== "system" && message.sender !== "human",
    )?.sender as AgentKind | undefined;
    const participant = chatAgentFor(text, environment.participants, priorAgent);
    if (!participant || !native) {
      setUiError(participant ? "Quick Edit is available in the Tauri desktop app." : "No Full-tier participant can perform a Quick Edit.");
      return;
    }
    const editId = crypto.randomUUID();
    setUiError("");
    setObjective("");
    setChatSending(true);
    try {
      setQuickEdit(await quickEditStart({
        editId,
        projectId: project.id,
        message: text,
        repositoryPath: project.repositoryPath,
        requestedAgent: participant,
      }));
    } catch (error) {
      setUiError(error instanceof Error ? error.message : String(error));
    } finally {
      setChatSending(false);
    }
  }

  async function applyQuickEdit() {
    if (!quickEdit) return;
    setUiError("");
    try {
      await quickEditApply(quickEdit.editId);
      setQuickEdit(undefined);
      await refreshRoom(project.id);
    } catch (error) {
      setUiError(error instanceof Error ? error.message : String(error));
    }
  }

  async function discardQuickEdit() {
    if (!quickEdit) return;
    setUiError("");
    try {
      await quickEditDiscard(quickEdit.editId);
      setQuickEdit(undefined);
    } catch (error) {
      setUiError(error instanceof Error ? error.message : String(error));
    }
  }

  async function submitComposer(event: FormEvent) {
    event.preventDefault();
    if (sideChatAvailable) await submitChat(true);
    else if (composerMode === "ask") await submitChat();
    else if (composerMode === "quick-edit") await submitQuickEdit();
    else await submitObjective();
  }

  async function handleStopChat() {
    const chatRunId = chatRunRef.current;
    if (!chatRunId) return;
    try {
      await stopRun(chatRunId);
      setChatSending(false);
      chatRunRef.current = undefined;
    } catch (error) {
      setUiError(error instanceof Error ? error.message : String(error));
    }
  }

  async function handleStop() {
    if (native) {
      try {
        const result = await stopRun(runRef.current.id);
        if (!result.cancelled) {
          setUiError(result.reason);
          return;
        }
        setRun((current) => ({ ...current, stopReason: result.reason }));
      } catch (error) {
        setUiError(error instanceof Error ? error.message : String(error));
        return;
      }
    }
    if (!native) {
      setRun((current) => ({
        ...current,
        state: "stopped",
        stopReason: "Stop requested. Agent Room is preserving recoverable work.",
      }));
    }
  }

  async function handleAbandon() {
    if (!native || activeStates.includes(run.state) || !run.worktreePath) return;
    if (!window.confirm("Abandon this Ship run and delete its preserved worktree? This cannot be undone.")) return;
    setUiError("");
    try {
      await abandonRun(run.id);
      await refreshRoom(project.id);
    } catch (error) {
      setUiError(error instanceof Error ? error.message : String(error));
    }
  }

  async function handleResume() {
    if (
      !native ||
      !run.worktreePath ||
      activeStates.includes(run.state) ||
      (run.recoveryCount ?? 0) >= 2
    ) {
      return;
    }
    setUiError("");
    setActivity([]);
    setComposerMode("ask");
    setRun((current) => ({
      ...current,
      state: "working",
      currentOwner: current.writer,
      stopReason: undefined,
      recoveryCount: (current.recoveryCount ?? 0) + 1,
      route: current.route,
    }));
    try {
      await startRoomRun({
        runId: run.id,
        projectId: project.id,
        objective: run.objective,
        repositoryPath: project.repositoryPath,
        requestedAgent: run.writer,
      });
      await refreshRoom(project.id);
      setEnvironment(await getEnvironment());
    } catch (error) {
      const detail = error instanceof Error ? error.message : String(error);
      setUiError(detail);
      setRun((current) => ({ ...current, state: "failed", stopReason: detail }));
      await refreshRoom(project.id).catch(() => undefined);
    }
  }

  return (
    <div className="app-shell" data-run-state={run.state}>
      <a className="skip-link" href="#room-main">
        Skip to room
      </a>
      <TitleBar
        project={project}
        native={native}
        autonomousShipEnabled={projectSettings.autonomousShipEnabled}
        searchOpen={searchOpen}
        searchQuery={searchQuery}
        searchInputRef={searchInputRef}
        inspectorOpen={inspectorOpen}
        contextToggleRef={contextToggleRef}
        onOpenRepository={() => {
          setActiveView("rooms");
          openInspector("Repository");
        }}
        onSearchOpen={() => {
          setSearchOpen(true);
          requestAnimationFrame(() => searchInputRef.current?.focus());
        }}
        onSearchClose={() => {
          setSearchOpen(false);
          setSearchQuery("");
        }}
        onSearchQueryChange={setSearchQuery}
        onInspectorToggle={() => setInspectorOpen((open) => !open)}
        onToggleMaximize={() => {
          if (native) void getCurrentWindow().toggleMaximize();
        }}
      />

      <NavRail
        activeView={activeView}
        onNavigate={showView}
        projects={projects}
        activeProjectId={project.id}
        onSelectProject={handleSelectProject}
        onAttachProject={handleAttachProject}
      />

      <main className="room" id="room-main" tabIndex={-1}>
        {(activeStates.includes(run.state) || ["waiting", "failed", "stopped"].includes(run.state)) && (
          <StatusPill
            state={run.state}
            stage={run.route.find((step) => step.state === "current")?.label ?? "Ship run"}
            elapsed={elapsedTime(run.startedAt)}
          />
        )}
        {!environment.attached ? (
          <AttachProjectView
            projects={projects}
            native={native}
            onAttach={handleAttachProject}
            onSelect={handleSelectProject}
          />
        ) : activeView === "activity" ? (
          <ActivityView messages={visibleMessages} query={searchQuery} />
        ) : activeView === "settings" ? (
          <SettingsView
            environment={environment}
            profiles={profiles}
            projectSettings={projectSettings}
            verificationConfig={verificationConfig}
            savingProjectSettings={savingProjectSettings}
            savingKind={savingProfileKind}
            refreshing={refreshingProviders}
            native={native}
            error={uiError}
            onRefresh={refreshProviders}
            onSaveProfile={handleSaveProfiles}
            testingKind={testingConnectionKind}
            onTestConnection={handleTestConnection}
            modelCatalog={modelCatalog}
            discoveringModelsKind={discoveringModelsKind}
            modelDiscoveryDetails={modelDiscoveryDetails}
            onRefreshModels={handleRefreshModels}
            onAutonomousShipChange={handleAutonomousShipChange}
            onVerificationConfigChange={setVerificationConfig}
            onSaveVerificationConfig={handleSaveVerificationConfig}
          />
        ) : (
          <>
        <div className="room-header">
          <div>
            <span className="eyebrow">Autonomous project room</span>
            <h1>{project.name}</h1>
            <p>{project.goal}</p>
          </div>
          <div className="room-meta">
            <span>
              <GitBranch size={14} />
              {project.branch}
            </span>
            <span>
              <Bot size={14} />
              {installedCount} of 4 ready
            </span>
          </div>
        </div>

        <Conversation
          ref={timelineRef}
          messages={messages}
          streaming={{
            active: chatSending || activeStates.includes(run.state),
            participant:
              environment.participants.find((participant) => streamTitle.startsWith(participant.name))?.kind
              ?? activeShipAgent,
            runId: chatSending ? chatRunRef.current : run.id || undefined,
          }}
          hasMore={hasMoreMessages}
          loadingOlder={loadingOlderMessages}
          onLoadOlder={() => void loadOlderMessages()}
          query={searchQuery}
          run={run}
          receipts={receipts}
          renderMessage={(message) => <TimelineMessageContent message={message} />}
          onResume={handleResume}
        >
          {attention && (
            <AttentionCard title={attention.title} detail={attention.detail} onDismiss={() => setAttention(undefined)} />
          )}
          {run.state !== "ready" && !searchQuery && (
            <RunProgressCard
              run={run}
              activity={activity}
              limits={`${run.revisionCount}/1 revise · ${run.reviewCount}/2 review · ${run.recoveryCount ?? 0}/2 recover`}
              onStop={handleStop}
              onResume={handleResume}
              onAbandon={handleAbandon}
            />
          )}
        </Conversation>

        <Composer
          busy={chatSending}
          canSelectParticipant={(participant) =>
            sideChatAvailable
              ? participant.kind === activeShipAgent
              : composerMode === "ask" || composerMode === "quick-edit"
                ? canUseChat(participant)
                : run.state !== "promoting" && isRunnableParticipant(participant)
          }
          contextualLabel={
            composerMode === "ask"
              ? sideChatAvailable ? "Ask about active Ship run" : "Ask a question"
              : composerMode === "quick-edit"
                ? "Quick edit"
                : run.state === "promoting"
                  ? "Promoting verified work"
                  : sideChatAvailable ? "Ask about active Ship run" : "Engineering objective"
          }
          error={uiError}
          inputDescribedBy={uiError ? "composer-error" : undefined}
          mode={composerMode}
          onModeChange={setComposerMode}
          onObjectiveChange={setObjective}
          onParticipantSelect={(participant) =>
            setObjective((current) => `@${participant.kind} ${current.replace(/^@\w+\s*/, "")}`)
          }
          onStop={() => void handleStopChat()}
          onSubmit={submitComposer}
          participants={environment.participants}
          placeholder={
            composerMode === "ask"
              ? sideChatAvailable
                ? `Ask ${activeShipAgent ? agentNames[activeShipAgent] : "the active agent"} what is happening...`
                : "Ask a question..."
              : composerMode === "quick-edit"
                ? "Describe a small, bounded edit..."
                : run.state === "promoting"
                  ? "Promotion is finishing safely..."
                  : sideChatAvailable
                    ? `Ask ${activeShipAgent ? agentNames[activeShipAgent] : "the active agent"} what is happening...`
                    : "@codex State the objective once..."
          }
          promoting={run.state === "promoting"}
          quickEditDisabled={sideChatAvailable || run.state === "promoting" || Boolean(quickEdit)}
          quickEditPreview={quickEdit && (
            <>
              <p><strong>{agentNames[quickEdit.participant]}</strong>: {quickEdit.summary}</p>
              <pre>{quickEdit.diff || "No file changes were produced."}</pre>
              <div className="composer-preview-actions">
                <button type="button" className="composer-preview-apply" onClick={() => void applyQuickEdit()} disabled={!quickEdit.diff}>
                  <Check size={15} /> Apply edit
                </button>
                <button type="button" className="danger-button" onClick={() => void discardQuickEdit()}>
                  <X size={15} /> Discard
                </button>
              </div>
            </>
          )}
          renderParticipantMark={(participant) => <ParticipantMark participant={participant} />}
          shipDisabled={sideChatAvailable || run.state === "promoting"}
          sideChatAvailable={sideChatAvailable}
          titleForParticipant={(participant) =>
            (composerMode === "ask" || composerMode === "quick-edit" ? canUseChat(participant) : isRunnableParticipant(participant))
              ? `${autonomyLabel(participant.capabilities.autonomyMode)}: ${participant.capabilities.autonomyNote}`
              : participant.capabilities.autonomyNote
          }
          value={objective}
        />
          </>
        )}
      </main>

      {inspectorOpen && (
        <>
          <div
            className="inspector-scrim"
            aria-hidden="true"
            onClick={() => {
              setInspectorOpen(false);
              requestAnimationFrame(() => contextToggleRef.current?.focus());
            }}
          />
          <div className="inspector-wrap open" id="room-context">
            <button
              type="button"
              className="inspector-close"
              aria-label="Close context"
              onClick={() => {
                setInspectorOpen(false);
                requestAnimationFrame(() => contextToggleRef.current?.focus());
              }}
            >
              <X size={18} />
            </button>
            <Inspector
              project={project}
              environment={environment}
              activeTab={activeTab}
              setActiveTab={setActiveTab}
              run={run}
              messages={messages}
              receipts={receipts}
            />
          </div>
        </>
      )}
    </div>
  );
}
