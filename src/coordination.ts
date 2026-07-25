import type { AgentKind, Participant, RouteStep, Run } from "./model";

const mentionPattern = /@(codex|claude|cursor|antigravity)\b/i;

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
    return participants.find((participant) => participant.kind === requested && participant.installed)?.kind;
  }
  if (replyTarget) {
    return participants.find((participant) => participant.kind === replyTarget && participant.installed)?.kind;
  }
  return participants.find((participant) => participant.installed)?.kind;
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

export function requestReview(
  run: Run,
  participants: Participant[],
): Run {
  if (!run.currentOwner) return run;
  const reviewer = participants.find(
    (participant) => participant.installed && participant.kind !== run.currentOwner,
  );
  if (!reviewer) {
    return {
      ...run,
      state: "waiting",
      stopReason: "Independent review is required, but no second participant is available.",
    };
  }

  const route: RouteStep[] = [
    ...run.route.map((step) => ({ ...step, state: "complete" as const })),
    { agent: reviewer.kind, label: "Review", state: "current" },
  ];
  return {
    ...run,
    state: "reviewing",
    currentOwner: reviewer.kind,
    reviewCount: run.reviewCount + 1,
    route,
  };
}

export function canRevise(run: Run): boolean {
  return run.revisionCount < 1 && run.reviewCount <= 2;
}

export function isTerminal(run: Run): boolean {
  return ["complete", "failed", "stopped"].includes(run.state);
}

export function contextPacketSize(parts: string[]): number {
  return parts.reduce((total, part) => total + new TextEncoder().encode(part).length, 0);
}
