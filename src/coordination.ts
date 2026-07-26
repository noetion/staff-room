import type { AgentKind, Participant, RouteStep, Run } from "./model";

const mentionPattern = /@(codex|claude|cursor|antigravity)\b/i;

export function participantIsRunnable(participant: Participant): boolean {
  return (
    participant.installed &&
    participant.connectionStatus === "connected" &&
    !["manual", "unavailable"].includes(participant.capabilities.autonomyMode)
  );
}

export function participantCanChat(participant: Participant): boolean {
  return participant.kind !== "antigravity" && participant.installed && participant.capabilities.nonInteractiveTurn;
}

export const participantCanQuickEdit = participantCanChat;

export function explicitAgent(objective: string): AgentKind | undefined {
  return objective.match(mentionPattern)?.[1].toLowerCase() as AgentKind | undefined;
}

export function selectParticipant(
  objective: string,
  participants: Participant[],
  replyTarget?: AgentKind,
): AgentKind | undefined {
  const requested = explicitAgent(objective);
  if (requested) {
    return participants.find(
      (participant) => participant.kind === requested && participantIsRunnable(participant),
    )?.kind;
  }
  if (replyTarget) {
    return participants.find(
      (participant) => participant.kind === replyTarget && participantIsRunnable(participant),
    )?.kind;
  }
  return participants.find(participantIsRunnable)?.kind;
}

export function selectChatParticipant(
  objective: string,
  participants: Participant[],
  replyTarget?: AgentKind,
): AgentKind | undefined {
  const requested = explicitAgent(objective);
  if (requested) {
    return participants.find((participant) => participant.kind === requested && participantCanChat(participant))?.kind;
  }
  if (replyTarget) {
    return participants.find((participant) => participant.kind === replyTarget && participantCanChat(participant))?.kind;
  }
  return participants.find(participantCanChat)?.kind;
}

export function createRun(
  objective: string,
  participants: Participant[],
  now = new Date(),
): Run {
  const owner = selectParticipant(objective, participants);
  const id = crypto.randomUUID();
  return {
    id,
    objective,
    state: owner ? "working" : "waiting",
    currentOwner: owner,
    route: owner ? [{ agent: owner, label: "Build", state: "current" }] : [],
    reviewCount: 0,
    revisionCount: 0,
    startedAt: now.toISOString(),
    stopReason: owner ? undefined : "No installed participant can accept this objective.",
  };
}

export function routeForPhase(
  phase: string,
  writer?: AgentKind,
  reviewer?: AgentKind,
): RouteStep[] {
  const steps: RouteStep[] = [
    { agent: writer, label: "Build", state: "next" },
    { label: "Verify", state: "next" },
    { agent: reviewer, label: "Review", state: "next" },
  ];
  if (phase === "revise" || phase === "final-review") {
    steps.push(
      { agent: writer, label: "Revise", state: "next" },
      { agent: reviewer, label: "Final review", state: "next" },
    );
  }
  steps.push({ label: "Promote", state: "next" });

  const order: Record<string, number> = {
    prepare: 0,
    build: 0,
    verify: 1,
    review: 2,
    revise: 3,
    "final-review": 4,
    promote: steps.length - 1,
    complete: steps.length,
  };
  const current = order[phase] ?? 0;
  return steps.map((step, index) => ({
    ...step,
    state: index < current ? "complete" : index === current ? "current" : "next",
  }));
}
