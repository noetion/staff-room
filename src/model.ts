export type AgentKind = "codex" | "claude" | "cursor" | "antigravity";

export type RunState =
  | "ready"
  | "selecting"
  | "working"
  | "verifying"
  | "reviewing"
  | "revising"
  | "awaiting-promotion"
  | "promoting"
  | "waiting"
  | "complete"
  | "failed"
  | "stopped"
  | "abandoned";

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
  warmSession: boolean;
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
  connectionStatus: "connected" | "sign-in-required" | "unverified" | "not-installed" | "failed";
  connectionDetail: string;
  lastVerifiedAt?: string;
  capabilities: ProviderCapabilities;
  models: string[];
  modelDiscoveryNote: string;
  supportsEffort: boolean;
  effortOptions: string[];
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
  status: "passed" | "failed" | "unavailable" | "not-run";
  detail: string;
}

export interface VerificationCommand {
  label: string;
  command: string;
  enabled: boolean;
}

export interface VerificationConfig {
  enabled: boolean;
  commands: VerificationCommand[];
  prepare?: string;
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
  attached: boolean;
  repositoryPath: string;
  branch: string;
  participants: Participant[];
  contextBudgetBytes: number;
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

export interface ChatResult {
  runId: string;
  participant: AgentKind;
  summary: string;
  sessionId?: string;
  actualModel?: string;
  stopped: boolean;
  shipIntent?: ShipIntent;
}

export interface StopRunResult {
  cancelled: boolean;
  reason: string;
}

export interface QuickEditResult {
  editId: string;
  participant: AgentKind;
  summary: string;
  diff: string;
  stopped: boolean;
}

export interface ShipIntent {
  schemaVersion: 1;
  objective: string;
  reason: string;
}

export interface ProjectSettings {
  autonomousShipEnabled: boolean;
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
  route: RouteStep[];
}

export interface ProviderUsage {
  inputTokens?: number | null;
  cachedInputTokens?: number | null;
  outputTokens?: number | null;
  totalCostUsd?: number | null;
  numTurns?: number | null;
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
  preflightMs?: number | null;
  processStartMs?: number | null;
  firstOutputMs?: number | null;
  totalMs?: number | null;
  sessionResumed: boolean;
  packetBytesSaved: number;
  stdoutLogPath?: string;
  stderrLogPath?: string;
}

export interface ProviderProfile {
  participantKind: AgentKind;
  route: "chat" | "build" | "review";
  model?: string;
  effort?: string;
}

export interface ModelDiscoveryResult {
  models: string[];
  detail: string;
}

export interface RoomSnapshot {
  messages: RoomMessage[];
  hasMore: boolean;
  nextMessageCursor?: MessageCursor;
  latestRun?: StoredRun;
  receipts: ExecutionReceipt[];
}

export interface MessageCursor {
  createdAt: string;
  id: string;
}

export const agentNames: Record<AgentKind, string> = {
  codex: "Codex",
  claude: "Claude",
  cursor: "Cursor",
  antigravity: "Antigravity",
};
