# Chat-first execution v1

## Outcome

Make the composer a fast, provider-native project chat by default. Keep ordinary questions read-only. When the project owner arms autonomous Ship, a chat agent may emit one schema-validated implementation intent and Agent Room carries it through the existing isolated build, verification, review, and promotion route.

## Non-goals

- Do not create a worktree, run verification, or promote from Chat.
- Do not let provider prose, terminal output, or an invalid marker start Ship.
- Do not claim that provider account context or token quotas are locally knowable.

## Acceptance criteria

1. Enter sends chat and Shift+Enter adds a newline.
2. Chat runs one selected CLI in the current project without a worktree or full route, surfacing normalized answer text or provider activity without exposing raw JSON or terminal redraws.
3. Chat uses a provider-native session where available, isolated from build/review sessions, and persists the result in the room timeline.
4. Ship remains directly selectable. When autonomous Ship is armed, a valid skill result can enter that same route without another approval click. The agent proposes the objective; Agent Room owns every gate.
5. Chat uses provider-specific read/plan modes where available and never directly commits or promotes. Cursor Chat uses its proved ask mode, sandbox, and partial-output flags.
6. A provider is never shown as connected solely because its binary or flags were found. Ship requires a successful explicit native connection test; Chat remains available to an installed CLI and records the actual outcome.
7. Normal provider output appears as an agent message in the room timeline for Codex, Claude Code, Cursor Agent, and Antigravity.
8. Chat records the provider-native session identifier and provider-reported usage for every turn when those values are available.
9. Provider discovery is cached on the send path. Explicit refresh re-runs live probes.
10. A resumed provider session receives only the new turn. A provider switch receives only the most recent useful exchange, capped at 4 KiB.
11. Every Chat receipt records preflight, first-output, and total latency, plus quiet links to durable diagnostics.
12. Model selection is stored separately for Chat, Build, and Review.

## Risks and rollback

Autonomous Ship is off by default and is enabled once per project. The skill does not bypass isolation, verification, review, bounded retries, or safe promotion. Antigravity print mode can be silent while it waits for authentication or a response, so Chat caps each silent attempt at 75 seconds and retries once with a fresh session. Connection tests are explicit because they invoke a provider and may use the account. Provider terminal UIs are not embedded into Chat. Authentication failures are surfaced as connection attention.

## Verification

Rust tests cover Chat success semantics, the four provider output translators, provider usage shapes, provider mode selection, the shorter Chat idle limit, provider caching, bounded handoffs, project-scoped autonomy, and Ship-intent validation. The frontend production build type-checks the Enter-to-send composer and automatic Chat-to-Ship transition. Live two-turn smoke tests remain opt-in because they consume provider quota.
