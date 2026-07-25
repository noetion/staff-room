import {
  Activity,
  ArrowRight,
  Bot,
  Braces,
  Check,
  ChevronRight,
  CircleStop,
  GitBranch,
  History,
  PanelRight,
  Play,
  Search,
  Settings,
  ShieldCheck,
  Sparkles,
  TerminalSquare,
  X,
} from "lucide-react";
import { FormEvent, useEffect, useMemo, useRef, useState } from "react";
import { createRun } from "./coordination";
import type {
  AgentKind,
  NativeEnvironment,
  Participant,
  Project,
  RoomMessage,
  Run,
} from "./model";
import { agentNames } from "./model";
import {
  getEnvironment,
  isNativeApp,
  onActivationEvent,
  startCodexRun,
  stopRun,
} from "./native";
import { previewEnvironment, seedMessages, seedProject, seedRun } from "./seed";

type InspectorTab = "Repository" | "Participants" | "Evidence" | "Memory";

function initials(kind: AgentKind): string {
  return { codex: "CX", claude: "CL", cursor: "CU", antigravity: "AG" }[kind];
}

function relativeTime(timestamp: string): string {
  const minutes = Math.max(0, Math.round((Date.now() - new Date(timestamp).getTime()) / 60_000));
  return minutes < 1 ? "now" : `${minutes}m`;
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
}: {
  run: Run;
  participants: Participant[];
  onStop: () => void;
}) {
  const active = run.currentOwner
    ? participants.find((participant) => participant.kind === run.currentOwner)
    : undefined;
  const isRunning = ["working", "reviewing", "revising", "selecting"].includes(run.state);

  return (
    <section className={`run-lens state-${run.state}`} aria-label="Current run" aria-live="polite">
      <div className="lens-glow" aria-hidden="true" />
      <div className="lens-copy">
        <span className="eyebrow">
          <span className="state-pulse" />
          {run.state === "ready" ? "Room ready" : run.state.replace("_", " ")}
        </span>
        <strong>
          {run.state === "waiting"
            ? "Your input is needed"
            : active
              ? `${active.name} owns this turn`
              : "State one objective"}
        </strong>
        <span className="lens-objective">{run.stopReason ?? run.objective}</span>
      </div>
      <div className="route" aria-label="Run route">
        {run.route.length ? (
          run.route.map((step, index) => (
            <div className={`route-step ${step.state}`} key={`${step.agent}-${index}`}>
              <span>{initials(step.agent)}</span>
              <small>{step.label}</small>
              {index < run.route.length - 1 && <ArrowRight size={14} aria-hidden="true" />}
            </div>
          ))
        ) : (
          <div className="route-placeholder">Objective → work → evidence</div>
        )}
      </div>
      <div className="lens-action">
        {isRunning ? (
          <button className="danger-button" onClick={onStop}>
            <CircleStop size={16} />
            Stop run
          </button>
        ) : (
          <div className="limit-readout">
            <span>{run.revisionCount}/1 revisions</span>
            <span>{run.reviewCount}/2 reviews</span>
          </div>
        )}
      </div>
    </section>
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
          {isHuman ? <Sparkles size={13} /> : message.kind === "evidence" ? <ShieldCheck size={13} /> : <Bot size={13} />}
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
        {message.reason && <div className="reason-line"><Braces size={14} />{message.reason}</div>}
        {message.changedFiles?.length ? (
          <div className="file-list">
            {message.changedFiles.map((file) => <code key={file}>{file}</code>)}
          </div>
        ) : null}
      </div>
    </article>
  );
}

function ProjectRail({ project }: { project: Project }) {
  return (
    <aside className="project-rail">
      <nav className="primary-nav" aria-label="Primary">
        <button className="nav-button active" aria-label="Rooms"><TerminalSquare size={19} /><span>Rooms</span></button>
        <button className="nav-button" aria-label="Activity"><Activity size={19} /><span>Activity</span></button>
        <button className="nav-button" aria-label="Settings"><Settings size={19} /><span>Settings</span></button>
      </nav>
      <div className="rail-section">
        <span className="rail-label">Recent rooms</span>
        <button className="project-row selected">
          <span className="project-monogram">AR</span>
          <span>
            <strong>{project.name}</strong>
            <small>Ready · Codex available</small>
          </span>
          <ChevronRight size={15} />
        </button>
      </div>
      <button className="new-room-button"><span>+</span> New project</button>
      <div className="rail-foot">
        <span className="local-indicator" />
        <span><strong>Local only</strong><small>Room data stays here</small></span>
      </div>
    </aside>
  );
}

function Inspector({
  project,
  environment,
  activeTab,
  setActiveTab,
  lastMessage,
}: {
  project: Project;
  environment: NativeEnvironment;
  activeTab: InspectorTab;
  setActiveTab: (tab: InspectorTab) => void;
  lastMessage?: RoomMessage;
}) {
  const tabs: InspectorTab[] = ["Repository", "Participants", "Evidence", "Memory"];
  return (
    <aside className="inspector">
      <div className="inspector-tabs" role="tablist">
        {tabs.map((tab) => (
          <button
            key={tab}
            className={activeTab === tab ? "active" : ""}
            onClick={() => setActiveTab(tab)}
            role="tab"
            aria-selected={activeTab === tab}
          >
            {tab}
          </button>
        ))}
      </div>
      {activeTab === "Repository" && (
        <div className="inspector-body">
          <div className="inspector-heading"><GitBranch size={17} /><span><small>Repository</small><strong>{project.name}</strong></span></div>
          <div className="data-row"><span>Branch</span><code>{environment.branch || "unavailable"}</code></div>
          <div className="path-block">{environment.repositoryPath}</div>
          <div className="status-note"><Check size={15} /><span><strong>Working tree observed</strong><small>Evidence is captured before and after native runs.</small></span></div>
        </div>
      )}
      {activeTab === "Participants" && (
        <div className="inspector-body participant-list">
          {environment.participants.map((participant) => (
            <div className="participant-row" key={participant.kind}>
              <ParticipantMark participant={participant} />
              <span className="participant-copy">
                <strong>{participant.name}</strong>
                <small>{participant.installed ? participant.version ?? "Installed" : "Not installed"}</small>
              </span>
              <span className={`status-chip ${participant.installed ? "available" : ""}`}>
                {participant.installed ? "Ready" : "Unavailable"}
              </span>
            </div>
          ))}
          <p className="inspector-help">Unavailable participants are never silently substituted.</p>
        </div>
      )}
      {activeTab === "Evidence" && (
        <div className="inspector-body">
          <div className="evidence-tile"><ShieldCheck size={18} /><span><strong>Repository truth</strong><small>Direct Git evidence, not an agent claim</small></span></div>
          <div className="data-row"><span>Latest event</span><strong>{lastMessage?.kind ?? "None yet"}</strong></div>
          <div className="data-row"><span>Changed files</span><strong>{lastMessage?.changedFiles?.length ?? 0}</strong></div>
          <div className="data-row"><span>Verification</span><strong>Not configured</strong></div>
        </div>
      )}
      {activeTab === "Memory" && (
        <div className="inspector-body memory-body">
          <span className="memory-label">Current direction</span>
          <p>Use a local room to carry objectives, evidence, and review findings between coding agents.</p>
          <span className="memory-label">Constraints</span>
          <p>One active writer. No self-review. One automatic revision. Two reviews maximum.</p>
          <button className="secondary-button"><History size={15} /> View revisions</button>
        </div>
      )}
    </aside>
  );
}

export function App() {
  const [project, setProject] = useState(seedProject);
  const [environment, setEnvironment] = useState(previewEnvironment);
  const [messages, setMessages] = useState<RoomMessage[]>(seedMessages);
  const [run, setRun] = useState<Run>(seedRun);
  const [objective, setObjective] = useState("");
  const [stream, setStream] = useState("");
  const [activeTab, setActiveTab] = useState<InspectorTab>("Repository");
  const [inspectorOpen, setInspectorOpen] = useState(false);
  const timelineRef = useRef<HTMLDivElement>(null);
  const native = isNativeApp();

  useEffect(() => {
    if (!native) return;
    getEnvironment()
      .then((nextEnvironment) => {
        setEnvironment(nextEnvironment);
        setProject((current) => ({
          ...current,
          repositoryPath: nextEnvironment.repositoryPath,
          branch: nextEnvironment.branch,
        }));
      })
      .catch((error) => console.error("Failed to inspect native environment", error));
    let unlisten: (() => void) | undefined;
    onActivationEvent((event) => {
      if (event.stream === "stdout") setStream((current) => `${current}${event.payload}\n`);
    }).then((dispose) => { unlisten = dispose; });
    return () => unlisten?.();
  }, [native]);

  useEffect(() => {
    timelineRef.current?.scrollTo({ top: timelineRef.current.scrollHeight, behavior: "smooth" });
  }, [messages, stream]);

  const installedCount = useMemo(
    () => environment.participants.filter((participant) => participant.installed).length,
    [environment],
  );

  async function submitObjective(event: FormEvent) {
    event.preventDefault();
    const text = objective.trim();
    if (!text || ["working", "reviewing", "revising"].includes(run.state)) return;
    const nextRun = createRun(text, environment.participants);
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

    if (!nextRun.currentOwner) {
      setMessages((current) => [...current, {
        id: crypto.randomUUID(),
        kind: "decision",
        sender: "system",
        body: nextRun.stopReason ?? "Choose an available participant.",
        createdAt: new Date().toISOString(),
        runId: nextRun.id,
      }]);
      return;
    }

    if (!native) {
      setRun({ ...nextRun, state: "waiting", stopReason: "Desktop runtime required to execute this objective." });
      setMessages((current) => [...current, {
        id: crypto.randomUUID(),
        kind: "status",
        sender: "system",
        body: "Preview mode does not execute provider CLIs. Launch the Tauri desktop app to run Codex in this repository.",
        reason: "The browser preview is intentionally read-only.",
        createdAt: new Date().toISOString(),
        runId: nextRun.id,
      }]);
      return;
    }

    try {
      const result = await startCodexRun(project.id, text, project.repositoryPath);
      setRun((current) => ({
        ...current,
        id: result.runId,
        state: result.stopped ? "stopped" : "complete",
        nativeSessionId: result.sessionId,
        route: current.route.map((step) => ({ ...step, state: "complete" })),
      }));
      setMessages((current) => [...current, {
        id: crypto.randomUUID(),
        kind: "agent",
        sender: "codex",
        body: result.summary || "Codex completed without a final summary.",
        createdAt: new Date().toISOString(),
        runId: result.runId,
        changedFiles: result.changedFiles,
      }, {
        id: crypto.randomUUID(),
        kind: "evidence",
        sender: "system",
        body: result.changedFiles.length
          ? `${result.changedFiles.length} changed file${result.changedFiles.length === 1 ? "" : "s"} captured from Git.`
          : "Git reported no changed files for this run.",
        reason: result.gitStatus,
        createdAt: new Date().toISOString(),
        runId: result.runId,
        changedFiles: result.changedFiles,
      }]);
    } catch (error) {
      const detail = error instanceof Error ? error.message : String(error);
      setRun((current) => ({ ...current, state: "failed", stopReason: detail }));
      setMessages((current) => [...current, {
        id: crypto.randomUUID(),
        kind: "error",
        sender: "system",
        body: "Codex stopped before returning a result.",
        reason: `${detail} Repository state may contain partial changes; inspect Git evidence before retrying.`,
        createdAt: new Date().toISOString(),
        runId: nextRun.id,
      }]);
    }
  }

  async function handleStop() {
    if (native) await stopRun(run.id);
    setRun((current) => ({ ...current, state: "stopped", stopReason: "Stopped by you." }));
  }

  return (
    <div className="app-shell">
      <header className="topbar">
        <div className="brand"><span className="brand-mark"><span /></span><strong>Agent Room</strong></div>
        <button className="project-switch"><span className="project-monogram small">AR</span><span><strong>{project.name}</strong><small>{project.branch}</small></span><ChevronRight size={15} /></button>
        <button className="search-button"><Search size={16} /><span>Search rooms and evidence</span><kbd>Ctrl K</kbd></button>
        <div className="topbar-status"><span className={native ? "online" : ""} />{native ? "Desktop runtime" : "Preview mode"}</div>
        <button className="icon-button context-toggle" aria-label="Toggle context" onClick={() => setInspectorOpen((open) => !open)}><PanelRight size={18} /></button>
        <button className="icon-button" aria-label="Settings"><Settings size={18} /></button>
      </header>

      <ProjectRail project={project} />

      <main className="room">
        <div className="room-header">
          <div>
            <span className="eyebrow">Project room</span>
            <h1>{project.name}</h1>
            <p>{project.goal}</p>
          </div>
          <div className="room-meta"><span><GitBranch size={14} />{project.branch}</span><span><Bot size={14} />{installedCount} of 4 ready</span></div>
        </div>

        <RunLens run={run} participants={environment.participants} onStop={handleStop} />

        <div className="timeline" ref={timelineRef}>
          <div className="timeline-date"><span>Room opened</span></div>
          {messages.map((message) => <TimelineEntry key={message.id} message={message} />)}
          {stream && (
            <article className="stream-block">
              <header><span className="stream-pulse" /><strong>Codex is working</strong><small>Live provider events</small></header>
              <pre>{stream.slice(-5000)}</pre>
            </article>
          )}
        </div>

        <form className="composer" onSubmit={submitObjective}>
          <div className="composer-context"><span><TerminalSquare size={14} />{project.name}</span><span><Bot size={14} />Mentions route explicitly</span></div>
          <textarea
            value={objective}
            onChange={(event) => setObjective(event.target.value)}
            onKeyDown={(event) => {
              if (event.ctrlKey && event.key === "Enter") event.currentTarget.form?.requestSubmit();
            }}
            placeholder="@codex State the objective once…"
            aria-label="Objective"
            rows={2}
          />
          <div className="composer-actions">
            <div className="mention-list">
              {environment.participants.map((participant) => (
                <button
                  type="button"
                  key={participant.kind}
                  disabled={!participant.installed}
                  onClick={() => setObjective((current) => `@${participant.kind} ${current.replace(/^@\w+\s*/, "")}`)}
                  title={participant.installed ? `Route to ${participant.name}` : `${participant.name} is not installed`}
                >
                  <ParticipantMark participant={participant} />
                  <span>{participant.name}</span>
                </button>
              ))}
            </div>
            <button className="send-button" disabled={!objective.trim() || ["working", "reviewing", "revising"].includes(run.state)}>
              <Play size={15} fill="currentColor" />
              Send objective
              <kbd>Ctrl ↵</kbd>
            </button>
          </div>
        </form>
      </main>

      <div className={`inspector-wrap ${inspectorOpen ? "open" : ""}`}>
        <button className="inspector-close" aria-label="Close context" onClick={() => setInspectorOpen(false)}><X size={18} /></button>
        <Inspector project={project} environment={environment} activeTab={activeTab} setActiveTab={setActiveTab} lastMessage={messages.at(-1)} />
      </div>
    </div>
  );
}
