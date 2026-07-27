import { describe, expect, it } from "vitest";
import type { RoomMessage } from "../model";
import { groupMessages } from "./grouping";

function message(id: string, sender: RoomMessage["sender"], createdAt: string, kind: RoomMessage["kind"] = "agent"): RoomMessage { return { id, sender, createdAt, kind, body: "" }; }
describe("groupMessages", () => {
  it("groups a single message", () => { expect(groupMessages([message("1", "codex", "2026-01-01T00:00:00Z")])).toHaveLength(1); });
  it("groups consecutive same-sender messages inside the window", () => { const groups = groupMessages([message("1", "codex", "2026-01-01T00:00:00Z"), message("2", "codex", "2026-01-01T00:01:00Z"), message("3", "codex", "2026-01-01T00:04:00Z")]); expect(groups).toHaveLength(1); expect(groups[0].messages).toHaveLength(3); });
  it("splits same-sender messages outside the window", () => { const groups = groupMessages([message("1", "codex", "2026-01-01T00:00:00Z"), message("2", "codex", "2026-01-01T00:01:00Z"), message("3", "codex", "2026-01-01T00:07:00Z")]); expect(groups).toHaveLength(2); expect(groups[1].messages).toHaveLength(1); });
  it("splits alternating senders", () => { expect(groupMessages([message("1", "codex", "2026-01-01T00:00:00Z"), message("2", "claude", "2026-01-01T00:01:00Z"), message("3", "codex", "2026-01-01T00:02:00Z")])).toHaveLength(3); });
  it("uses evidence messages to split an agent run", () => { const groups = groupMessages([message("1", "codex", "2026-01-01T00:00:00Z"), message("2", "codex", "2026-01-01T00:01:00Z", "evidence"), message("3", "codex", "2026-01-01T00:02:00Z")]); expect(groups.map((group) => group.kind)).toEqual(["agent", "block", "agent"]); });
  it("returns no groups for no messages", () => { expect(groupMessages([])).toEqual([]); });
});
