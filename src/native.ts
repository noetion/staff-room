import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AgentKind,
  ChatResult,
  ModelDiscoveryResult,
  NativeEnvironment,
  Participant,
  ProviderProfile,
  Project,
  ProjectSettings,
  VerificationConfig,
  QuickEditResult,
  MessageCursor,
  RoomSnapshot,
  StartRunResult,
  StopRunResult,
} from "./model";

export interface RunEvent {
  runId: string;
  eventType: "phase" | "stream" | "text-delta" | "attention" | "complete";
  phase: string;
  state: string;
  agent?: AgentKind;
  title: string;
  detail: string;
  textDelta?: string;
  contextBytes?: number;
}

export function isNativeApp(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

export async function getEnvironment(forceRefresh = false): Promise<NativeEnvironment> {
  return invoke<NativeEnvironment>("get_environment", { forceRefresh });
}

export async function projectPick(): Promise<string | undefined> {
  return invoke<string | undefined>("project_pick");
}

export async function projectAttach(path: string): Promise<Project> {
  return invoke<Project>("project_attach", { path });
}

export async function projectList(): Promise<Project[]> {
  return invoke<Project[]>("project_list");
}

export async function projectSelect(id: string): Promise<Project> {
  return invoke<Project>("project_select", { id });
}

export async function projectActive(): Promise<Project | undefined> {
  return invoke<Project | undefined>("project_active");
}

export interface VoiceStatus {
  available: boolean;
  recording: boolean;
  modelPath?: string;
  detail: string;
  maxSeconds: number;
}

export interface VoiceLevel {
  level: number;
}

export interface VoiceTranscription {
  text: string;
  durationMs: number;
}

export interface QuickEditActionResult {
  cleanupWarning?: string;
}

export async function allocateOperationId(
  projectId: string,
  kind: "chat" | "quick-edit" | "ship",
): Promise<string> {
  return invoke<string>("allocate_operation_id", { request: { projectId, kind } });
}

export async function loadRoom(
  projectId: string,
  before?: MessageCursor,
): Promise<RoomSnapshot> {
  return invoke<RoomSnapshot>("load_room", { projectId, before });
}

export async function loadProjectSettings(projectId: string): Promise<ProjectSettings> {
  return invoke<ProjectSettings>("load_project_settings", { projectId });
}

export async function saveProjectSettings(
  projectId: string,
  settings: ProjectSettings,
): Promise<void> {
  return invoke("save_project_settings", {
    settings: { projectId, ...settings },
  });
}

export async function loadVerificationConfig(projectId: string): Promise<VerificationConfig> {
  return invoke<VerificationConfig>("load_verification_config", { projectId });
}

export async function saveVerificationConfig(
  projectId: string,
  config: VerificationConfig,
): Promise<void> {
  return invoke("save_verification_config", {
    config: { projectId, ...config },
  });
}

export async function loadProviderProfiles(
  projectId: string,
): Promise<ProviderProfile[]> {
  return invoke<ProviderProfile[]>("load_provider_profiles", { projectId });
}

export async function saveProviderProfile(
  projectId: string,
  profile: ProviderProfile,
): Promise<void> {
  return invoke("save_provider_profile", { profile: { ...profile, projectId } });
}

export async function startRoomRun(
  request: {
    runId: string;
    projectId: string;
    objective: string;
    requestedAgent?: AgentKind;
  },
): Promise<StartRunResult> {
  return invoke<StartRunResult>("start_room_run", { request });
}

export async function startRoomChat(
  request: {
    runId: string;
    projectId: string;
    message: string;
    requestedAgent?: AgentKind;
    activeRunId?: string;
  },
): Promise<ChatResult> {
  return invoke<ChatResult>("start_room_chat", { request });
}

export async function quickEditStart(
  request: {
    editId: string;
    projectId: string;
    message: string;
    requestedAgent?: AgentKind;
  },
): Promise<QuickEditResult> {
  return invoke<QuickEditResult>("quick_edit_start", { request });
}

export async function quickEditApply(projectId: string, editId: string): Promise<QuickEditActionResult> {
  return invoke<QuickEditActionResult>("quick_edit_apply", { request: { projectId, editId } });
}

export async function quickEditDiscard(projectId: string, editId: string): Promise<QuickEditActionResult> {
  return invoke<QuickEditActionResult>("quick_edit_discard", { request: { projectId, editId } });
}

export async function testProviderConnection(request: {
  projectId: string;
  participantKind: AgentKind;
  model: string;
  effort: string;
}): Promise<Participant> {
  return invoke<Participant>("test_provider_connection", { request });
}

export async function discoverProviderModels(
  participantKind: AgentKind,
): Promise<ModelDiscoveryResult> {
  return invoke<ModelDiscoveryResult>("discover_provider_models", {
    request: { participantKind },
  });
}

export async function stopRun(projectId: string, runId: string): Promise<StopRunResult> {
  return invoke<StopRunResult>("stop_run", { request: { projectId, runId } });
}

export async function abandonRun(projectId: string, runId: string): Promise<void> {
  return invoke("abandon_run", { request: { projectId, runId } });
}

export async function approveRunPromotion(projectId: string, runId: string): Promise<void> {
  return invoke("approve_run_promotion", { request: { projectId, runId } });
}

export async function getVoiceStatus(): Promise<VoiceStatus> {
  return invoke<VoiceStatus>("voice_status");
}

export async function pickVoiceModel(): Promise<VoiceStatus> {
  return invoke<VoiceStatus>("voice_pick_model");
}

export async function pickVoiceEngine(): Promise<VoiceStatus> {
  return invoke<VoiceStatus>("voice_pick_engine");
}

export async function startVoiceCapture(): Promise<void> {
  return invoke("voice_start");
}

export async function stopVoiceCapture(cancel = false): Promise<VoiceTranscription> {
  return invoke<VoiceTranscription>("voice_stop", { cancel });
}

export async function onVoiceLevel(
  callback: (level: number) => void,
): Promise<UnlistenFn> {
  return listen<VoiceLevel>("voice-level", (event) => callback(event.payload.level));
}

export async function onRunEvent(
  callback: (event: RunEvent) => void,
): Promise<UnlistenFn> {
  return listen<RunEvent>("run-event", (event) => callback(event.payload));
}
