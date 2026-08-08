# Chat-first execution v1

## Outcome

Make the composer a fast, provider-native working chat by default. Ask is non-mutating. Quick Edit prepares an explicit bounded change in managed isolation and shows its diff before Apply can update the attached checkout. When the project owner arms autonomous Ship, a chat agent may route substantial or unattended implementation through the isolated build, verification, review, and promotion workflow.

## Non-goals

- Do not create a worktree, run broad verification, commit, or promote from Chat.
- Do not let provider prose, terminal output, or an invalid marker start Ship.
- Do not claim that provider account context or token quotas are locally knowable.

## Acceptance criteria

1. Enter sends chat and Shift+Enter adds a newline.
2. Chat runs one selected CLI in the current project without a worktree or full route, surfacing normalized answer text or provider activity without exposing raw JSON or terminal redraws.
3. Chat uses a provider-native session where available, isolated from build/review sessions, and persists the result in the room timeline.
4. Ship remains directly selectable. When autonomous Ship is armed, only a substantial or unattended task enters that route without another approval click. Bounded changes remain in Quick Edit. The agent proposes the objective; The Staff Room owns every gate.
5. Ask invokes supported CLIs in read-only mode. Quick Edit may write only inside managed isolation and keeps the attached checkout unchanged until the user applies the visible diff. Active-Ship side chat remains read-only.
6. A provider is never shown as connected solely because its binary or flags were found. Ship requires a successful explicit native connection test; Chat remains available to an installed CLI and records the actual outcome.
7. Normal provider output appears as an agent message in the room timeline for Codex, Claude Code, and Cursor Agent in Ask and Quick Edit. Antigravity is visibly Ship-only and participates only in supported Build and Review routes.
8. Chat records the provider-native session identifier and provider-reported usage for every turn when those values are available.
9. Provider discovery is cached on the send path. Explicit refresh re-runs live probes.
10. A resumed provider session receives only the new turn. A provider switch receives only the most recent useful exchange, capped at 4 KiB.
11. Every Chat receipt records preflight, first-output, and total latency, plus quiet links to durable diagnostics.
12. Model selection is stored separately for Chat, Build, and Review.
13. During build, verification, review, or revision, the Ship composer becomes a read-only side chat attached to the current managed workspace. It can explain live state without modifying or steering the active phase. It closes during final promotion so no chat process can keep the worktree or snapshot clone open while The Staff Room cleans it up.
14. Provider-emitted reasoning summaries, commands, and tools appear as compact live timeline activity. The Staff Room never fabricates or exposes hidden chain-of-thought.
15. Ship progress appears inside the room conversation with the current Build, Verify, Review, Revise, Final review, and Promote stage plus normalized live activity. Raw provider diagnostics remain in durable logs.

## Risks and rollback

Autonomous Ship is off by default and is enabled once per project. The skill does not bypass isolation, verification, review, bounded retries, or safe promotion. Antigravity is excluded from Ask and Quick Edit because its print mode cannot provide the required read-only contract. For supported unattended Build and Review phases, The Staff Room combines Antigravity's permission bypass with its sandbox. Connection tests are explicit because they invoke a provider and may use the account. Provider terminal UIs are not embedded into Ask. Authentication failures are surfaced as connection attention.

## Verification

Rust tests cover Chat success semantics, the four provider output translators, provider usage shapes, provider mode selection, the shorter Chat idle limit, provider caching, bounded handoffs, project-scoped autonomy, and Ship-intent validation. The frontend production build type-checks the Enter-to-send composer and automatic Chat-to-Ship transition. Live two-turn smoke tests remain opt-in because they consume provider quota.
