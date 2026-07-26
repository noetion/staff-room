# Agent Room

Agent Room is a local-first desktop room that carries one engineering objective, compact project context, repository evidence, implementation results, and review findings between coding-agent CLIs.

The working v1 provides:

- instant project chat as the default route: Enter sends one selected CLI in the attached repository without creating a worktree;
- an optional project-scoped autonomous Ship skill that lets a chat agent propose a structured implementation objective, then hands control to Agent Room's isolated verification, review, and promotion route without routine approval prompts;
- an explicit Ship route for directly stated implementation objectives;
- Codex, Claude Code, Cursor Agent, and Antigravity automation probes that verify required flags from each installed CLI's live help output;
- cached provider discovery on the message path, with an explicit refresh control instead of repeating every CLI probe before every turn;
- Antigravity invocation with its managed worktree explicitly attached through `--add-dir`, plus its declared sandboxed unattended permission mode;
- explicit `@agent` selection with a deterministic fallback;
- a managed Git branch and worktree for every objective;
- provider-native streaming, cancellation, session capture, and revision resume where supported;
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
- policy-gated fast-forward promotion only when the base checkout is clean and unchanged;
- preserved worktrees on failure or attention states, with a bounded Resume recovery action;
- durable capped stdout/stderr logs and final responses under the app's local run-artifact directory;
- desktop notifications for completion, failure, stop, and required attention;
- SQLite-backed rooms, runs, activations, evidence, and provider session identifiers;
- a desktop Handoff Lens UI with keyboard navigation, labelled controls, accessible live/error regions, reduced-motion, increased-contrast, and solid-surface fallbacks.

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
