# Agent Room plan evaluation

## Verdict

Build it, with the project room and coordination policy as the product boundary.

The three plans align on the real problem: project context, repository evidence, and agent conversations are fragmented across tools, so the user acts as a courier. A local desktop process is the right owner for provider execution, durable state, context selection, Git evidence, cancellation, and bounded handoffs.

## Decisions retained

- One project and one primary repository are the root object.
- One conversational interaction model; explicit `@agent` mentions always win.
- The coordinator owns completion, limits, and routing policy. Agents propose; they do not grant themselves authority.
- Only one active writer can use a repository checkout.
- Review uses a different participant, with one automatic revision and two reviews maximum.
- The full transcript is not broadcast. Each activation receives a selected context packet.
- SQLite is operational truth. `.agent-room/memory.md` is a human-readable projection owned by Agent Room.
- Provider capability must be displayed truthfully.
- Tauri commands and events are the desktop boundary. No local HTTP server is needed.

## Changes to the plan

1. **Provider proof is a hard gate, not a documentation task.** The implementation must be driven by installed CLI output and current primary documentation. Shared abstractions should follow two proven adapters, not precede them.
2. **Do not start with thirteen tables or every module in the proposed tree.** The first schema includes only projects, messages, runs, and provider sessions. Evidence can remain attached to run completion until a second consumer needs independent querying.
3. **Codex is the first adapter on this machine.** Codex 0.144.4 is installed. Claude, Cursor Agent, and Antigravity are not installed, so their UI state is unavailable and no execution behavior is simulated.
4. **A second provider is the next product gate.** One-provider execution proves the desktop boundary, but the core problem is not solved until a build result and repository evidence can be passed to a different reviewer without copying.
5. **Memory merge follows cross-agent relay.** Before two real providers produce competing durable proposals, a generalized memory merge engine has no load-bearing consumer.
6. **The Handoff Lens is the single expressive visual device.** Timeline entries, evidence, findings, and memory remain solid, readable surfaces.

## Current slice

This repository implements the first native vertical slice:

1. inspect the local repository and installed participants;
2. state an objective once with an explicit or default Codex route;
3. execute `codex exec` directly in the repository with NDJSON output;
4. stream native events into the room;
5. stop the owned child process;
6. persist the human objective, run, result, and provider session;
7. capture changed files and Git status as direct evidence;
8. recover with an explicit error that states possible repository impact.

## Remaining gate

Install and prove one second automation-friendly provider, then implement:

```text
Codex build
→ capture result + diff + checks
→ select only relevant context
→ second provider review
→ structured findings
→ one bounded revision
→ final review or user decision
```

That slice is the first point at which the original manual-transfer problem is fully removed.
