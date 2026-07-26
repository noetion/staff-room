export type AgentKind = "codex" | "claude" | "cursor" | "antigravity";

export type RunState =
  | "ready"
  | "selecting"
  | "working"
  | "verifying"
  | "reviewing"
  | "revising"
  | "promoting"
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
  approvalBridge: boolean;
  usageReporting: boolean;
  repositoryScoping: boolean;
  autonomyMode:
    | "isolated-auto"
    | "reviewed-auto"
    | "unattended-bypass"
    | "manual"
    | "unavailable";
  autonomyNote: string;
  capabilityProof: string[];
}

export interface Participant {
  kind: AgentKind;
  name: string;
  installed: boolean;
  version?: string;
  executablePath?: string;
  state: "ready" | "running" | "reviewing" | "manual" | "unavailable";
  capabilities: ProviderCapabilities;
  models: string[];
  modelDiscoveryNote: string;
  supportsEffort: boolean;
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
  agent?: AgentKind;
  label: "Build" | "Verify" | "Review" | "Revise" | "Final review" | "Promote";
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
  writer?: AgentKind;
  reviewer?: AgentKind;
  degradedReview?: boolean;
  worktreePath?: string;
  branch?: string;
  contextBytes?: number;
  artifactPath?: string;
  instructionFiles?: string[];
  skillFiles?: string[];
  recoveryCount?: number;
}

export interface NativeEnvironment {
  native: boolean;
  repositoryPath: string;
  branch: string;
  participants: Participant[];
}

export interface StartRunResult {
  runId: string;
  state: RunState;
  summary: string;
  builder: AgentKind;
  reviewer: AgentKind;
  degradedReview: boolean;
  sessionId?: string;
  changedFiles: string[];
  gitStatus: string;
  verification: VerificationResult[];
  stopped: boolean;
  promoted: boolean;
  worktreePath?: string;
  branch: string;
  contextBytes: number;
  attentionReason?: string;
  artifactPath?: string;
  instructionFiles: string[];
  skillFiles: string[];
  recoveryCount: number;
}

export interface StoredRun {
  id: string;
  objective: string;
  state: RunState;
  currentOwner?: AgentKind;
  writer?: AgentKind;
  reviewer?: AgentKind;
  reviewCount: number;
  revisionCount: number;
  startedAt: string;
  stopReason?: string;
  nativeSessionId?: string;
  worktreePath?: string;
  branch?: string;
  contextBytes: number;
  degradedReview: boolean;
  artifactPath?: string;
  instructionFiles: string[];
  skillFiles: string[];
  recoveryCount: number;
}

export interface ProviderUsage {
  inputTokens?: number;
  cachedInputTokens?: number;
  outputTokens?: number;
  totalCostUsd?: number;
  numTurns?: number;
}

export interface ExecutionReceipt {
  id: string;
  phase: string;
  participant: AgentKind;
  providerVersion?: string;
  requestedModel?: string;
  requestedEffort?: string;
  actualModel?: string;
  sessionId?: string;
  contextBytes: number;
  usage: ProviderUsage;
  usageNote: string;
  createdAt: string;
}

export interface ProviderProfile {
  participantKind: AgentKind;
  model?: string;
  effort?: string;
}

export interface RoomSnapshot {
  messages: RoomMessage[];
  latestRun?: StoredRun;
  receipts: ExecutionReceipt[];
}

export const agentNames: Record<AgentKind, string> = {
  codex: "Codex",
  claude: "Claude",
  cursor: "Cursor",
  antigravity: "Antigravity",
};
