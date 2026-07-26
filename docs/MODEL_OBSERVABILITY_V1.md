# Model and usage observability v1

## Outcome

Let a user save a model and optional reasoning-effort choice for each installed CLI, apply those choices to every phase, and inspect a durable execution receipt after the run.

## Non-goals

- Do not scrape provider account pages, local credentials, or desktop applications.
- Do not infer an actual model, token count, quota balance, price, or reset window when a provider does not report it.
- Do not create a generic remote billing integration.

## Acceptance criteria

1. Provider settings persist separate Model and Effort choices for Chat, Build, and Review.
2. Every provider presents the same route-control structure. Model choices show exact maintained identifiers, such as `claude-opus-4-8`, instead of ambiguous tier aliases.
3. Cursor and Antigravity refresh their signed-in account catalogues from their native model commands. Cursor's compound effort, context, thinking, and speed presets are grouped by base model because effort is selected separately. Codex and Claude use maintained exact identifiers because their installed CLIs do not expose a zero-spend account catalogue.
4. Codex effort maps to `model_reasoning_effort`, Claude and Antigravity use native `--effort` flags, and Cursor effort is encoded in its parameterized model value.
5. Build, review, revision, and final review receive the configured provider model and effort.
6. Every completed or failed provider phase persists a receipt with provider version, requested model, requested effort, session ID, Agent Room context size, reported usage fields, and an explicit availability note.
7. The evidence UI distinguishes Agent Room limits from provider-reported usage and quota information.

## Risks and rollback

Invalid provider model strings fail inside the owned provider process and preserve the managed worktree. Removing a profile returns that provider to its native default model. Database additions are additive.

## Verification

Rust tests cover receipt parsing. Frontend tests, production build, and Rust checks cover the integration. A disposable-repository smoke run should verify a selected Antigravity model and receipt after restarting the desktop app.
