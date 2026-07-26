import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AgentKind,
  NativeEnvironment,
  ProviderProfile,
  Project,
  RoomSnapshot,
  StartRunResult,
} from "./model";

export interface RunEvent {
  runId: string;
  eventType: "phase" | "stream" | "attention" | "complete";
  phase: string;
  state: string;
  agent?: AgentKind;
  title: string;
  detail: string;
  contextBytes?: number;
}

export function isNativeApp(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

export async function getEnvironment(): Promise<NativeEnvironment> {
  return invoke<NativeEnvironment>("get_environment");
}

export async function saveProject(project: Project): Promise<void> {
  return invoke("save_project", { project });
}

export async function loadRoom(projectId: string): Promise<RoomSnapshot> {
  return invoke<RoomSnapshot>("load_room", { projectId });
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

export async function stopRun(runId: string): Promise<boolean> {
  return invoke<boolean>("stop_run", { runId });
}

export async function onRunEvent(
  callback: (event: RunEvent) => void,
): Promise<UnlistenFn> {
  return listen<RunEvent>("run-event", (event) => callback(event.payload));
}
