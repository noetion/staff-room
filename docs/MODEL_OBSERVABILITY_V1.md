# Model and usage observability v1

## Outcome

Let a user save a model and optional reasoning-effort choice for each installed CLI, apply those choices to every phase, and inspect a durable execution receipt after the run.

## Non-goals

- Do not scrape provider account pages, local credentials, or desktop applications.
- Do not infer an actual model, token count, quota balance, price, or reset window when a provider does not report it.
- Do not create a generic remote billing integration.

## Acceptance criteria

1. Provider settings persist a model choice per CLI.
2. Antigravity discovers models from `agy models`; other providers accept an explicit model string when their CLI has no dependable discovery command.
3. Build, review, revision, and final review receive the configured provider model and effort where supported.
4. Every completed or failed provider phase persists a receipt with provider version, requested model, requested effort, session ID, Agent Room context size, reported usage fields, and an explicit availability note.
5. The evidence UI distinguishes Agent Room limits from provider-reported usage and quota information.

## Risks and rollback

Invalid provider model strings fail inside the owned provider process and preserve the managed worktree. Removing a profile returns that provider to its native default model. Database additions are additive.

## Verification

Rust tests cover receipt parsing. Frontend tests, production build, and Rust checks cover the integration. A disposable-repository smoke run should verify a selected Antigravity model and receipt after restarting the desktop app.
