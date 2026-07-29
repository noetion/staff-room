import type { MessageKind, RoomMessage } from "../model";
import { parseTimestamp } from "./format";

export type MessageGroupKind = "human" | "system" | "block" | "agent";
export interface MessageGroup { sender: RoomMessage["sender"]; kind: MessageGroupKind; messages: RoomMessage[]; }
/** Kinds that always stand alone rather than merging into a sender run. */
const blockKinds: MessageKind[] = ["evidence", "error"];
function groupKind(message: RoomMessage): MessageGroupKind { if (message.sender === "human") return "human"; if (message.sender === "system") return "system"; if (blockKinds.includes(message.kind)) return "block"; return "agent"; }
export function groupMessages(messages: RoomMessage[], windowMs = 300000): MessageGroup[] { const groups: MessageGroup[] = []; for (const message of messages) { const kind = groupKind(message); const previous = groups.at(-1); const previousMessage = previous?.messages.at(-1); const isWithinWindow = previousMessage !== undefined && parseTimestamp(message.createdAt) - parseTimestamp(previousMessage.createdAt) <= windowMs; if (kind !== "block" && previous?.kind === kind && previous.sender === message.sender && isWithinWindow) previous.messages.push(message); else groups.push({ sender: message.sender, kind, messages: [message] }); } return groups; }
