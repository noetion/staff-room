import type { NativeEnvironment, Project, RoomMessage, Run } from "./model";

export const seedProject: Project = {
  id: "agent-room",
  name: "Agent Room",
  goal: "Remove manual context transfer between coding agents.",
  repositoryPath: "<repo>",
  branch: "main",
};

export const previewEnvironment: NativeEnvironment = {
  native: false,
  repositoryPath: seedProject.repositoryPath,
  branch: "main",
  participants: [
    {
      kind: "codex",
      name: "Codex",
      installed: true,
      version: "preview",
      state: "ready",
      capabilities: {
        nonInteractiveTurn: true,
        streaming: true,
        structuredOutput: true,
        exactResume: true,
        cancellation: true,
        writeMode: true,
      },
    },
    {
      kind: "claude",
      name: "Claude",
      installed: false,
      state: "unavailable",
      capabilities: {
        nonInteractiveTurn: true,
        streaming: true,
        structuredOutput: true,
        exactResume: true,
        cancellation: true,
        writeMode: true,
      },
    },
    {
      kind: "cursor",
      name: "Cursor",
      installed: false,
      state: "unavailable",
      capabilities: {
        nonInteractiveTurn: true,
        streaming: true,
        structuredOutput: true,
        exactResume: true,
        cancellation: true,
        writeMode: true,
      },
    },
    {
      kind: "antigravity",
      name: "Antigravity",
      installed: false,
      state: "unavailable",
      capabilities: {
        nonInteractiveTurn: true,
        streaming: false,
        structuredOutput: false,
        exactResume: true,
        cancellation: true,
        writeMode: true,
      },
    },
  ],
};

export const seedRun: Run = {
  id: "welcome",
  objective: "Build a durable room that carries work between coding agents.",
  state: "ready",
  route: [],
  reviewCount: 0,
  revisionCount: 0,
  startedAt: new Date().toISOString(),
};

export const seedMessages: RoomMessage[] = [
  {
    id: "welcome",
    kind: "status",
    sender: "system",
    body: "This room keeps the objective, repository evidence, handoffs, and review findings together.",
    reason: "Start with one objective. Agent Room selects only an available participant.",
    createdAt: new Date().toISOString(),
  },
];
