import {
  Braces,
  Check,
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
import { AttentionCard, EvidenceCard, MarkdownBody } from "./components/conversation";
import { agentNames } from "./model";
import {
  unavailableModelValues,
  type ProviderDraft,
} from "./lib/provider-profiles";
import { initials } from "./lib/format";
import {
  autonomyLabel,
  canUseChat,
  chatAgentFor,
  isRunnableParticipant,
  shipAgentFor,
} from "./lib/participants";
import { asRunState, hydrateRun, isRunRecoverable, shouldShowRunProgress } from "./lib/runs";
import {
  approveRunPromotion,
  allocateOperationId,
  getEnvironment,
  getVoiceStatus,
  discoverProviderModels,
  isNativeApp,
  loadProviderProfiles,
  loadProjectSettings,
  loadVerificationConfig,
  loadRoom,
  onRunEvent,
  onVoiceLevel,
  projectActive,
  projectAttach,
  projectList,
  projectPick,
  projectSelect,
  pickVoiceEngine,
  pickVoiceModel,
  quickEditApply,
  abandonRun,
  quickEditDiscard,
  quickEditStart,
  saveProviderProfile,
  saveProjectSettings,
  saveVerificationConfig,
  startRoomChat,
  startRoomRun,
  startVoiceCapture,
  stopRun,
  stopVoiceCapture,
  testProviderConnection,
  type RunEvent,
  type VoiceStatus,
} from "./native";
import { NavRail, TitleBar } from "./components/chrome";
import { StatusPill } from "./components/chrome/StatusPill";
import { Conversation } from "./components/conversation";
import { RunProgressCard } from "./components/conversation/RunProgressCard";
import { Aurora, Monogram, RefractionFilter, Wordmark } from "./components/primitives";
import { ActivityView, AttachProjectView, EmptyState, ErrorState, RoomHeader } from "./components/views";
import { SettingsView } from "./components/settings";
export { ProviderProfileCard } from "./components/settings";
import { Composer } from "./components/composer";
import { Inspector as InspectorSheet } from "./components/inspector";

type InspectorTab = "Repository" | "Participants" | "Evidence" | "Memory";
type PrimaryView = "rooms" | "activity" | "settings";

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

/// Module scope keeps this reference stable, so the meter subscription is created once
/// per recording rather than on every render of a very large component.
async function subscribeVoiceLevel(
  handler: (level: number) => void,
): Promise<() => void> {
  if (!isNativeApp()) return () => {};
  return onVoiceLevel(handler);
}

const unavailableVoice: VoiceStatus = {
  available: false,
  recording: false,
  detail: "Local voice dictation is available in the Tauri desktop app.",
  maxSeconds: 30,
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
/** How close to the bottom of the timeline still counts as "following the run". */
const followThresholdPx = 120;

type ComposerMode = "ask" | "quick-edit" | "ship";
type LiveActivityItem = { title: string; detail: string };
type QuickEditPreview = QuickEditResult & { projectId: string; projectName: string };

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
    <Monogram label={initials(participant.kind)} aria-hidden="true">
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
  const [voiceStatus, setVoiceStatus] = useState<VoiceStatus>(unavailableVoice);
  const [savingProjectSettings, setSavingProjectSettings] = useState(false);
  const [modelCatalog, setModelCatalog] = useState<Partial<Record<AgentKind, string[]>>>({});
  const [modelDiscoveryDetails, setModelDiscoveryDetails] = useState<Partial<Record<AgentKind, string>>>({});
  const [discoveringModelsKind, setDiscoveringModelsKind] = useState<AgentKind>();
  const [run, setRun] = useState<Run>(emptyRun);
  const [objective, setObjective] = useState("");
  const [composerMode, setComposerMode] = useState<ComposerMode>("ask");
  const [chatSending, setChatSending] = useState(false);
  const [quickEdit, setQuickEdit] = useState<QuickEditPreview>();
  const [quickEditAction, setQuickEditAction] = useState<"apply" | "discard">();
  const [bootState, setBootState] = useState<"loading" | "ready" | "error">("loading");
  const [bootError, setBootError] = useState("");
  const [bootAttempt, setBootAttempt] = useState(0);
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
  const pinnedToBottomRef = useRef(true);
  const runRef = useRef(run);
  const chatRunRef = useRef<string | undefined>(undefined);
  const quickEditRunRef = useRef<string | undefined>(undefined);
  const activeProjectIdRef = useRef(project.id);
  const activationGenerationRef = useRef(0);
  const streamTextBufferRef = useRef("");
  const streamFrameRef = useRef<number | undefined>(undefined);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const contextToggleRef = useRef<HTMLButtonElement>(null);
  const native = isNativeApp();

  useEffect(() => {
    runRef.current = run;
  }, [run]);

  useEffect(() => {
    activeProjectIdRef.current = project.id;
  }, [project.id]);

  useEffect(() => {
    if (!native) return;
    void getVoiceStatus()
      .then(setVoiceStatus)
      .catch((error) => {
        setVoiceStatus({
          ...unavailableVoice,
          detail: error instanceof Error ? error.message : String(error),
        });
      });
  }, [native]);

  async function refreshRoom(projectId = project.id) {
    if (!native) return;
    const snapshot = await loadRoom(projectId);
    if (projectId !== activeProjectIdRef.current) return;
    setMessages(snapshot.messages);
    setHasMoreMessages(snapshot.hasMore);
    setRun(snapshot.latestRun ? hydrateRun(snapshot.latestRun) : emptyRun);
    setReceipts(snapshot.receipts);
  }

  async function activateProject(nextProject: Project, knownEnvironment?: NativeEnvironment) {
    const generation = ++activationGenerationRef.current;
    const nextEnvironment = knownEnvironment ?? await getEnvironment();
    const [snapshot, nextProfiles, nextSettings, nextVerificationConfig, nextProjects] = await Promise.all([
      loadRoom(nextProject.id),
      loadProviderProfiles(nextProject.id),
      loadProjectSettings(nextProject.id),
      loadVerificationConfig(nextProject.id),
      projectList(),
    ]);
    if (generation !== activationGenerationRef.current) return;
    activeProjectIdRef.current = nextProject.id;
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
    setQuickEdit(undefined);
    setActivity([]);
    setAttention(undefined);
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
          setBootState("ready");
        });
      } else {
        setBootState("ready");
      }
      return;
    }
    setBootState("loading");
    setBootError("");
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
          setBootState("ready");
          return;
        }
        await activateProject(nextProject, nextEnvironment);
        if (!disposed) setBootState("ready");
      })
      .catch((error) => {
        if (disposed) return;
        setBootError(error instanceof Error ? error.message : String(error));
        setBootState("error");
      });

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
            : `The Staff Room / ${event.phase}`,
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
        const activeProjectId = activeProjectIdRef.current;
        if (activeProjectId) void refreshRoom(activeProjectId).catch(() => undefined);
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
  }, [native, bootAttempt]);

  useEffect(() => {
    if (loadingOlderMessagesRef.current) {
      loadingOlderMessagesRef.current = false;
      return;
    }
    const timeline = timelineRef.current;
    if (!timeline) return;
    // Follow the newest output only while the reader is already at the bottom.
    // Scrolling back through a long run must not be interrupted by every delta.
    const distanceFromBottom = timeline.scrollHeight - timeline.scrollTop - timeline.clientHeight;
    if (!pinnedToBottomRef.current && distanceFromBottom > followThresholdPx) return;
    timeline.scrollTo({ top: timeline.scrollHeight, behavior: "smooth" });
  }, [messages, activity]);

  useEffect(() => {
    const timeline = timelineRef.current;
    if (!timeline) return;
    const onScroll = () => {
      pinnedToBottomRef.current =
        timeline.scrollHeight - timeline.scrollTop - timeline.clientHeight <= followThresholdPx;
    };
    onScroll();
    timeline.addEventListener("scroll", onScroll, { passive: true });
    return () => timeline.removeEventListener("scroll", onScroll);
  }, [activeView, environment.attached]);

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
    if (chatSending || quickEdit) {
      setUiError("Finish or discard the active Chat or Quick Edit before switching projects.");
      return;
    }
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
    if (chatSending || quickEdit) {
      setUiError("Finish or discard the active Chat or Quick Edit before switching projects.");
      return;
    }
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
    const unavailableModels = unavailableModelValues(
      nextProfiles,
      modelCatalog[participantKind] ?? [],
      modelCatalog[participantKind] !== undefined,
    );
    if (unavailableModels.length > 0) {
      setUiError(
        `${agentNames[participantKind]} model ${unavailableModels.map((model) => `\`${model}\``).join(", ")} is not in the refreshed catalogue. Choose an exact model identifier before saving.`,
      );
      return;
    }
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
      setModelCatalog((current) => {
        if (!(kind in current)) return current;
        const next = { ...current };
        delete next[kind];
        return next;
      });
      setModelDiscoveryDetails((current) => {
        if (!(kind in current)) return current;
        const next = { ...current };
        delete next[kind];
        return next;
      });
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

    // Preserve an explicit participant choice all the way to Rust. The native
    // capability check must reject an unavailable agent visibly instead of the
    // renderer silently falling back to the first runnable participant.
    const writer = requestedWriter;
    let operationId: string = crypto.randomUUID();
    if (native) {
      try {
        operationId = await allocateOperationId(project.id, "ship");
      } catch (error) {
        setUiError(error instanceof Error ? error.message : String(error));
        return;
      }
    }
    const nextRun: Run = {
      id: operationId,
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
          reason: "Launch the Tauri desktop app to create a managed worktree and run human-gated Ship.",
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
          body: "The Ship run could not start.",
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
    if (run.state === "awaiting-promotion") {
      setUiError("Promote or abandon the verified Ship result before starting another objective.");
      return;
    }
    if (run.state === "promoting") {
      setUiError("Promotion is finishing now. Side chat will reopen when this Ship run completes.");
      return;
    }
    if (sideChatAvailable) {
      await submitChat(true);
      return;
    }
    await runShipObjective(text, shipAgentFor(text, environment.participants));
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
    let chatRunId: string = crypto.randomUUID();
    if (native) {
      try {
        chatRunId = await allocateOperationId(project.id, "chat");
      } catch (error) {
        setUiError(error instanceof Error ? error.message : String(error));
        return;
      }
    }
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
      if (chatRunRef.current === chatRunId) {
        setChatSending(false);
        chatRunRef.current = undefined;
      }
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
    let editId: string;
    try {
      editId = await allocateOperationId(project.id, "quick-edit");
    } catch (error) {
      setUiError(error instanceof Error ? error.message : String(error));
      return;
    }
    setUiError("");
    setObjective("");
    // The composer's Stop button reads this ref. Without it a Quick Edit shows a
    // live Stop control that cancels nothing.
    quickEditRunRef.current = editId;
    setChatSending(true);
    try {
      const result = await quickEditStart({
        editId,
        projectId: project.id,
        message: text,
        requestedAgent: participant,
      });
      setQuickEdit({ ...result, projectId: project.id, projectName: project.name });
    } catch (error) {
      setUiError(error instanceof Error ? error.message : String(error));
    } finally {
      if (quickEditRunRef.current === editId) quickEditRunRef.current = undefined;
      setChatSending(false);
    }
  }

  async function applyQuickEdit() {
    if (!quickEdit) return;
    if (quickEdit.projectId !== project.id) {
      setUiError("This Quick Edit belongs to another project and cannot be applied here.");
      return;
    }
    setUiError("");
    setQuickEditAction("apply");
    try {
      const result = await quickEditApply(project.id, quickEdit.editId);
      if (!result.completed) return;
      setQuickEdit(undefined);
      if (result.cleanupWarning) {
        setUiError(`The edit was applied, but temporary isolation cleanup needs attention: ${result.cleanupWarning}`);
      }
      await refreshRoom(project.id);
    } catch (error) {
      setUiError(error instanceof Error ? error.message : String(error));
    } finally {
      setQuickEditAction(undefined);
    }
  }

  async function handlePickVoiceEngine() {
    setUiError("");
    try {
      await pickVoiceEngine();
      setVoiceStatus(await getVoiceStatus());
    } catch (error) {
      setUiError(error instanceof Error ? error.message : String(error));
    }
  }

  async function handlePickVoiceModel() {
    setUiError("");
    try {
      await pickVoiceModel();
      setVoiceStatus(await getVoiceStatus());
    } catch (error) {
      setUiError(error instanceof Error ? error.message : String(error));
    }
  }

  async function handleVoiceStart() {
    setUiError("");
    await startVoiceCapture();
    setVoiceStatus((current) => ({ ...current, recording: true }));
  }

  async function handleVoiceStop(cancel = false) {
    try {
      const result = await stopVoiceCapture(cancel);
      return result.text;
    } finally {
      setVoiceStatus((current) => ({ ...current, recording: false }));
    }
  }

  async function discardQuickEdit() {
    if (!quickEdit) return;
    setUiError("");
    setQuickEditAction("discard");
    try {
      await quickEditDiscard(project.id, quickEdit.editId);
      setQuickEdit(undefined);
    } catch (error) {
      setUiError(error instanceof Error ? error.message : String(error));
    } finally {
      setQuickEditAction(undefined);
    }
  }

  async function submitComposer(event: FormEvent) {
    event.preventDefault();
    // Sending is an explicit request to follow the newest output again.
    pinnedToBottomRef.current = true;
    if (sideChatAvailable) await submitChat(true);
    else if (composerMode === "ask") await submitChat();
    else if (composerMode === "quick-edit") await submitQuickEdit();
    else await submitObjective();
  }

  /**
   * The composer's Stop button covers both composer-owned operations: a chat turn
   * and a Quick Edit. Both register a cancellation entry under their operation ID
   * in Rust, so both are stoppable through the same command.
   */
  async function handleStopChat() {
    const operationId = chatRunRef.current ?? quickEditRunRef.current;
    if (!operationId) return;
    try {
      const result = await stopRun(project.id, operationId);
      if (!result.cancelled) {
        setUiError(result.reason);
        return;
      }
      if (chatRunRef.current === operationId) {
        setChatSending(false);
        chatRunRef.current = undefined;
      }
    } catch (error) {
      setUiError(error instanceof Error ? error.message : String(error));
    }
  }

  async function handleStop() {
    if (native) {
      try {
        const result = await stopRun(project.id, runRef.current.id);
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
        stopReason: "Stop requested. The Staff Room is preserving recoverable work.",
      }));
    }
  }

  async function handleAbandon() {
    if (!native || activeStates.includes(run.state) || !run.worktreePath) return;
    setUiError("");
    try {
      await abandonRun(project.id, run.id);
      await refreshRoom(project.id);
    } catch (error) {
      setUiError(error instanceof Error ? error.message : String(error));
    }
  }

  async function handlePromote() {
    if (!native || run.state !== "awaiting-promotion" || !run.worktreePath) return;
    setUiError("");
    setRun((current) => ({ ...current, state: "promoting", stopReason: undefined }));
    try {
      await approveRunPromotion(project.id, run.id);
      await refreshRoom(project.id);
      setEnvironment(await getEnvironment());
    } catch (error) {
      setUiError(error instanceof Error ? error.message : String(error));
      await refreshRoom(project.id).catch(() => undefined);
    }
  }

  async function handleResume() {
    if (
      !native ||
      !isRunRecoverable(run)
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

  if (bootState === "loading") {
    return (
      <main className="boot-screen" aria-busy="true">
        <span className="eyebrow">Local workspace</span>
        <h1>Opening The Staff Room</h1>
        <p>Loading projects, provider state, and the latest room evidence.</p>
      </main>
    );
  }

  if (bootState === "error") {
    return (
      <main className="boot-screen">
        <span className="eyebrow">Startup stopped</span>
        <h1>The Staff Room could not open</h1>
        <ErrorState
          cause={bootError}
          action={(
            <button type="button" className="secondary-button" onClick={() => setBootAttempt((attempt) => attempt + 1)}>
              Retry startup
            </button>
          )}
        />
      </main>
    );
  }

  return (
    <div className="app-shell" data-run-state={run.state}>
      <Aurora />
      <RefractionFilter
        composerVisible={environment.attached && activeView === "rooms"}
        surfaceKey={`${environment.attached}:${activeView}`}
      />
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

      <main
        className={`room${!environment.attached || activeView === "settings" ? " room--signature" : ""}`}
        id="room-main"
        tabIndex={-1}
      >
        {(!environment.attached || activeView === "settings") && (
          <Wordmark
            corner="bottom-left"
            size="calc(var(--s-8) * 3 + var(--s-2))"
          />
        )}
        {(activeStates.includes(run.state) || ["awaiting-promotion", "waiting", "failed", "stopped"].includes(run.state)) && (
          <StatusPill
            state={run.state}
            stage={run.route.find((step) => step.state === "current")?.label ?? "Ship run"}
            startedAt={run.startedAt}
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
          <ActivityView messages={visibleMessages} query={searchQuery} renderMessage={(message) => <TimelineMessageContent message={message} />} />
        ) : activeView === "settings" ? (
          <SettingsView
            environment={environment}
            profiles={profiles}
            projectSettings={projectSettings}
            verificationConfig={verificationConfig}
            voiceStatus={voiceStatus}
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
            onPickVoiceEngine={() => void handlePickVoiceEngine()}
            onPickVoiceModel={() => void handlePickVoiceModel()}
            onVerificationConfigChange={setVerificationConfig}
            onSaveVerificationConfig={handleSaveVerificationConfig}
          />
        ) : (
          <>
        <RoomHeader project={project} installedCount={installedCount} />

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
          {!messages.length && !searchQuery && (
            <EmptyState
              title="Nothing has happened in this room yet."
              suggestions={[
                { label: "Plan the next task", value: "Plan the next task for this repository." },
                { label: "Review the codebase", value: "Review the codebase and identify the highest-value next step." },
                { label: "Explain this project", value: "Explain this project and its current state." },
              ]}
              onSuggestion={setObjective}
            />
          )}
          {attention && (
            <AttentionCard title={attention.title} detail={attention.detail} onDismiss={() => setAttention(undefined)} />
          )}
          {shouldShowRunProgress(run) && !searchQuery && (
            <RunProgressCard
              run={run}
              activity={activity}
              limits={`${run.revisionCount}/1 revise · ${run.reviewCount}/2 review · ${run.recoveryCount ?? 0}/2 recover`}
              onStop={handleStop}
              onPromote={handlePromote}
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
          onVoiceError={setUiError}
          onVoiceStart={handleVoiceStart}
          onVoiceStop={handleVoiceStop}
          subscribeVoiceLevel={subscribeVoiceLevel}
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
          quickEditDisabled={sideChatAvailable || ["awaiting-promotion", "promoting"].includes(run.state) || Boolean(quickEdit)}
          quickEditPreview={quickEdit && (
            <>
              <div className="composer-preview-context">
                <span>{quickEdit.projectName}</span>
                <span>{quickEdit.editId}</span>
              </div>
              <p><strong>{agentNames[quickEdit.participant]}</strong>: {quickEdit.summary}</p>
              {quickEdit.stopped && (
                <p className="composer-preview-warning" role="status">
                  This Quick Edit was stopped early. Review the partial diff carefully before applying it.
                </p>
              )}
              <pre>{quickEdit.diff || "No file changes were produced."}</pre>
              <div className="composer-preview-actions">
                <button type="button" className="composer-preview-apply" onClick={() => void applyQuickEdit()} disabled={!quickEdit.diff || Boolean(quickEditAction)}>
                  <Check size={15} /> {quickEditAction === "apply" ? "Applying..." : "Apply edit"}
                </button>
                <button type="button" className="danger-button" onClick={() => void discardQuickEdit()} disabled={Boolean(quickEditAction)}>
                  <X size={15} /> {quickEditAction === "discard" ? "Discarding..." : "Discard"}
                </button>
              </div>
            </>
          )}
          renderParticipantMark={(participant) => <ParticipantMark participant={participant} />}
          shipDisabled={sideChatAvailable || ["awaiting-promotion", "promoting"].includes(run.state)}
          sideChatAvailable={sideChatAvailable}
          titleForParticipant={(participant) =>
            (composerMode === "ask" || composerMode === "quick-edit" ? canUseChat(participant) : isRunnableParticipant(participant))
              ? `${autonomyLabel(participant.capabilities.autonomyMode)}: ${participant.capabilities.autonomyNote}`
              : participant.capabilities.autonomyNote
          }
          value={objective}
          voiceAvailable={native && voiceStatus.available}
          voiceDetail={voiceStatus.detail}
          voiceDisabled={chatSending || run.state === "promoting" || Boolean(quickEditAction)}
        />
          </>
        )}
      </main>

      {inspectorOpen && <InspectorSheet project={project} environment={environment} activeTab={activeTab} setActiveTab={setActiveTab} run={run} messages={messages} receipts={receipts} onClose={() => { setInspectorOpen(false); requestAnimationFrame(() => contextToggleRef.current?.focus()); }} />}
    </div>
  );
}
