# Four-provider Chat Transport Evaluation

## Decision

Use one Agent Room chat contract with four provider-owned adapters. Do not treat a terminal UI as the shared protocol.

Each adapter must use the most structured, resumable interface its CLI actually exposes. The room timeline consumes only normalized chat events. A terminal surface is not a chat response and is reserved for an explicit provider interaction that cannot be represented as a normal prompt.

## Evidence

- The current implementation starts every provider as a persistent terminal UI and only understands Antigravity's screen. Codex, Claude Code, and Cursor therefore expose terminal redraws rather than a stable chat contract.
- Codor uses a provider adapter boundary. One delivery is one turn, provider output becomes normalized events, native session identifiers are persisted, and a turn has exactly one terminal outcome.
- Codor's current Codex adapter uses one `codex app-server` JSON-line process and resumes a persisted thread.
- Codor's current Claude adapter uses the first-party Agent SDK with a long-lived streaming input and resumes a persisted session.
- The installed Codex CLI exposes `app-server`, JSONL `exec`, exact resume, model selection, read-only sandboxing, and never-ask approvals.
- The installed Claude CLI exposes print mode, streaming JSON input/output, partial messages, exact resume, model selection, and plan permission mode.
- Cursor Agent exposes structured print output on supported versions. Its exact resume capability must be taken from the installed help rather than assumed.
- `agy` 1.1.7 exposes print mode, project attachment, model and effort selection, and conversation resume, but no structured streaming output. Antigravity is therefore a truthful completion-stream adapter for v1.

## Product contract

Every provider turn emits the same bounded event vocabulary:

1. `started`: the local turn was accepted and identifies its provider.
2. `activity`: the provider is running but has not produced answer text.
3. `text`: the current assistant answer snapshot.
4. `usage`: provider-reported tokens, cost, context window, or rate window when present.
5. `interaction-required`: authentication, consent, approval, or a provider question needs the user.
6. `completed`, `failed`, or `stopped`: exactly one terminal outcome.

Project/provider session identity is durable. Process persistence is an adapter optimization, not the product contract. A provider process may be recreated if its native session can be resumed without replaying the room transcript.

The implementation remains four built-in Rust adapters selected by provider kind. Agent Room does not need a plugin framework for four fixed CLIs.

## Adapter matrix

| Provider | v1 transport | Context continuity | Visible response | Truthful downgrade |
| --- | --- | --- | --- | --- |
| Codex | JSONL `exec` with final-output file, then app-server optimization | exact thread resume | structured answer snapshots | process starts per turn until app-server is promoted |
| Claude Code | print mode with stream JSON | exact session resume | structured answer snapshots | process starts per turn until SDK/stream-input transport is promoted |
| Cursor Agent | print mode with stream JSON, ask mode, sandboxing, and partial output where installed help proves them | exact resume only when installed help proves it | structured answer deltas | capability is limited to the flags proved by the installed CLI |
| Antigravity | `--print` in plan mode | conversation resume | plain answer stream | completion-stream, not token-delta streaming |

## Model selection

The Settings picker shows provider-supplied model choices when a zero-spend model command exists. Otherwise it provides a truthful curated alias list plus exact manual entry. A native interactive picker is not scraped as an account model API.

## Acceptance gates

1. Local send acknowledgement is visible within 150ms.
2. Chat never creates a worktree, runs the Ship route, or shows raw JSON or terminal redraws.
3. Every installed provider can complete two turns in the attached repository while retaining its own native context where the CLI supports resume.
4. Normal answer text appears as an agent timeline message for all four providers.
5. Authentication or unsupported capabilities stop with a provider-specific, actionable status.
6. A provider without token-delta output is shown as completion-stream, not "instant streaming."
7. Fixture replay covers success, resume identity, malformed output, nonzero exit, empty EOF, and cancellation for each adapter.
8. Live smoke tests are opt-in because they consume provider quota. A public v1 claim requires a recorded live pass for every supported installed CLI.

## Risks and fallback

- Provider JSON vocabularies change. Parsers therefore use captured scrubbed fixtures and ignore unknown events.
- Process startup remains in the Codex and Claude v1 path. Native session resume removes transcript replay; app-server and SDK transports are measured optimizations after the correctness gate.
- Antigravity has no structured streaming mode in the installed CLI. Its print output is the supported v1 contract; its full TUI is not embedded into the room.
- Cursor capability claims follow its installed help. The verified v1 path uses ask mode plus sandboxing for Chat and confines unattended write mode to Agent Room's managed worktree.
- If an installed CLI does not prove the required flags, Agent Room disables that adapter with the missing capability instead of guessing arguments.

## Rollback

The rollback boundary is the adapter dispatcher and normalized chat event parser. Ship continues to use the existing provider invocation path. Removing the automatic terminal path does not change worktrees, verification, review, promotion, or stored room messages.
