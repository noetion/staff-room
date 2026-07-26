import {
  Activity,
  ArrowRight,
  Bot,
  Braces,
  Check,
  ChevronRight,
  CircleAlert,
  CircleStop,
  Gauge,
  GitBranch,
  HardDrive,
  History,
  Minus,
  PanelRight,
  Play,
  RefreshCw,
  Search,
  Settings,
  ShieldCheck,
  Square,
  Sparkles,
  TerminalSquare,
  Workflow,
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
import { createRun, participantCanChat, participantIsRunnable, routeForPhase, selectChatParticipant, selectParticipant } from "./coordination";
import type {
  AgentKind,
  ExecutionReceipt,
  NativeEnvironment,
  Participant,
  ProviderProfile,
  Project,
  ProjectSettings,
  RoomMessage,
  Run,
  RunState,
  StoredRun,
  VerificationResult,
} from "./model";
import { agentNames } from "./model";
import {
  getEnvironment,
  discoverProviderModels,
  isNativeApp,
  loadProviderProfiles,
  loadProjectSettings,
  loadRoom,
  onRunEvent,
  saveProject,
  saveProviderProfile,
  saveProjectSettings,
  startRoomChat,
  startRoomRun,
  stopRun,
  testProviderConnection,
  type RunEvent,
} from "./native";
import { previewEnvironment, seedMessages, seedProject, seedRun } from "./seed";

type InspectorTab = "Repository" | "Participants" | "Evidence" | "Memory";
type PrimaryView = "rooms" | "activity" | "settings";
type ComposerMode = "chat" | "ship";
type LiveActivityItem = { title: string; detail: string };

const activeStates: RunState[] = [
  "selecting",
  "working",
  "verifying",
  "reviewing",
  "revising",
  "promoting",
];

const validStates: RunState[] = [
  "ready",
  "selecting",
  "working",
  "verifying",
  "reviewing",
  "revising",
  "promoting",
  "waiting",
  "complete",
  "failed",
  "stopped",
];

function asRunState(value: string): RunState {
  return validStates.includes(value as RunState) ? (value as RunState) : "working";
}

function initials(kind?: AgentKind): string {
  if (!kind) return "AR";
  return { codex: "CX", claude: "CL", cursor: "CU", antigravity: "AG" }[kind];
}

function relativeTime(timestamp: string): string {
  const normalized = timestamp.includes("T") ? timestamp : `${timestamp.replace(" ", "T")}Z`;
  const minutes = Math.max(0, Math.round((Date.now() - new Date(normalized).getTime()) / 60_000));
  if (minutes < 1) return "now";
  if (minutes < 60) return `${minutes}m`;
  return `${Math.floor(minutes / 60)}h`;
}

function contextLabel(bytes = 0): string {
  if (!bytes) return "Packet not assembled";
  return `${Math.max(1, Math.round(bytes / 1024))} KiB / 48 KiB`;
}

function usageLabel(receipt: ExecutionReceipt): string {
  const { inputTokens, cachedInputTokens, outputTokens, totalCostUsd, numTurns } = receipt.usage;
  const parts = [
    inputTokens !== undefined ? `${inputTokens.toLocaleString()} in` : undefined,
    cachedInputTokens !== undefined ? `${cachedInputTokens.toLocaleString()} cached` : undefined,
    outputTokens !== undefined ? `${outputTokens.toLocaleString()} out` : undefined,
    totalCostUsd !== undefined ? `$${totalCostUsd.toFixed(4)}` : undefined,
    numTurns !== undefined ? `${numTurns} turns` : undefined,
  ].filter(Boolean);
  return parts.length ? parts.join(" · ") : "Not reported";
}

function latencyLabel(receipt: ExecutionReceipt): string | undefined {
  if (receipt.totalMs === undefined) return undefined;
  const total = (receipt.totalMs / 1000).toFixed(2);
  const first = receipt.firstOutputMs === undefined
    ? "first output unavailable"
    : `${(receipt.firstOutputMs / 1000).toFixed(2)}s to first output`;
  const preflight = receipt.preflightMs === undefined
    ? undefined
    : `${(receipt.preflightMs / 1000).toFixed(2)}s preflight`;
  return [`${total}s total`, first, preflight].filter(Boolean).join(" Â· ");
}

function millisecondsLabel(value: number | undefined): string {
  return value === undefined ? "—" : `${value.toLocaleString()} ms`;
}

function autonomyLabel(mode: Participant["capabilities"]["autonomyMode"]): string {
  return {
    "isolated-auto": "Isolated auto",
    "reviewed-auto": "Reviewed auto",
    "unattended-bypass": "Sandbox bypass",
    manual: "Manual",
    unavailable: "Unavailable",
  }[mode];
}

function connectionLabel(status: Participant["connectionStatus"]): string {
  return {
    connected: "Connected",
    "sign-in-required": "Sign in required",
    unverified: "Test connection",
    "not-installed": "Not installed",
    failed: "Connection failed",
  }[status];
}

function phaseFromStoredRun(run: StoredRun): string {
  if (run.state === "complete") return "complete";
  if (run.state === "verifying") return "verify";
  if (run.state === "revising") return "revise";
  if (run.state === "promoting") return "promote";
  if (run.state === "reviewing") return run.reviewCount > 1 ? "final-review" : "review";
  return "build";
}

function hydrateRun(stored: StoredRun): Run {
  const route = routeForPhase(phaseFromStoredRun(stored), stored.writer, stored.reviewer);
  if (stored.state === "complete") {
    route.forEach((step) => {
      step.state = "complete";
    });
  }
  return {
    id: stored.id,
    objective: stored.objective,
    state: stored.state,
    currentOwner: stored.currentOwner,
    route,
    reviewCount: stored.reviewCount,
    revisionCount: stored.revisionCount,
    startedAt: stored.startedAt,
    stopReason: stored.stopReason,
    nativeSessionId: stored.nativeSessionId,
    writer: stored.writer,
    reviewer: stored.reviewer,
    degradedReview: stored.degradedReview,
    worktreePath: stored.worktreePath,
    branch: stored.branch,
    contextBytes: stored.contextBytes,
    artifactPath: stored.artifactPath,
    instructionFiles: stored.instructionFiles,
    skillFiles: stored.skillFiles,
    recoveryCount: stored.recoveryCount,
  };
}

function ParticipantMark({ participant }: { participant: Participant }) {
  return (
    <span className={`participant-mark participant-${participant.kind}`} aria-hidden="true">
      {initials(participant.kind)}
    </span>
  );
}

function RunLens({
  run,
  participants,
  activity,
  streamTitle,
  onStop,
  onResume,
}: {
  run: Run;
  participants: Participant[];
  activity: LiveActivityItem[];
  streamTitle: string;
  onStop: () => void;
  onResume: () => void;
}) {
  const active = run.currentOwner
    ? participants.find((participant) => participant.kind === run.currentOwner)
    : undefined;
  const running = activeStates.includes(run.state);
  const autonomy = active?.capabilities.autonomyMode;
  const recoverable =
    Boolean(run.worktreePath) &&
    ["waiting", "failed", "stopped"].includes(run.state) &&
    (run.recoveryCount ?? 0) < 2;

  return (
    <article
      className={`timeline-entry entry-run-progress state-${run.state}`}
      aria-label="Current Ship run"
      aria-live="polite"
    >
      <div className="entry-rail">
        <span className="entry-dot progress-dot">
          <Workflow size={13} />
        </span>
        <span className="entry-line" />
      </div>
      <section className="entry-content run-progress">
        <header>
          <span className="state-pulse" />
          <strong>Agent Room / Ship</strong>
          <span>{run.state.replace("-", " ")}</span>
        </header>
        <div className="progress-summary">
          <strong>
            {run.state === "waiting"
              ? "Your attention is needed"
              : active
                ? `${active.name} is handling ${run.route.find((step) => step.state === "current")?.label ?? "this phase"}`
                : run.state === "complete"
                  ? "Verified workflow complete"
                  : "Ship workflow"}
          </strong>
          <p>{run.stopReason ?? run.objective}</p>
        </div>

        <div className="progress-route" aria-label="Ship stages">
          {run.route.map((step, index) => (
            <div className={`progress-step ${step.state}`} key={`${step.label}-${index}`}>
              <span>{step.state === "complete" ? <Check size={11} /> : step.agent ? initials(step.agent) : index + 1}</span>
              <small>{step.label}</small>
              {index < run.route.length - 1 && <ArrowRight size={12} aria-hidden="true" />}
            </div>
          ))}
        </div>

        {activity.length > 0 && (
          <div className="progress-activity" role="log" aria-live="polite" aria-relevant="additions text">
            <div className="progress-live-label">
              <span className="stream-pulse" />
              <strong>{streamTitle}</strong>
              <span>live</span>
            </div>
            <div className="activity-list">
              {activity.slice(-4).map((item, index) => (
                <div className="activity-item" key={`${item.title}-${index}`}>
                  <strong>{item.title}</strong>
                  <p>{item.detail}</p>
                </div>
              ))}
            </div>
          </div>
        )}

        <div className="progress-footer">
          <div className="progress-meta">
            <span>
              <ShieldCheck size={12} />
              {run.degradedReview ? "Same-provider review" : run.reviewer ? "Independent review" : "Reviewer selected at run time"}
            </span>
            <span>
              <Gauge size={12} />
              {contextLabel(run.contextBytes)}
            </span>
            <span>
              <Workflow size={12} />
              {autonomy ? autonomyLabel(autonomy) : "Bounded autonomous route"}
            </span>
          </div>
          <div className="progress-action">
            {running ? (
              <button type="button" className="danger-button" onClick={onStop}>
                <CircleStop size={15} />
                Stop
              </button>
            ) : recoverable ? (
              <button type="button" className="send-button recovery-button" onClick={onResume}>
                <History size={14} />
                Resume recovery
              </button>
            ) : (
              <span className="progress-limits">
                {run.revisionCount}/1 revisions · {run.reviewCount}/2 reviews · {run.recoveryCount ?? 0}/2 recoveries
              </span>
            )}
          </div>
        </div>
      </section>
    </article>
  );
}

function VerificationList({ verification }: { verification: VerificationResult[] }) {
  if (!verification.length) return null;
  return (
    <div className="verification-list">
      {verification.map((result) => (
        <div className={`verification-row verification-${result.status}`} key={result.label}>
          {result.status === "passed" ? <Check size={13} /> : <CircleAlert size={13} />}
          <span>
            <strong>{result.label}</strong>
            <small>{result.status}</small>
          </span>
        </div>
      ))}
    </div>
  );
}

function TimelineEntry({ message }: { message: RoomMessage }) {
  const isHuman = message.kind === "human";
  const reason = message.reason;
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
        <p>{message.body}</p>
        {reason && (
          <div className="reason-line">
            <Braces size={14} />
            <span>{reason}</span>
          </div>
        )}
        {message.changedFiles?.length ? (
          <div className="file-list">
            {message.changedFiles.map((file) => (
              <code key={file}>{file}</code>
            ))}
          </div>
        ) : null}
        <VerificationList verification={message.verification ?? []} />
      </div>
    </article>
  );
}

function LiveActivity({
  title,
  activity,
}: {
  title: string;
  activity: LiveActivityItem[];
}) {
  return (
    <article
      className="timeline-entry entry-activity"
      role="log"
      aria-live="polite"
      aria-relevant="additions text"
    >
      <div className="entry-rail">
        <span className="entry-dot activity-dot">
          <Bot size={13} />
        </span>
        <span className="entry-line" />
      </div>
      <div className="entry-content activity-content">
        <header>
          <span className="stream-pulse" />
          <strong>{title}</strong>
          <span>live</span>
        </header>
        <div className="activity-list">
          {activity.slice(-6).map((item, index) => (
            <div className="activity-item" key={`${item.title}-${index}`}>
              <strong>{item.title}</strong>
              <p>{item.detail}</p>
            </div>
          ))}
        </div>
        <small className="activity-note">
          Provider-reported reasoning and tool activity only. Full output remains in the local run log.
        </small>
      </div>
    </article>
  );
}

function WindowControls({ native }: { native: boolean }) {
  return (
    <div className="window-controls" role="group" aria-label="Window controls">
      <button
        type="button"
        className="window-control"
        aria-label="Minimize window"
        disabled={!native}
        title={native ? "Minimize window" : "Inert in browser preview mode"}
        onClick={() => {
          if (native) void getCurrentWindow().minimize();
        }}
      >
        <Minus size={14} strokeWidth={1.7} />
      </button>
      <button
        type="button"
        className="window-control"
        aria-label="Maximize or restore window"
        disabled={!native}
        title={native ? "Maximize or restore window" : "Inert in browser preview mode"}
        onClick={() => {
          if (native) void getCurrentWindow().toggleMaximize();
        }}
      >
        <Square size={11} strokeWidth={1.7} />
      </button>
      <button
        type="button"
        className="window-control window-close"
        aria-label="Close Agent Room"
        disabled={!native}
        title={native ? "Close Agent Room" : "Inert in browser preview mode"}
        onClick={() => {
          if (native) void getCurrentWindow().close();
        }}
      >
        <X size={14} strokeWidth={1.7} />
      </button>
    </div>
  );
}

function ProjectRail({
  activeView,
  onNavigate,
}: {
  activeView: PrimaryView;
  onNavigate: (view: PrimaryView) => void;
}) {
  return (
    <aside className="project-rail">
      <nav className="primary-nav" aria-label="Primary">
        <button
          type="button"
          className={`nav-button ${activeView === "rooms" ? "active" : ""}`}
          aria-current={activeView === "rooms" ? "page" : undefined}
          onClick={() => onNavigate("rooms")}
        >
          <TerminalSquare size={19} />
          <span>Rooms</span>
        </button>
        <button
          type="button"
          className={`nav-button ${activeView === "activity" ? "active" : ""}`}
          aria-current={activeView === "activity" ? "page" : undefined}
          onClick={() => onNavigate("activity")}
        >
          <Activity size={19} />
          <span>Activity</span>
        </button>
        <button
          type="button"
          className={`nav-button ${activeView === "settings" ? "active" : ""}`}
          aria-current={activeView === "settings" ? "page" : undefined}
          onClick={() => onNavigate("settings")}
        >
          <Settings size={19} />
          <span>Settings</span>
        </button>
      </nav>
      <div className="rail-foot">
        <span className="local-indicator" />
        <span>Local</span>
      </div>
    </aside>
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

function ProviderProfileCard({
  participant,
  profiles,
  saving,
  onSave,
  testing,
  onTest,
  models,
  discoveringModels,
  modelDiscoveryDetail,
  onRefreshModels,
}: {
  participant: Participant;
  profiles: ProviderProfile[];
  saving: boolean;
  onSave: (profiles: ProviderProfile[]) => Promise<void>;
  testing: boolean;
  onTest: (kind: AgentKind) => void;
  models: string[];
  discoveringModels: boolean;
  modelDiscoveryDetail?: string;
  onRefreshModels: (kind: AgentKind) => void;
}) {
  const routes = ["chat", "build", "review"] as const;
  const [drafts, setDrafts] = useState<Record<(typeof routes)[number], {
    model: string;
    effort: string;
  }>>({
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
      <div className="route-profile-list">
        {routes.map((route) => (
          <div className="route-profile-row" key={route}>
            <strong>{route === "chat" ? "Chat" : route === "build" ? "Builder" : "Reviewer"}</strong>
            <label htmlFor={`model-${participant.kind}-${route}`}>
              <span>Model</span>
              <select
                id={`model-${participant.kind}-${route}`}
                value={drafts[route].model}
                onChange={(event) => setDrafts((current) => ({
                  ...current,
                  [route]: { ...current[route], model: event.target.value },
                }))}
                disabled={!participant.installed || saving}
              >
                <option value="">Default</option>
                {drafts[route].model && !models.includes(drafts[route].model) && (
                  <option value={drafts[route].model}>{drafts[route].model}</option>
                )}
                {models.map((option) => <option key={option} value={option}>{option}</option>)}
              </select>
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
                  || (participant.kind === "cursor" && !drafts[route].model)
                }
                title={
                  participant.kind === "cursor" && !drafts[route].model
                    ? "Choose a Cursor model before setting effort."
                    : undefined
                }
              >
                <option value="">Default</option>
                {participant.effortOptions.map((option) => (
                  <option key={option} value={option}>
                    {option[0].toUpperCase() + option.slice(1)}
                  </option>
                ))}
              </select>
            </label>
          </div>
        ))}
      </div>
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
          onClick={() => onTest(participant.kind)}
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
}: {
  environment: NativeEnvironment;
  profiles: ProviderProfile[];
  projectSettings: ProjectSettings;
  savingProjectSettings: boolean;
  savingKind?: AgentKind;
  refreshing: boolean;
  native: boolean;
  error: string;
  onRefresh: () => void;
  onSaveProfile: (profiles: ProviderProfile[]) => Promise<void>;
  testingKind?: AgentKind;
  onTestConnection: (kind: AgentKind) => void;
  modelCatalog: Partial<Record<AgentKind, string[]>>;
  discoveringModelsKind?: AgentKind;
  modelDiscoveryDetails: Partial<Record<AgentKind, string>>;
  onRefreshModels: (kind: AgentKind) => void;
  onAutonomousShipChange: (enabled: boolean) => void;
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
                  {autonomyLabel(participant.capabilities.autonomyMode)}
                </span>
              </div>
              <p>{participant.capabilities.autonomyNote}</p>
              <div className="capability-line">
                <span>{participant.capabilities.exactResume ? "Resume" : "Fresh session"}</span>
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
            <strong>{contextLabel(run.contextBytes)}</strong>
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
  const [project, setProject] = useState(seedProject);
  const [environment, setEnvironment] = useState(previewEnvironment);
  const [messages, setMessages] = useState<RoomMessage[]>(seedMessages);
  const [receipts, setReceipts] = useState<ExecutionReceipt[]>([]);
  const [profiles, setProfiles] = useState<ProviderProfile[]>([]);
  const [projectSettings, setProjectSettings] = useState<ProjectSettings>({
    autonomousShipEnabled: false,
  });
  const [savingProjectSettings, setSavingProjectSettings] = useState(false);
  const [modelCatalog, setModelCatalog] = useState<Partial<Record<AgentKind, string[]>>>({});
  const [modelDiscoveryDetails, setModelDiscoveryDetails] = useState<Partial<Record<AgentKind, string>>>({});
  const [discoveringModelsKind, setDiscoveringModelsKind] = useState<AgentKind>();
  const [run, setRun] = useState<Run>(seedRun);
  const [objective, setObjective] = useState("");
  const [composerMode, setComposerMode] = useState<ComposerMode>("chat");
  const [chatSending, setChatSending] = useState(false);
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
  const timelineRef = useRef<HTMLDivElement>(null);
  const runRef = useRef(run);
  const chatRunRef = useRef<string | undefined>(undefined);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const contextToggleRef = useRef<HTMLButtonElement>(null);
  const native = isNativeApp();

  useEffect(() => {
    runRef.current = run;
  }, [run]);

  async function refreshRoom(projectId = project.id) {
    if (!native) return;
    const snapshot = await loadRoom(projectId);
    if (snapshot.messages.length) setMessages(snapshot.messages);
    if (snapshot.latestRun) setRun(hydrateRun(snapshot.latestRun));
    setReceipts(snapshot.receipts);
  }

  useEffect(() => {
    if (!native) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;

    getEnvironment()
      .then(async (nextEnvironment) => {
        if (disposed) return;
        const nextProject = {
          ...seedProject,
          repositoryPath: nextEnvironment.repositoryPath,
          branch: nextEnvironment.branch,
        };
        setEnvironment(nextEnvironment);
        setProject(nextProject);
        await saveProject(nextProject);
        const [_, nextProfiles, nextSettings] = await Promise.all([
          refreshRoom(nextProject.id),
          loadProviderProfiles(nextProject.id),
          loadProjectSettings(nextProject.id),
        ]);
        if (!disposed) {
          setProfiles(nextProfiles);
          setProjectSettings(nextSettings);
        }
      })
      .catch((error) => console.error("Failed to inspect native environment", error));

    onRunEvent((event: RunEvent) => {
      if (event.eventType === "stream") {
        if (
          (event.phase === "chat" && event.runId !== chatRunRef.current) ||
          (event.phase !== "chat" && event.runId !== runRef.current.id)
        ) {
          return;
        }
        setStreamTitle(event.agent ? `${agentNames[event.agent]} / ${event.phase}` : event.title);
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
      setRun((current) => {
        if (current.id !== event.runId) return current;
        const writer = current.writer ?? (event.phase === "build" ? event.agent : undefined);
        const reviewer =
          current.reviewer ?? (["review", "final-review"].includes(event.phase) ? event.agent : undefined);
        return {
          ...current,
          state: asRunState(event.state),
          currentOwner: event.agent,
          writer,
          reviewer,
          contextBytes: Math.max(current.contextBytes ?? 0, event.contextBytes ?? 0),
          stopReason: event.eventType === "attention" ? event.detail : current.stopReason,
          route: routeForPhase(event.phase, writer, reviewer),
        };
      });
    }).then((dispose) => {
      if (disposed) dispose();
      else unlisten = dispose;
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [native]);

  useEffect(() => {
    timelineRef.current?.scrollTo({
      top: timelineRef.current.scrollHeight,
      behavior: "smooth",
    });
  }, [messages, activity]);

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
    () => environment.participants.filter(participantIsRunnable).length,
    [environment],
  );
  const activeShipAgent = run.currentOwner
    ?? (run.state === "reviewing" ? run.reviewer : run.writer)
    ?? run.reviewer;
  const sideChatAvailable = activeStates.includes(run.state) && run.state !== "promoting";

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

  async function handleTestConnection(kind: AgentKind) {
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
    } catch (error) {
      setUiError(error instanceof Error ? error.message : String(error));
    } finally {
      setDiscoveringModelsKind(undefined);
    }
  }

  async function runShipObjective(
    text: string,
    requestedWriter?: AgentKind,
    recordHumanMessage = true,
  ) {
    setUiError("");

    const nextRun = createRun(text, environment.participants);
    const requestedParticipant = environment.participants.find(
      (participant) =>
        participant.kind === requestedWriter && participantIsRunnable(participant),
    );
    const writer = requestedParticipant?.kind ?? nextRun.currentOwner;
    const reviewer =
      environment.participants.find(
        (participant) => participantIsRunnable(participant) && participant.kind !== writer,
      )?.kind ?? writer;
    nextRun.writer = writer;
    nextRun.currentOwner = writer;
    nextRun.reviewer = reviewer;
    nextRun.degradedReview = Boolean(writer && reviewer === writer);
    nextRun.route = routeForPhase("build", writer, reviewer);
    setRun(nextRun);
    setComposerMode("chat");
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

    if (!writer) {
      setMessages((current) => [
        ...current,
        {
          id: crypto.randomUUID(),
          kind: "decision",
          sender: "system",
          body: nextRun.stopReason ?? "Choose an available participant.",
          createdAt: new Date().toISOString(),
          runId: nextRun.id,
        },
      ]);
      return;
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
      : selectChatParticipant(text, environment.participants, priorAgent);
    const chatRunId = crypto.randomUUID();
    setUiError("");
    setObjective("");
    setActivity([]);
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
        setChatSending(false);
        chatRunRef.current = undefined;
        setComposerMode("ship");
        await runShipObjective(
          result.shipIntent.objective,
          participant,
          false,
        );
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

  async function submitComposer(event: FormEvent) {
    event.preventDefault();
    if (sideChatAvailable) await submitChat(true);
    else if (composerMode === "chat") await submitChat();
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
        await stopRun(runRef.current.id);
      } catch (error) {
        setUiError(error instanceof Error ? error.message : String(error));
        return;
      }
    }
    setRun((current) => ({
      ...current,
      state: "stopped",
      stopReason: "Stop requested. Agent Room is preserving recoverable work.",
    }));
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
    setComposerMode("chat");
    setRun((current) => ({
      ...current,
      state: "working",
      currentOwner: current.writer,
      stopReason: undefined,
      recoveryCount: (current.recoveryCount ?? 0) + 1,
      route: routeForPhase("build", current.writer, current.reviewer),
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
    <div className="app-shell">
      <a className="skip-link" href="#room-main">
        Skip to room
      </a>
      <header className="topbar">
        <div className="brand">
          <span className="brand-mark">
            <span />
          </span>
          <strong>Agent Room</strong>
        </div>
        <button
          type="button"
          className="project-switch"
          onClick={() => {
            setActiveView("rooms");
            openInspector("Repository");
          }}
          aria-label={`Open ${project.name} repository details`}
        >
          <span className="project-monogram small">AR</span>
          <span>
            <strong>{project.name}</strong>
            <small>{project.branch}</small>
          </span>
          <ChevronRight size={15} />
        </button>
        {searchOpen ? (
          <div className="search-field" role="search">
            <Search size={16} aria-hidden="true" />
            <input
              ref={searchInputRef}
              value={searchQuery}
              onChange={(event) => setSearchQuery(event.target.value)}
              placeholder="Search rooms and evidence"
              aria-label="Search rooms and evidence"
            />
            <button
              type="button"
              className="search-close"
              onClick={() => {
                setSearchOpen(false);
                setSearchQuery("");
              }}
              aria-label="Close search"
            >
              <X size={15} />
            </button>
          </div>
        ) : (
          <button
            type="button"
            className="search-button"
            onClick={() => {
              setSearchOpen(true);
              requestAnimationFrame(() => searchInputRef.current?.focus());
            }}
          >
            <Search size={16} />
            <span>Search rooms and evidence</span>
            <kbd>Ctrl K</kbd>
          </button>
        )}
        <div
          className="titlebar-drag"
          data-tauri-drag-region
          onDoubleClick={() => {
            if (native) void getCurrentWindow().toggleMaximize();
          }}
          aria-hidden="true"
        />
        <div className="topbar-status">
          <span className={native ? "online" : ""} />
          {native
            ? projectSettings.autonomousShipEnabled
              ? "Autonomy armed"
              : "Local runtime"
            : "Read-only preview"}
        </div>
        <button
          ref={contextToggleRef}
          type="button"
          className="icon-button context-toggle"
          aria-label={inspectorOpen ? "Close context" : "Open context"}
          aria-expanded={inspectorOpen}
          aria-controls="room-context"
          onClick={() => setInspectorOpen((open) => !open)}
        >
          <PanelRight size={18} />
        </button>
        <WindowControls native={native} />
      </header>

      <ProjectRail
        activeView={activeView}
        onNavigate={showView}
      />

      <main className="room" id="room-main" tabIndex={-1}>
        {activeView === "activity" ? (
          <ActivityView messages={visibleMessages} query={searchQuery} />
        ) : activeView === "settings" ? (
          <SettingsView
            environment={environment}
            profiles={profiles}
            projectSettings={projectSettings}
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

        <div className="timeline" ref={timelineRef} role="feed" aria-label="Room timeline">
          <div className="timeline-date">
            <span>Durable room</span>
          </div>
          {visibleMessages.map((message) => (
            <TimelineEntry key={message.id} message={message} />
          ))}
          {searchQuery && !visibleMessages.length && (
            <p className="empty-state">No room activity matches this search.</p>
          )}
          {run.state !== "ready" && !searchQuery && (
            <RunLens
              run={run}
              participants={environment.participants}
              activity={activity}
              streamTitle={streamTitle}
              onStop={handleStop}
              onResume={handleResume}
            />
          )}
          {run.state === "ready" && activity.length > 0 && (
            <LiveActivity title={streamTitle} activity={activity} />
          )}
        </div>

        <form className="composer" onSubmit={submitComposer}>
          <div className="composer-context">
            <label className="composer-label" htmlFor="room-objective">
              {composerMode === "chat"
                ? sideChatAvailable
                  ? "Ask about active Ship run"
                  : "Project chat and quick edits"
                : run.state === "promoting"
                  ? "Promoting verified work"
                  : sideChatAvailable
                  ? "Ask about active Ship run"
                  : "Engineering objective"}
            </label>
            <div className="composer-mode" role="group" aria-label="Message route">
              <button
                type="button"
                className={composerMode === "chat" ? "active" : ""}
                onClick={() => setComposerMode("chat")}
                aria-pressed={composerMode === "chat"}
              >
                Chat
              </button>
              <button
                type="button"
                className={composerMode === "ship" ? "active" : ""}
                onClick={() => setComposerMode("ship")}
                aria-pressed={composerMode === "ship"}
                disabled={sideChatAvailable || run.state === "promoting"}
              >
                Ship
              </button>
            </div>
          </div>
          <textarea
            id="room-objective"
            value={objective}
            onChange={(event) => setObjective(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" && !event.shiftKey) {
                event.preventDefault();
                event.currentTarget.form?.requestSubmit();
              }
            }}
            placeholder={
              composerMode === "chat"
                ? sideChatAvailable
                  ? `Ask ${activeShipAgent ? agentNames[activeShipAgent] : "the active agent"} what is happening...`
                  : "Ask a question or request a quick edit..."
                : run.state === "promoting"
                  ? "Promotion is finishing safely..."
                  : sideChatAvailable
                  ? `Ask ${activeShipAgent ? agentNames[activeShipAgent] : "the active agent"} what is happening...`
                  : "@codex State the objective once..."
            }
            aria-describedby={uiError ? "composer-error" : undefined}
            rows={2}
          />
          <div className="composer-actions">
            <div className="mention-list">
              {environment.participants.map((participant) => (
                <button
                  type="button"
                  key={participant.kind}
                  disabled={
                    sideChatAvailable
                      ? participant.kind !== activeShipAgent
                      : composerMode === "chat"
                      ? !participantCanChat(participant)
                      : run.state === "promoting"
                        ? true
                        : sideChatAvailable
                        ? participant.kind !== activeShipAgent
                        : !participantIsRunnable(participant)
                  }
                  onClick={() =>
                    setObjective(
                      (current) => `@${participant.kind} ${current.replace(/^@\w+\s*/, "")}`,
                    )
                  }
                  title={
                    (composerMode === "chat" ? participantCanChat(participant) : participantIsRunnable(participant))
                      ? `${autonomyLabel(participant.capabilities.autonomyMode)}: ${participant.capabilities.autonomyNote}`
                      : participant.capabilities.autonomyNote
                  }
                >
                  <ParticipantMark participant={participant} />
                  <span>{participant.name}</span>
                </button>
              ))}
            </div>
            {chatSending ? (
              <button type="button" className="danger-button chat-stop" onClick={handleStopChat}>
                <CircleStop size={15} />
                Stop response
              </button>
            ) : (
              <button
                type="submit"
                className="send-button"
                disabled={!objective.trim() || (composerMode === "ship" && run.state === "promoting")}
                aria-busy={composerMode === "chat" ? chatSending : false}
              >
                {composerMode === "chat" ? <Sparkles size={15} /> : <Play size={15} fill="currentColor" />}
                {composerMode === "chat"
                  ? sideChatAvailable
                    ? "Ask active run"
                    : "Send"
                  : run.state === "promoting"
                    ? "Promoting"
                    : sideChatAvailable
                    ? "Ask active run"
                    : "Run autonomously"}
                <kbd>Enter</kbd>
              </button>
            )}
          </div>
          {uiError && (
            <p className="composer-error" id="composer-error" role="alert">
              {uiError}
            </p>
          )}
        </form>
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
