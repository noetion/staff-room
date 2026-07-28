# Agent Room

Agent Room is a local-first desktop room that carries one engineering objective, compact project context, repository evidence, implementation results, and review findings between coding-agent CLIs.

The working v1 provides:

- instant project working chat as the default route: Enter sends one selected CLI in the attached repository without creating a worktree, and explicit bounded edits can update the attached checkout directly;
- an optional project-scoped autonomous Ship skill that reserves the isolated verification, review, and promotion route for substantial or unattended implementation instead of simple file edits;
- an explicit Ship route for directly stated implementation objectives;
- Codex, Claude Code, Cursor Agent, and Antigravity automation probes that verify required flags from each installed CLI's live help output;
- cached provider discovery on the message path, with an explicit refresh control instead of repeating every CLI probe before every turn;
- Antigravity invocation with its managed worktree explicitly attached through `--add-dir`, plus its declared sandboxed unattended permission mode;
- explicit `@agent` selection with a deterministic fallback;
- isolated execution for every objective: a Git worktree for a clean checkout, or a private snapshot clone when current uncommitted files must be included;
- provider-native streaming, cancellation, session capture, and revision resume where supported, with provider-reported reasoning and tool activity normalized into the room timeline while raw diagnostics remain in durable logs;
- in-conversation Ship progress showing Build, Verify, Review, optional revision, final review, and Promote stages instead of relying on a detached status banner;
- read-only side chat against the active managed workspace while build, verification, review, or revision continues, so the user can ask what is happening without starting another run or interrupting the route; side chat closes during final promotion so the worktree or snapshot clone can be cleaned up safely;
- one compact four-provider settings grid with per-route Model and Effort controls for Chat, Build, and Review; exact maintained model IDs; live Cursor or Antigravity account discovery; and a grouped Cursor catalogue that does not repeat effort, context, thinking, and speed presets as separate models;
- bounded cross-provider Chat handoffs, while resumed native sessions receive only the new turn;
- durable per-phase execution receipts with preflight, first-output, and total latency; requested and provider-reported models; token/cost telemetry when emitted; and an explicit "not reported" state for provider quota and reset-window data;
- automatic project verification detected from `package.json` and Cargo manifests;
- independent cross-provider review when a second CLI is installed;
- a visible fresh-session same-provider review fallback when only one CLI is available;
- one bounded revision, two reviews, two recovery attempts, a 20-minute phase limit, and a five-minute idle-output limit;
- hierarchical root and nested `AGENTS.md` / `CLAUDE.md` discovery;
- objective-matched repository skills from `.codex/skills`, `.agents/skills`, `.claude/skills`, and `skills`, referenced by path so the provider reads the selected `SKILL.md` in full;
- a repository-owned autonomous Ship skill whose schema is validated by Agent Room before any implementation route can start;
- a measured 48 KiB context packet containing applicable instructions, selected skill references, memory, handoff, diff, and checks rather than the complete room transcript;
- schema-v1 JSON handoffs containing status, summary, changed files, checks, findings, and next action;
- policy-gated promotion: clean checkouts fast-forward, while dirty-checkout runs verify an exact workspace fingerprint and reapply only the reviewed agent delta without committing or stashing the user's work;
- preserved worktrees on failure or attention states, with a bounded Resume recovery action;
- restart reconciliation and coordinator-error finalisation, so an interrupted Ship run becomes recoverable instead of remaining permanently “Working”;
- a single desktop process per user, preventing a second launch from misclassifying an active run while focusing the existing window instead;
- process-scoped Git trust for managed worktrees and background-only Windows subprocesses, preventing dubious-ownership failures and unwanted console windows without changing global Git configuration;
- durable capped stdout/stderr logs and final responses under the app's local run-artifact directory;
- desktop notifications for completion, failure, stop, and required attention;
- SQLite-backed rooms, runs, activations, evidence, and provider session identifiers;
- a light-first Liquid Glass machine conversation UI with directional messages, ambient run-responsive aurora, keyboard navigation, labelled controls, accessible live/error regions, reduced-motion, increased-contrast, and solid-surface fallbacks.

## Run

```powershell
npm install
npm run tauri dev
```

Frontend-only visual preview:

```powershell
npm run dev
```

The browser preview never fabricates native execution.

## Verify

```powershell
npm test
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

## Product boundary

Agent Room is a desktop application, not a phone app, IDE, terminal multiplexer, cloud sandbox service, issue tracker, or generic workflow engine. It coordinates existing local coding CLIs to eliminate manual context transfer.

See [docs/V1_REVISED_PLAN.md](docs/V1_REVISED_PLAN.md) for the accepted v1 contract and [docs/PLAN_EVALUATION.md](docs/PLAN_EVALUATION.md) for the scope decision.
