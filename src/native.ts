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

export async function loadRoom(projectId: string): Promise<RoomSnapshot> {
  return invoke<RoomSnapshot>("load_room", { projectId });
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
    repositoryPath: string;
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
    repositoryPath: string;
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
    repositoryPath: string;
    requestedAgent?: AgentKind;
  },
): Promise<QuickEditResult> {
  return invoke<QuickEditResult>("quick_edit_start", { request });
}

export async function quickEditApply(editId: string): Promise<void> {
  return invoke("quick_edit_apply", { request: { editId } });
}

export async function quickEditDiscard(editId: string): Promise<void> {
  return invoke("quick_edit_discard", { request: { editId } });
}

export async function testProviderConnection(request: {
  projectId: string;
  repositoryPath: string;
  participantKind: AgentKind;
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

export async function stopRun(runId: string): Promise<StopRunResult> {
  return invoke<StopRunResult>("stop_run", { runId });
}

export async function abandonRun(runId: string): Promise<void> {
  return invoke("abandon_run", { runId });
}

export async function onRunEvent(
  callback: (event: RunEvent) => void,
): Promise<UnlistenFn> {
  return listen<RunEvent>("run-event", (event) => callback(event.payload));
}
