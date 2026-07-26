import { describe, expect, it } from "vitest";
import {
  canRevise,
  contextPacketSize,
  createRun,
  explicitAgent,
  selectChatParticipant,
  requestReview,
  routeForPhase,
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
  { kind: "codex", name: "Codex", installed: true, state: "ready", connectionStatus: "connected", connectionDetail: "Test", capabilities, models: [], modelDiscoveryNote: "Test", supportsEffort: true, effortOptions: ["low", "medium", "high", "xhigh"] },
  { kind: "claude", name: "Claude", installed: true, state: "ready", connectionStatus: "connected", connectionDetail: "Test", capabilities, models: [], modelDiscoveryNote: "Test", supportsEffort: true, effortOptions: ["low", "medium", "high", "xhigh", "max"] },
  { kind: "cursor", name: "Cursor", installed: false, state: "unavailable", connectionStatus: "not-installed", connectionDetail: "Test", capabilities, models: [], modelDiscoveryNote: "Test", supportsEffort: true, effortOptions: ["low", "medium", "high", "xhigh", "max"] },
  { kind: "antigravity", name: "Antigravity", installed: false, state: "unavailable", connectionStatus: "not-installed", connectionDetail: "Test", capabilities, models: [], modelDiscoveryNote: "Test", supportsEffort: true, effortOptions: ["low", "medium", "high"] },
];

describe("coordination policy", () => {
  it("honours an explicit installed participant", () => {
    expect(explicitAgent("@claude review this")).toBe("claude");
    expect(createRun("@claude review this", participants).currentOwner).toBe("claude");
  });

  it("does not silently route an unavailable explicit participant", () => {
    const run = createRun("@cursor implement this", participants);
    expect(run.state).toBe("waiting");
    expect(run.currentOwner).toBeUndefined();
  });

  it("does not route an installed but unverified participant", () => {
    const unverified = participants.map((participant) =>
      participant.kind === "claude" ? { ...participant, connectionStatus: "unverified" as const } : participant,
    );
    expect(createRun("@claude review this", unverified).state).toBe("waiting");
  });

  it("allows project chat with an installed but unverified CLI", () => {
    const unverified = participants.map((participant) =>
      participant.kind === "claude" ? { ...participant, connectionStatus: "unverified" as const } : participant,
    );
    expect(selectChatParticipant("@claude hello", unverified)).toBe("claude");
  });

  it("never assigns review to the writer when a second participant exists", () => {
    const reviewed = requestReview(createRun("@codex build this", participants), participants);
    expect(reviewed.currentOwner).toBe("claude");
    expect(reviewed.reviewCount).toBe(1);
  });

  it("bounds automatic revision to one pass", () => {
    const run = { ...createRun("build", participants), revisionCount: 1 };
    expect(canRevise(run)).toBe(false);
  });

  it("measures selected context in UTF-8 bytes", () => {
    expect(contextPacketSize(["room", "\u00e9"])).toBe(6);
  });

  it("renders the bounded revision route deterministically", () => {
    const route = routeForPhase("final-review", "codex", "claude");
    expect(route.map((step) => step.label)).toEqual([
      "Build",
      "Verify",
      "Review",
      "Revise",
      "Final review",
      "Promote",
    ]);
    expect(route.find((step) => step.label === "Final review")?.state).toBe("current");
  });
});
