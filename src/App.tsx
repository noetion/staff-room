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
  PanelRight,
  Play,
  RefreshCw,
  Search,
  Settings,
  ShieldCheck,
  Sparkles,
  TerminalSquare,
  Workflow,
  X,
} from "lucide-react";
import { FormEvent, useEffect, useMemo, useRef, useState } from "react";
import { createRun, participantIsRunnable, routeForPhase } from "./coordination";
import type {
  AgentKind,
  ExecutionReceipt,
  NativeEnvironment,
  Participant,
  ProviderProfile,
  Project,
  RoomMessage,
  Run,
  RunState,
  StoredRun,
  VerificationResult,
} from "./model";
import { agentNames } from "./model";
import {
  getEnvironment,
  isNativeApp,
  loadProviderProfiles,
  loadRoom,
  onRunEvent,
  saveProject,
  saveProviderProfile,
  startRoomRun,
  stopRun,
  type RunEvent,
} from "./native";
import { previewEnvironment, seedMessages, seedProject, seedRun } from "./seed";

type InspectorTab = "Repository" | "Participants" | "Evidence" | "Memory";
type PrimaryView = "rooms" | "activity" | "settings";

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

function autonomyLabel(mode: Participant["capabilities"]["autonomyMode"]): string {
  return {
    "isolated-auto": "Isolated auto",
    "reviewed-auto": "Reviewed auto",
    "unattended-bypass": "Sandbox bypass",
    manual: "Manual",
    unavailable: "Unavailable",
  }[mode];
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
  onStop,
  onResume,
}: {
  run: Run;
  participants: Participant[];
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
    <section className={`run-lens state-${run.state}`} aria-label="Current run" aria-live="polite">
      <div className="lens-glow" aria-hidden="true" />
      <div className="lens-copy">
        <span className="eyebrow">
          <span className="state-pulse" />
          {run.state === "ready" ? "Room ready" : run.state.replace("-", " ")}
        </span>
        <strong>
          {run.state === "waiting"
            ? "Your attention is needed"
            : active
              ? `${active.name} owns this phase`
              : run.state === "complete"
                ? "Verified handoff complete"
                : "State one objective"}
        </strong>
        <span className="lens-objective">{run.stopReason ?? run.objective}</span>
      </div>

      <div className="route-shell">
        <span className="handoff-beam" aria-hidden="true" />
        <div className="route" aria-label="Autonomous route">
          {run.route.length ? (
            run.route.map((step, index) => (
              <div className={`route-step ${step.state}`} key={`${step.label}-${index}`}>
                <span>{step.agent ? initials(step.agent) : index + 1}</span>
                <small>{step.label}</small>
                {index < run.route.length - 1 && <ArrowRight size={13} aria-hidden="true" />}
              </div>
            ))
          ) : (
            <div className="route-placeholder">Build / verify / review / promote</div>
          )}
        </div>
      </div>

      <div className="lens-action">
        {running ? (
          <button type="button" className="danger-button" onClick={onStop}>
            <CircleStop size={16} />
            Stop
          </button>
        ) : recoverable ? (
          <button type="button" className="send-button recovery-button" onClick={onResume}>
            <History size={15} />
            Resume recovery
          </button>
        ) : (
          <div className="limit-readout">
            <span>{run.revisionCount}/1 revisions</span>
            <span>{run.reviewCount}/2 reviews</span>
            <span>{run.recoveryCount ?? 0}/2 recoveries</span>
          </div>
        )}
      </div>

      <div className="lens-facts">
        <span>
          <ShieldCheck size={13} />
          {run.degradedReview ? "Same-provider review" : run.reviewer ? "Independent review" : "Reviewer selected at run time"}
        </span>
        <span>
          <Gauge size={13} />
          {contextLabel(run.contextBytes)}
        </span>
        <span>
          <Workflow size={13} />
          {autonomy ? autonomyLabel(autonomy) : "Bounded autonomous route"}
        </span>
      </div>
    </section>
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
        {message.reason && (
          <div className="reason-line">
            <Braces size={14} />
            <span>{message.reason}</span>
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

function ProjectRail({
  project,
  run,
  installedCount,
  activeView,
  onNavigate,
  onOpenProject,
}: {
  project: Project;
  run: Run;
  installedCount: number;
  activeView: PrimaryView;
  onNavigate: (view: PrimaryView) => void;
  onOpenProject: () => void;
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
      <div className="rail-section">
        <span className="rail-label">Project room</span>
        <button type="button" className="project-row selected" onClick={onOpenProject}>
          <span className="project-monogram">AR</span>
          <span>
            <strong>{project.name}</strong>
            <small>
              {run.state} / {installedCount} agents ready
            </small>
          </span>
          <ChevronRight size={15} />
        </button>
      </div>
      <div className="rail-policy">
        <ShieldCheck size={15} />
        <span>
          <strong>Autonomous policy</strong>
          <small>Interrupts only when policy cannot continue safely.</small>
        </span>
      </div>
      <div className="rail-foot">
        <span className="local-indicator" />
        <span>
          <strong>Local orchestration</strong>
          <small>SQLite and managed Git worktrees</small>
        </span>
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
  profile,
  saving,
  onSave,
}: {
  participant: Participant;
  profile?: ProviderProfile;
  saving: boolean;
  onSave: (profile: ProviderProfile) => void;
}) {
  const [model, setModel] = useState(profile?.model ?? "");
  const [effort, setEffort] = useState(profile?.effort ?? "");

  useEffect(() => {
    setModel(profile?.model ?? "");
    setEffort(profile?.effort ?? "");
  }, [profile?.model, profile?.effort]);

  return (
    <article className="participant-card provider-profile-card">
      <div className="participant-row">
        <ParticipantMark participant={participant} />
        <span className="participant-copy">
          <strong>{participant.name}</strong>
          <small>{participant.installed ? participant.version ?? "Installed" : "Not installed"}</small>
        </span>
      </div>
      <p>{participant.capabilities.autonomyNote}</p>
      <div className="provider-fields">
        <label htmlFor={`model-${participant.kind}`}>
          Model
          <input
            id={`model-${participant.kind}`}
            list={`models-${participant.kind}`}
            value={model}
            onChange={(event) => setModel(event.target.value)}
            placeholder="Provider default"
            disabled={!participant.installed || saving}
          />
          <datalist id={`models-${participant.kind}`}>
            {participant.models.map((option) => <option key={option} value={option} />)}
          </datalist>
        </label>
        {participant.supportsEffort && (
          <label htmlFor={`effort-${participant.kind}`}>
            Reasoning effort
            <select
              id={`effort-${participant.kind}`}
              value={effort}
              onChange={(event) => setEffort(event.target.value)}
              disabled={!participant.installed || saving}
            >
              <option value="">Provider default</option>
              <option value="low">Low</option>
              <option value="medium">Medium</option>
              <option value="high">High</option>
            </select>
          </label>
        )}
      </div>
      <small className="model-discovery-note">{participant.modelDiscoveryNote}</small>
      <button
        type="button"
        className="secondary-button save-profile-button"
        disabled={!participant.installed || saving}
        onClick={() => onSave({
          participantKind: participant.kind,
          model: model.trim() || undefined,
          effort: effort || undefined,
        })}
      >
        {saving ? "Saving model" : "Save model"}
      </button>
    </article>
  );
}

function SettingsView({
  environment,
  profiles,
  savingKind,
  refreshing,
  native,
  error,
  onRefresh,
  onSaveProfile,
}: {
  environment: NativeEnvironment;
  profiles: ProviderProfile[];
  savingKind?: AgentKind;
  refreshing: boolean;
  native: boolean;
  error: string;
  onRefresh: () => void;
  onSaveProfile: (profile: ProviderProfile) => void;
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
      <div className="settings-grid">
        {environment.participants.map((participant) => (
          <ProviderProfileCard
            key={participant.kind}
            participant={participant}
            profile={profiles.find((profile) => profile.participantKind === participant.kind)}
            saving={savingKind === participant.kind}
            onSave={onSaveProfile}
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
            {receipts.length ? receipts.map((receipt) => (
              <article className="receipt-card" key={receipt.id}>
                <div>
                  <strong>{receipt.phase} · {agentNames[receipt.participant]}</strong>
                  <small>{receipt.actualModel ?? receipt.requestedModel ?? "Provider default"}{receipt.requestedEffort ? ` · ${receipt.requestedEffort} effort` : ""}</small>
                </div>
                <p>{usageLabel(receipt)}</p>
                <small>{contextLabel(receipt.contextBytes)} sent. {receipt.usageNote}</small>
              </article>
            )) : (
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
  const [run, setRun] = useState<Run>(seedRun);
  const [objective, setObjective] = useState("");
  const [stream, setStream] = useState("");
  const [streamTitle, setStreamTitle] = useState("Provider events");
  const [activeView, setActiveView] = useState<PrimaryView>("rooms");
  const [activeTab, setActiveTab] = useState<InspectorTab>("Repository");
  const [inspectorOpen, setInspectorOpen] = useState(false);
  const [searchOpen, setSearchOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [refreshingProviders, setRefreshingProviders] = useState(false);
  const [savingProfileKind, setSavingProfileKind] = useState<AgentKind>();
  const [uiError, setUiError] = useState("");
  const timelineRef = useRef<HTMLDivElement>(null);
  const runRef = useRef(run);
  const searchInputRef = useRef<HTMLInputElement>(null);
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
        const [_, nextProfiles] = await Promise.all([
          refreshRoom(nextProject.id),
          loadProviderProfiles(nextProject.id),
        ]);
        if (!disposed) setProfiles(nextProfiles);
      })
      .catch((error) => console.error("Failed to inspect native environment", error));

    onRunEvent((event: RunEvent) => {
      if (event.eventType === "stream") {
        setStreamTitle(event.agent ? `${agentNames[event.agent]} / ${event.phase}` : event.title);
        setStream((current) => `${current}${event.detail}\n`.slice(-12_000));
        return;
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
      unlisten = dispose;
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
  }, [messages, stream]);

  useEffect(() => {
    function handleShortcut(event: KeyboardEvent) {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setSearchOpen(true);
        requestAnimationFrame(() => searchInputRef.current?.focus());
      }
      if (event.key === "Escape" && searchOpen) {
        setSearchOpen(false);
        setSearchQuery("");
      }
    }
    window.addEventListener("keydown", handleShortcut);
    return () => window.removeEventListener("keydown", handleShortcut);
  }, [searchOpen]);

  const installedCount = useMemo(
    () => environment.participants.filter(participantIsRunnable).length,
    [environment],
  );

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
      const nextEnvironment = await getEnvironment();
      setEnvironment(nextEnvironment);
      setProject((current) => ({ ...current, branch: nextEnvironment.branch }));
    } catch (error) {
      setUiError(error instanceof Error ? error.message : String(error));
    } finally {
      setRefreshingProviders(false);
    }
  }

  async function handleSaveProfile(profile: ProviderProfile) {
    setUiError("");
    setSavingProfileKind(profile.participantKind);
    try {
      if (native) await saveProviderProfile(project.id, profile);
      setProfiles((current) => [
        ...current.filter((value) => value.participantKind !== profile.participantKind),
        profile,
      ]);
    } catch (error) {
      setUiError(error instanceof Error ? error.message : String(error));
    } finally {
      setSavingProfileKind(undefined);
    }
  }

  async function submitObjective(event: FormEvent) {
    event.preventDefault();
    const text = objective.trim();
    if (!text || activeStates.includes(run.state)) return;
    setUiError("");

    const nextRun = createRun(text, environment.participants);
    const writer = nextRun.currentOwner;
    const reviewer =
      environment.participants.find(
        (participant) => participantIsRunnable(participant) && participant.kind !== writer,
      )?.kind ?? writer;
    nextRun.writer = writer;
    nextRun.reviewer = reviewer;
    nextRun.degradedReview = Boolean(writer && reviewer === writer);
    nextRun.route = routeForPhase("build", writer, reviewer);
    setRun(nextRun);
    setObjective("");
    setStream("");
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
    setStream("");
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
        <div className="topbar-status">
          <span className={native ? "online" : ""} />
          {native ? "Autonomous runtime" : "Read-only preview"}
        </div>
        <button
          type="button"
          className="icon-button context-toggle"
          aria-label="Toggle context"
          onClick={() => setInspectorOpen((open) => !open)}
        >
          <PanelRight size={18} />
        </button>
        <button
          type="button"
          className="icon-button"
          aria-label="Open provider settings"
          onClick={() => setActiveView("settings")}
        >
          <Settings size={18} />
        </button>
      </header>

      <ProjectRail
        project={project}
        run={run}
        installedCount={installedCount}
        activeView={activeView}
        onNavigate={showView}
        onOpenProject={() => {
          setActiveView("rooms");
          openInspector("Repository");
        }}
      />

      <main className="room" id="room-main" tabIndex={-1}>
        {activeView === "activity" ? (
          <ActivityView messages={visibleMessages} query={searchQuery} />
        ) : activeView === "settings" ? (
          <SettingsView
            environment={environment}
            profiles={profiles}
            savingKind={savingProfileKind}
            refreshing={refreshingProviders}
            native={native}
            error={uiError}
            onRefresh={refreshProviders}
            onSaveProfile={handleSaveProfile}
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

        <RunLens
          run={run}
          participants={environment.participants}
          onStop={handleStop}
          onResume={handleResume}
        />

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
          {stream && activeStates.includes(run.state) && (
            <article
              className="stream-block"
              role="log"
              aria-live="polite"
              aria-relevant="additions text"
            >
              <header>
                <span className="stream-pulse" />
                <strong>{streamTitle}</strong>
                <small>Live, capped locally</small>
              </header>
              <pre>{stream}</pre>
            </article>
          )}
        </div>

        <form className="composer" onSubmit={submitObjective}>
          <div className="composer-context">
            <span>
              <TerminalSquare size={14} />
              {project.name}
            </span>
            <span>
              <ShieldCheck size={14} />
              Isolate, verify, review, promote
            </span>
          </div>
          <label className="composer-label" htmlFor="room-objective">
            Engineering objective
            <span>State it once. Agent Room carries the handoffs.</span>
          </label>
          <textarea
            id="room-objective"
            value={objective}
            onChange={(event) => setObjective(event.target.value)}
            onKeyDown={(event) => {
              if (event.ctrlKey && event.key === "Enter") event.currentTarget.form?.requestSubmit();
            }}
            placeholder="@codex State the objective once..."
            aria-describedby={uiError ? "composer-error" : undefined}
            rows={2}
          />
          <div className="composer-actions">
            <div className="mention-list">
              {environment.participants.map((participant) => (
                <button
                  type="button"
                  key={participant.kind}
                  disabled={!participantIsRunnable(participant)}
                  onClick={() =>
                    setObjective(
                      (current) => `@${participant.kind} ${current.replace(/^@\w+\s*/, "")}`,
                    )
                  }
                  title={
                    participantIsRunnable(participant)
                      ? `${autonomyLabel(participant.capabilities.autonomyMode)}: ${participant.capabilities.autonomyNote}`
                      : participant.capabilities.autonomyNote
                  }
                >
                  <ParticipantMark participant={participant} />
                  <span>{participant.name}</span>
                </button>
              ))}
            </div>
            <button
              type="submit"
              className="send-button"
              disabled={!objective.trim() || activeStates.includes(run.state)}
              aria-busy={activeStates.includes(run.state)}
            >
              <Play size={15} fill="currentColor" />
              Run autonomously
              <kbd>Ctrl Enter</kbd>
            </button>
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

      <div className={`inspector-wrap ${inspectorOpen ? "open" : ""}`}>
        <button
          type="button"
          className="inspector-close"
          aria-label="Close context"
          onClick={() => setInspectorOpen(false)}
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
    </div>
  );
}
