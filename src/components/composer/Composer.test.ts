import { describe, expect, it } from "vitest";
import { insertTranscript } from "./Composer";

describe("insertTranscript", () => {
  it("inserts editable dictation at the caret without sending", () => {
    expect(insertTranscript("Review now", 7, 7, "the diff")).toEqual({
      value: "Review the diff now",
      caret: 15,
    });
  });

  it("replaces the selected range and normalizes surrounding spaces", () => {
    expect(insertTranscript("Ask old text please", 4, 12, "the agent")).toEqual({
      value: "Ask the agent please",
      caret: 13,
    });
  });

  it("leaves the draft unchanged when no speech was produced", () => {
    expect(insertTranscript("Typed draft", 5, 5, "   ")).toEqual({
      value: "Typed draft",
      caret: 5,
    });
  });
});
