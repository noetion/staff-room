import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { NativeEnvironment, StartRunResult } from "./model";

export interface ActivationEvent {
  runId: string;
  stream: "stdout" | "stderr" | "status";
  payload: string;
}

export function isNativeApp(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

export async function getEnvironment(): Promise<NativeEnvironment> {
  return invoke<NativeEnvironment>("get_environment");
}

export async function startCodexRun(
  projectId: string,
  objective: string,
  repositoryPath: string,
): Promise<StartRunResult> {
  return invoke<StartRunResult>("start_codex_run", {
    projectId,
    objective,
    repositoryPath,
  });
}

export async function stopRun(runId: string): Promise<boolean> {
  return invoke<boolean>("stop_run", { runId });
}

export async function onActivationEvent(
  callback: (event: ActivationEvent) => void,
): Promise<UnlistenFn> {
  return listen<ActivationEvent>("activation-event", (event) => callback(event.payload));
}
