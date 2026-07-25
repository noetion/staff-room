export type AgentKind = "codex" | "claude" | "cursor" | "antigravity";

export type RunState =
  | "ready"
  | "selecting"
  | "working"
  | "reviewing"
  | "revising"
  | "waiting"
  | "complete"
  | "failed"
  | "stopped";

export type MessageKind =
  | "human"
  | "agent"
  | "handoff"
  | "review"
  | "evidence"
  | "status"
  | "decision"
  | "error";

export interface ProviderCapabilities {
  nonInteractiveTurn: boolean;
  streaming: boolean;
  structuredOutput: boolean;
  exactResume: boolean;
  cancellation: boolean;
  writeMode: boolean;
}

export interface Participant {
  kind: AgentKind;
  name: string;
  installed: boolean;
  version?: string;
  executablePath?: string;
  state: "ready" | "running" | "reviewing" | "unavailable";
  capabilities: ProviderCapabilities;
}

export interface Project {
  id: string;
  name: string;
  goal: string;
  repositoryPath: string;
  branch: string;
}

export interface RoomMessage {
  id: string;
  kind: MessageKind;
  sender: "human" | "system" | AgentKind;
  body: string;
  createdAt: string;
  runId?: string;
  changedFiles?: string[];
  verification?: VerificationResult[];
  reason?: string;
}

export interface VerificationResult {
  label: string;
  status: "passed" | "failed" | "not-run";
  detail: string;
}

export interface RouteStep {
  agent: AgentKind;
  label: "Build" | "Review" | "Revise" | "Final review";
  state: "complete" | "current" | "next";
}

export interface Run {
  id: string;
  objective: string;
  state: RunState;
  currentOwner?: AgentKind;
  route: RouteStep[];
  reviewCount: number;
  revisionCount: number;
  startedAt: string;
  stopReason?: string;
  nativeSessionId?: string;
}

export interface NativeEnvironment {
  native: boolean;
  repositoryPath: string;
  branch: string;
  participants: Participant[];
}

export interface StartRunResult {
  runId: string;
  summary: string;
  sessionId?: string;
  changedFiles: string[];
  gitStatus: string;
  stopped: boolean;
}

export const agentNames: Record<AgentKind, string> = {
  codex: "Codex",
  claude: "Claude",
  cursor: "Cursor",
  antigravity: "Antigravity",
};
