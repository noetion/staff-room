import { describe, expect, it } from "vitest";
import {
  explicitAgent,
  participantCanChat,
  participantIsRunnable,
  selectChatParticipant,
} from "./coordination";
import type { Participant } from "./model";

const capabilities = {
  nonInteractiveTurn: true,
  streaming: true,
  structuredOutput: true,
  exactResume: true,
  cancellation: true,
  writeMode: true,
  approvalBridge: true,
  usageReporting: true,
  repositoryScoping: true,
  autonomyMode: "isolated-auto" as const,
  autonomyNote: "Test capability.",
  capabilityProof: ["Test proof."],
};

const participants: Participant[] = [
  {
    kind: "codex", name: "Codex", installed: true, state: "ready", connectionStatus: "connected", connectionDetail: "Test", capabilities,
    models: [], modelDiscoveryNote: "Test", supportsEffort: true, effortOptions: ["low", "medium", "high", "xhigh"],
  },
  {
    kind: "claude", name: "Claude", installed: true, state: "ready", connectionStatus: "connected", connectionDetail: "Test", capabilities,
    models: [], modelDiscoveryNote: "Test", supportsEffort: true, effortOptions: ["low", "medium", "high", "xhigh", "max"],
  },
];

describe("participant selection", () => {
  it("parses every explicit agent mention case-insensitively", () => {
    expect(explicitAgent("@Claude review this")).toBe("claude");
    expect(explicitAgent("@CODEX review this")).toBe("codex");
    expect(explicitAgent("@cursor review this")).toBe("cursor");
    expect(explicitAgent("@Antigravity review this")).toBe("antigravity");
    expect(explicitAgent("review this")).toBeUndefined();
  });

  it("allows chat only for an installed non-interactive participant", () => {
    expect(participantCanChat(participants[0])).toBe(true);
    expect(participantCanChat({ ...participants[0], installed: false })).toBe(false);
    expect(participantCanChat({ ...participants[0], capabilities: { ...capabilities, nonInteractiveTurn: false } })).toBe(false);
  });

  it("requires installation, a connection, and autonomous capability to run", () => {
    expect(participantIsRunnable(participants[0])).toBe(true);
    expect(participantIsRunnable({ ...participants[0], installed: false })).toBe(false);
    expect(participantIsRunnable({ ...participants[0], connectionStatus: "unverified" })).toBe(false);
    expect(participantIsRunnable({ ...participants[0], capabilities: { ...capabilities, autonomyMode: "manual" } })).toBe(false);
    expect(participantIsRunnable({ ...participants[0], capabilities: { ...capabilities, autonomyMode: "unavailable" } })).toBe(false);
  });

  it("allows project chat with an installed but unverified CLI", () => {
    const unverified = participants.map((participant) => participant.kind === "claude" ? { ...participant, connectionStatus: "unverified" as const } : participant);
    expect(selectChatParticipant("@claude hello", unverified)).toBe("claude");
  });
});
