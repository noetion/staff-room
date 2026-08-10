import { describe, expect, it } from "vitest";
import type { AgentKind, Participant } from "../model";
import { chatAgentFor, shipAgentFor } from "./participants";

function participant(kind: AgentKind): Participant {
  return {
    kind,
    installed: true,
    connectionStatus: "connected",
    capabilities: {
      autonomyMode: kind === "antigravity" ? "unattended-bypass" : "isolated-auto",
      nonInteractiveTurn: true,
    },
  } as Participant;
}

const participants = [
  participant("codex"),
  participant("claude"),
  participant("cursor"),
  participant("antigravity"),
];

describe("participant routing", () => {
  it("honours the visible Ship provider selection", () => {
    expect(shipAgentFor("@claude implement it", participants)).toBe("claude");
    expect(shipAgentFor("@cursor implement it", participants)).toBe("cursor");
    expect(shipAgentFor("@antigravity implement it", participants)).toBe("antigravity");
  });

  it("keeps Antigravity Ship-only", () => {
    expect(chatAgentFor("@antigravity explain it", participants)).toBeUndefined();
    expect(shipAgentFor("@antigravity implement it", participants)).toBe("antigravity");
  });

  it("preserves an explicit unavailable Ship participant so native validation can reject it", () => {
    const disconnected = participants.map((entry) => entry.kind === "cursor"
      ? { ...entry, connectionStatus: "failed" as const }
      : entry);
    const unavailable = participants.map((entry) => entry.kind === "claude"
      ? { ...entry, capabilities: { ...entry.capabilities, autonomyMode: "unavailable" as const } }
      : entry);

    expect(shipAgentFor("@cursor implement it", disconnected)).toBe("cursor");
    expect(shipAgentFor("@claude implement it", unavailable)).toBe("claude");
    expect(shipAgentFor("implement it", disconnected)).toBe("codex");
  });
});
