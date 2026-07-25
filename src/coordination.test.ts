import { describe, expect, it } from "vitest";
import { canRevise, contextPacketSize, createRun, explicitAgent, requestReview } from "./coordination";
import type { Participant } from "./model";

const capabilities = {
  nonInteractiveTurn: true,
  streaming: true,
  structuredOutput: true,
  exactResume: true,
  cancellation: true,
  writeMode: true,
};

const participants: Participant[] = [
  { kind: "codex", name: "Codex", installed: true, state: "ready", capabilities },
  { kind: "claude", name: "Claude", installed: true, state: "ready", capabilities },
  { kind: "cursor", name: "Cursor", installed: false, state: "unavailable", capabilities },
  { kind: "antigravity", name: "Antigravity", installed: false, state: "unavailable", capabilities },
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

  it("never assigns review to the writer", () => {
    const reviewed = requestReview(createRun("@codex build this", participants), participants);
    expect(reviewed.currentOwner).toBe("claude");
    expect(reviewed.reviewCount).toBe(1);
  });

  it("bounds automatic revision to one pass", () => {
    const run = { ...createRun("build", participants), revisionCount: 1 };
    expect(canRevise(run)).toBe(false);
  });

  it("measures selected context in bytes", () => {
    expect(contextPacketSize(["room", "é"])).toBe(6);
  });
});
