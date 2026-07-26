# Agent Room — v1 Implementation Plan

**Audience:** the coding agent implementing this. You do not need to read the audit that produced it; everything required is below.
**Baseline commit:** `2d687a6` on `feature/liquid-glass-ui`.
**Definition of done:** Agent Room runs correctly as a local desktop build on Windows (`npm run tauri dev` and a local release build), passes the §9 acceptance matrix, and attaches to any Git repository the user picks. Installer signing, auto-update and distribution are explicitly out of scope.
**Warm persistent sessions are in scope for v1.**

---

## 0. Ground rules

Read these before writing any code. They exist because the current codebase failed on each of them.

1. **Never invent a CLI flag.** §2 is the complete, verified flag reference, taken from live `--help` output on the target machine. If you need a flag that is not in §2, run `<cli> --help` yourself, add it to §2 with the version it came from, and only then use it. Do not infer flags from documentation, training data, or another provider's interface.
2. **Never assert a capability from a substring match on help text.** The old code decided whether Claude could run unattended by searching help output for the literal `"auto"` with quotes. Capability comes from the declared table in §2, confirmed once per CLI version by a real turn, and cached.
3. **One step per commit — this is non-negotiable, including when implementing the whole plan in one session.** Each step in §5 ends with a green build: `npm test && npm run build && cargo test --manifest-path src-tauri/Cargo.toml && cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`. Do not start the next step with a red tree. If you are working through all 13 steps in one pass: commit after each, re-read this file's step section at the start of each step rather than holding the whole plan in context, and never batch two steps into one commit. Thirteen steps against an 8,098-line file with one commit at the end is unbisectable, and Step 2 alone touches every module.
4. **Prompts never enforce policy.** If the requirement is "this turn cannot write files", it is enforced by a sandbox flag or a tool allowlist. A sentence in the prompt is documentation, not a mechanism.
5. **The database is the only source of truth for run state.** React renders what the backend sends. It does not derive run state, does not compute routes, and never optimistically sets a terminal state.
6. **Persist before you emit.** Every run state transition writes to SQLite *before* the corresponding event reaches the webview.
7. **Record fixtures for every provider output shape you parse.** Capture real stdout to `src-tauri/tests/fixtures/<provider>/<case>.jsonl` and write the parser test against the file. Do not hand-write fixture JSON from memory.
8. **Do not touch `promote_worktree`, `workspace_fingerprint`, `create_isolation_at_root`, or their tests** except where a step explicitly says to. This is the one part of the system that is correct, and it is the part that can destroy a user's work if broken.
9. **Delete by symbol boundary, never by line range.** Ranges in this document are indicative and were not compiler-verified.

---

## 1. What the repository is today

| Fact | Value |
|---|---|
| Backend | `src-tauri/src/lib.rs` — **8,098 lines, one file** |
| Frontend | `src/App.tsx` — 2,013 lines; `src/styles.css` — 514; `model.ts`, `coordination.ts`, `native.ts`, `seed.ts` |
| Stack | Tauri 2, React 19, Vite 6, rusqlite (bundled SQLite), tokio |
| Tauri plugins | `notification`, `single-instance`. **No dialog plugin.** |
| Tests | 45 Rust tests in `lib.rs:6835+`; 8 frontend tests in `coordination.test.ts` |
| DB | `<app_data>/agent-room.db`, WAL, FK on |
| Artifacts | `<app_local_data>/runs/<run_id>/{build,review,revise,final-review,chat}.{stdout,stderr}.log` + `*.final.txt` |
| Worktrees | `<app_cache>/worktrees/<short_run_id>` |

**The five defects this plan exists to fix:**

1. No repository attachment. `get_environment` (`lib.rs:4166`) infers the repo from `std::env::current_dir()`; the project ID is the hardcoded literal `"agent-room"` (`lib.rs:4185`, `seed.ts:4`). A packaged build attaches nothing and shows seeded fake data.
2. Every turn spawns a new CLI process (`lib.rs:2367`). No warm sessions anywhere. The only session subsystem is unreachable behind `EMBEDDED_TUI_CHAT_ENABLED: bool = false` (`lib.rs:47`).
3. Ship's verification runs `npm run build` / `cargo check` in a worktree with no `node_modules` or `target/` and no install step (`lib.rs:3776`), so it fails for environmental reasons. Separately, `verification_passed` (`lib.rs:3910`) treats "no checks found" as success, so repos without npm/cargo auto-promote unverified.
4. Chat writes to the user's live checkout with no gate (`lib.rs:5078`); the only restraint is prompt text.
5. UI state is derived client-side. Stop reports success regardless of outcome (`App.tsx:1630`); there is no Abandon control; `load_room` fetches the **oldest** 200 messages (`lib.rs:4476`) so long rooms freeze.

---

## 2. Verified provider reference

Captured from live `--help` on the target machine. **This is the authoritative source for command construction.** Versions: `codex-cli 0.144.4`, `claude 2.1.220`, `cursor-agent 2026.07.23-e383d2b`, `agy 1.1.7`.

### 2.0 Provider tiers — read this first

The four CLIs are **not** equally capable, and pretending otherwise is what forced the current codebase to bend its shared path around its weakest member. v1 defines two tiers explicitly and shows them in the UI.

| Tier | Providers | Available in |
|---|---|---|
| **Full** | Codex, Claude, Cursor | Ask, Quick Edit, Ship (builder + reviewer) |
| **Ship-only** | Antigravity | Ship (builder + reviewer) only |

**Why Antigravity is Ship-only.** `agy` exposes only `accept-edits` and `plan` modes — **there is no read-only mode**. It also has no structured output, no usage reporting, no token deltas and no warm transport. Those are fatal for Ask (where the guarantee is "this turn cannot write") and irrelevant for Ship (where write access is the point, isolation is real, and the handoff is a marker block that parses identically regardless of streaming).

Keeping it in Ship preserves the only Gemini path in the product. Removing it from Ask removes the reason the shared code was compromised.

**What this deletes from the plan** (all of it complexity that existed only to make Antigravity survive Chat):

- the Antigravity-only retry-with-a-fresh-session branch (`lib.rs:5121–5155`) — deleted in Step 1;
- the 75-second `CHAT_IDLE_TIMEOUT_SECONDS` hack, which exists because a cold text-only provider can be silent that long — Chat's idle limit is now set by the three streaming providers;
- the live-test gate that previously blocked Step 8 (former unknown #2);
- four cells in the acceptance matrix, which become honest `n/a` rather than aspirational checkboxes.

**Reviewer integrity is handled generally, not per-provider.** Because Antigravity can review but cannot prove read-only by flag, Step 9 adds a post-review mutation guard that applies to **all four** providers: snapshot the worktree before the review phase, compare after, fail the phase if the reviewer changed anything. This is strictly better than trusting any provider's read-only flag, and it removes the need to tier the review role separately.

### 2.1 Codex

```
codex exec [OPTIONS] [PROMPT]          # prompt from stdin when PROMPT is "-" or omitted
codex exec resume [OPTIONS] <SESSION>  # exact resume
codex app-server [--stdio]             # [experimental] persistent JSON-RPC over stdio
codex mcp-server                       # persistent MCP server over stdio (stabler than app-server)
```

| Need | Flag | Notes |
|---|---|---|
| Prompt transport | stdin, pass `-` as PROMPT | **Use stdin. Never argv.** |
| Structured events | `--json` | on `exec` only |
| Final message file | `-o, --output-last-message <FILE>` | on `exec` only |
| **Schema-enforced output** | `--output-schema <FILE>` | on `exec`. **Use this instead of marker parsing.** |
| Approvals | `-a, --ask-for-approval never` | values: `untrusted`, `on-request`, `never` |
| Sandbox | `-s, --sandbox <read-only\|workspace-write\|danger-full-access>` | real enforcement |
| Working root | `-C, --cd <DIR>` | |
| Extra writable dirs | `--add-dir <DIR>` | |
| Model | `-m, --model <MODEL>` | |
| Effort | `-c model_reasoning_effort="<level>"` | config override, not a flag |
| Env inherit | `-c shell_environment_policy.inherit=all` | |
| Resume | `codex exec resume <SESSION_ID>` | |

**Do not use:** `--ephemeral` (kills resume), `--dangerously-bypass-approvals-and-sandbox`.
**Session ID:** cannot be pre-assigned; parse it from the `--json` stream.
**Warm session:** `app-server` is marked `[experimental]` in this version. `mcp-server` is the stabler stdio option. See Step 7 for the decision procedure and the mandatory fallback.

### 2.2 Claude Code

```
claude -p [OPTIONS] [prompt]
```

| Need | Flag | Notes |
|---|---|---|
| Non-interactive | `-p, --print` | **Skips the workspace-trust dialog automatically** |
| Prompt transport | stdin | |
| Structured stream | `--output-format stream-json --verbose` | |
| Token deltas | `--include-partial-messages` | requires `--print` + `stream-json` |
| **Streaming input (warm)** | `--input-format stream-json` | **This is the warm-session transport** |
| Input acknowledgement | `--replay-user-messages` | requires both formats `stream-json` |
| **Assign session ID** | `--session-id <uuid>` | **We generate it. No parsing needed.** |
| Resume | `-r, --resume <id>` | |
| Fork for fresh review | `--fork-session` | use with `--resume` |
| Permission mode | `--permission-mode <acceptEdits\|auto\|bypassPermissions\|manual\|dontAsk\|plan>` | |
| **Hard tool allowlist** | `--tools "Read,Grep,Glob"` | **Real read-only enforcement** |
| Model | `--model <alias\|full-id>` | aliases `opus`, `sonnet`, `fable` are stable; full IDs like `claude-fable-5` |
| Effort | `--effort <low\|medium\|high\|xhigh\|max>` | |
| **Schema-enforced output** | `--json-schema <schema>` | **Use instead of marker parsing** |
| Cost ceiling | `--max-budget-usd <amount>` | `--print` only. Expose in settings. |
| Cache-friendly prompt | `--exclude-dynamic-system-prompt-sections` | moves per-machine sections out of the system prompt |
| Stable contract text | `--append-system-prompt <prompt>` | put the handoff contract here, not in the user packet |
| Extra dirs | `--add-dir <dirs...>` | |

**`--max-turns` does not exist in 2.1.220.** `lib.rs:2433–2440` conditionally passes it based on a `capability_proof` string that `capabilities_for` never produces — dead code, delete it.
**Do not use:** `--no-session-persistence`, `--worktree` (we own isolation), `--dangerously-skip-permissions`.

### 2.3 Cursor Agent

```
cursor-agent -p [OPTIONS] [prompt...]
cursor-agent create-chat     # prints a new chat ID
cursor-agent models          # account model list
```

| Need | Flag | Notes |
|---|---|---|
| Non-interactive | `-p, --print` | help states print mode "has access to all tools, including write and shell" — **write is the default** |
| Prompt transport | positional `[prompt...]` | **argv only — see the Windows limit below** |
| Structured stream | `--output-format stream-json` | |
| Token deltas | `--stream-partial-output` | requires `--print` + `stream-json` |
| Read-only | `--mode ask` (or `--mode plan`) | only `plan` and `ask` are valid; there is no write value |
| Sandbox | `--sandbox <enabled\|disabled>` | |
| Write | `-f, --force` (alias `--yolo`) | |
| **Workspace trust** | `--trust` | **required for headless runs in a fresh worktree** |
| Workspace root | `--workspace <path>`, `--add-dir <path>` | prefer over relying on cwd |
| Resume | `--resume [chatId]` | |
| **Pre-allocate session** | `cursor-agent create-chat` → chat ID | **Use this; do not parse the ID out of the stream** |
| Model + effort | `--model 'base[context=1m,effort=high,fast=false]'` | effort is encoded in the bracket; **there is no `--effort` flag** |
| Model list | `cursor-agent models` | |

**No warm transport exists.** Per-turn spawn is the truthful v1 contract.
**Do not use:** `-w/--worktree`.

### 2.4 Antigravity (`agy`) — **Ship-only tier**

```
agy --print "<prompt>" [OPTIONS]
agy models
```

Go-style flag parser; `--print` **takes the prompt as its value**.

| Need | Flag | Notes |
|---|---|---|
| Non-interactive | `--print "<prompt>"` (alias `--prompt`) | argv only |
| Print timeout | `--print-timeout <duration>` | Go duration, default `5m0s` |
| Sandbox | `--sandbox` | terminal restrictions |
| Permission bypass | `--dangerously-skip-permissions` | **Build and Revise only. Never Review.** |
| Mode | `--mode <accept-edits\|plan>` | **no read-only/ask mode exists** — `accept-edits` for build/revise, `plan` for review |
| Workspace | `--add-dir <DIR>` (repeatable) | |
| Resume | `--conversation <ID>` | also `-c/--continue` for most recent |
| Model | `--model <name>` | |
| Effort | `--effort <low\|medium\|high>` | |
| Model list | `agy models` | |

**No structured output, no usage reporting, no token deltas.** Completion-stream only. Display this as a capability downgrade; do not paper over it.

**Antigravity has no true read-only mode, which is why it is Ship-only (§2.0).** It is never offered for Ask or Quick Edit, so no read-only guarantee is ever claimed for it. As a reviewer it runs `--sandbox --mode plan` **without** the permission bypass, and the universal post-review mutation guard in Step 9 catches any write it makes inside the worktree.

### 2.5 Cross-provider matrix

| | Codex | Claude | Cursor | Antigravity |
|---|---|---|---|---|
| **Tier** | Full | Full | Full | **Ship-only** |
| Prompt transport | **stdin** | **stdin** | argv | argv |
| Warm session possible | experimental | **yes** | no | no |
| Session ID | parse from stream | **we assign** | **pre-allocate** | parse / `--continue` |
| Token deltas | yes | yes | yes | **no** |
| Usage reported | yes | yes | **no** | **no** |
| Read-only mechanism | `-s read-only` | `--tools` allowlist + `plan` | `--mode ask --sandbox enabled` | **none — not offered for Ask** |
| Schema output | `--output-schema` | `--json-schema` | markers | markers |
| Model list command | none | none | `models` | `models` |

### 2.6 The Windows argv limit — must be fixed

Cursor (`lib.rs:2469`) and Antigravity (`lib.rs:2495`) receive the prompt as a command-line argument. Windows `CreateProcess` caps the command line at 32,767 UTF-16 characters. The build context budget is 48 KiB (`lib.rs:32`). **Ship builds through Cursor or Antigravity can therefore fail to spawn with OS error 206.**

Required fix, implemented in Step 5:

- Write the packet to `<artifact_dir>/<phase>.prompt.txt`.
- Codex and Claude: pipe the file to stdin (unchanged behaviour, no limit).
- Cursor and Antigravity: pass a short argv prompt that instructs the agent to read the packet file as its first action, e.g. `Read the file at <abs path> in full. It contains your complete assignment. Follow it exactly.`
- Hard-assert in code that any argv-delivered prompt is under 8,000 characters, and fail with a clear error rather than letting `CreateProcess` fail opaquely.

---

## 3. Target architecture

```
src-tauri/src/
  main.rs
  lib.rs                  # tauri::Builder wiring, state registration, generate_handler! only
  db/
    mod.rs                # Connection wrapper, transactions
    schema.rs             # migrate(), versioned
    migrations.rs         # v1→v2 project-id migration (Step 4)
    models.rs             # row structs
  git/
    mod.rs
    workspace.rs          # MOVE UNCHANGED: fingerprint, isolation, promote, discard
    verify.rs             # prepare + run + status (Step 10)
  providers/
    mod.rs                # ProviderAdapter trait, TurnEvent, Capabilities, Mode
    capabilities.rs       # declared table + version-keyed handshake cache
    codex.rs
    claude.rs
    cursor.rs
    antigravity.rs
    parse.rs              # shared JSON helpers + usage merge
  session/
    mod.rs                # SessionManager: warm processes + durable native ids, idle GC
  run/
    mod.rs                # RunCoordinator state machine
    context.rs            # ContextBuilder: budgets, resume-aware dedup, truncation
  events.rs               # EventBus: coalescing, RunSnapshot, TurnEvent → webview
  commands/
    mod.rs                # every #[tauri::command], thin
src/
  App.tsx                 # shell only
  components/             # Timeline, Composer, RunCard, DiffSheet, Inspector, Settings, ProjectSwitcher
  state/                  # useRoom, useRun, useProviders
  native.ts               # invoke wrappers
  model.ts                # types
```

**Ownership:** only `session` spawns/kills provider processes. Only `git/workspace` touches Git. Only `run` writes the `runs` table. `commands` orchestrates nothing.

**Run state machine** — every edge persisted before the event is emitted:

```
preparing ─┬─▶ building ─▶ verifying ─▶ reviewing ─┬─▶ promoting ─▶ complete
           │                                       └─▶ revising ─▶ verifying ─▶ reviewing
           └─▶ failed
any active ─▶ stopped | waiting | failed | abandoned
stopped|waiting|failed ─▶ preparing        (≤2 recoveries, resumes at last completed phase)
promoting interrupted ─▶ waiting           (NEVER auto-resumable)
```

---

## 4. Step index

**Step numbers are stable identifiers, not execution order.** Step 2 is deferred
to second-to-last (see its section for why). Nothing depends on it.

**Execution order: 1 → 3 → 4 → 5 → 6 → 7 → 8 → 9 → 10 → 11 → 12 → 2 → 13.**

| # | Step | Run | Blocking for v1 | Depends on |
|---|---|---|---|---|
| 1 | Delete dead code | 1st | yes | — |
| 2 | Split `lib.rs` into modules | **12th** | yes | 12 |
| 3 | Measurement columns | 2nd | yes | 1 |
| 4 | Real repository attachment | 3rd | yes | 1 |
| 5 | Adapter trait + verified command construction | 4th | yes | 1 |
| 6 | TurnEvent delta streaming | 5th | yes | 5 |
| 7 | SessionManager + warm Claude + warm Codex | 6th | yes | 5, 6 |
| 8 | Ask / Quick Edit / Ship modes | 7th | yes | 5 |
| 9 | RunCoordinator + abandon + truthful stop | 8th | yes | 4 |
| 10 | Verification prepare + `unavailable` status | 9th | yes | 9 |
| 11 | Context deduplication | 10th | yes | 7, 9 |
| 12 | UI corrections | 11th | yes | 6, 9 |
| 13 | Acceptance run | **13th** | yes | all |

---

## 5. The steps

---

### Step 1 — Delete dead code

**Goal.** Remove everything unreachable so the module split in Step 2 is smaller and honest.

**Why.** `EMBEDDED_TUI_CHAT_ENABLED: bool = false` (`lib.rs:47`) makes an entire PTY subsystem unreachable, but `stop_run` still walks its data structures on every call, and five tests assert its behaviour.

**Delete:**

| Location | What |
|---|---|
| `lib.rs:47` | `EMBEDDED_TUI_CHAT_ENABLED` const |
| `lib.rs:4900–5035` | the whole `if EMBEDDED_TUI_CHAT_ENABLED { ... }` block in `start_room_chat` |
| `lib.rs:61–70` | `struct InteractiveSession` |
| `lib.rs:57–58` | `RuntimeState.interactive_sessions`, `.interactive_runs` |
| `lib.rs:2774–3177` | `terminal_session_key`, `antigravity_chat_response`, `terminal_screen_needs_input`, `antigravity_response_is_complete`, `interactive_command`, `spawn_interactive_session`, `interactive_session_has_ended`, `close_interactive_session`, `terminal_key_sequence`, `apply_interactive_provider_environment` |

| `lib.rs:4654–4678` | `send_terminal_key` command + its entry in `generate_handler!` (`lib.rs:6828`) |
| `lib.rs:4629–4652` | the interactive-session branch inside `stop_run` |
| `lib.rs:2433–2440` | the `--max-turns` branch — the flag does not exist in Claude 2.1.220 and the `capability_proof` string it tests for is never generated |
| `lib.rs:5121–5155` | the Antigravity-only retry-with-a-fresh-session branch in `start_room_chat` — Antigravity is Ship-only per §2.0 and never reaches the Chat path |
| `Cargo.toml:25–26` | `portable-pty`, `vt100` |
| `lib.rs:6996–7121` | tests `native_terminal_parser_replaces_redrawn_content`, `antigravity_terminal_screen_extracts_only_the_current_chat_response`, `onboarding_screen_remains_an_interactive_terminal`, `terminal_input_allows_navigation_and_one_typed_character`, `windows_native_pty_remains_alive_for_two_turns` |
| `src/coordination.ts` | `requestReview`, `canRevise`, `isTerminal`, `contextPacketSize` only. Verify that each has no non-test callers before deletion. Keep `createRun`, `routeForPhase`, `selectParticipant`, `explicitAgent`, `participantCanChat`, `participantIsRunnable`, and `selectChatParticipant` through Step 8; Step 9 deletes that remaining client-side derivation together with its App call sites. |
| `src/coordination.test.ts` | reduce to the surviving selection predicates: `explicitAgent`, `participantCanChat`, `participantIsRunnable`, and `selectChatParticipant`. Delete only the simulation-layer tests. |
| `App.tsx:351–375` | Delete `visibleMessageReason`, then replace its call site with `const reason = message.reason;` so the timeline continues to render the stored backend reason unchanged. |

**Autonomy amendment.** This supersedes the Stop and Ask rules for this class of issue.

Diagnose before remedying. If a gate fails on code you did not intend to modify, `git diff` it first. The most likely cause is that the change clipped or shifted adjacent code, not that pre-existing code was broken.

Self-correct and continue, without asking, when:

1. A plan deletion target has live callers. Keep it, log the deferral and the step that should remove it, then continue.
2. A plan line range clips adjacent code. Delete by symbol boundary instead, log it, then continue.
3. A plan reference does not match the code. Locate by symbol, implement the intent, log it, then continue.
4. A deletion breaks a build or gate in an unanticipated way. Use the smallest change that implements the plan’s intent while keeping all four gates green, log it, then continue.

Log every deviation in `docs/PLAN_DEVIATIONS.md` as: step, what the plan said, what you did, why. This file is a deliverable.

Still stop and ask for a database migration whose row counts cannot be verified, any change to `promote_worktree`, `workspace_fingerprint`, `create_isolation_at_root`, or `discard_isolation`, weakening/skipping/reinterpreting a verification gate, a required flag unavailable from the verified §2 reference and live `--help`, or anything irreversible, destructive, or affecting data outside the repository.

**Acceptance.** `cargo tree` no longer lists `portable-pty` or `vt100`. `~600` lines gone. Build and remaining tests green.

**Verification.** `cargo clippy --all-targets -- -D warnings` reports no dead-code or unused-import warnings.

**Risk.** None — nothing deleted is reachable.

---

### Step 2 — Split `lib.rs` into modules  ·  **DEFERRED: RUN 12th, NOT 2nd**

> **Do not run this step second.** Run it after Step 12 and before Step 13.
>
> It was originally placed here on the reasoning that a huge file is unsafe for an
> agent with a bounded context window. Two attempts proved the opposite: *deciding*
> module boundaries requires whole-file comprehension (~100k tokens before a line is
> written), while every other step is a targeted edit to named functions that never
> needs the whole file. Both attempts stalled after extracting one module.
>
> It also fails this plan's own test that each step produce a user-verifiable
> improvement: it changes nothing a user can see and costs the most context of any step.
>
> Nothing downstream depends on it. Steps 3–12 land in `lib.rs`; the file grows to
> roughly 9k lines before this step shrinks it. That is ugly and harmless.
>
> **When you do run it:** extract with PowerShell line slicing
> (`$l = Get-Content lib.rs; $l[453..698] | Set-Content db/schema.rs`). Never load a
> file into model context to move it. Run `cargo check` after each extraction and fix
> only the symbols the compiler names. The compiler is the verification mechanism;
> reading the moved code is not.

**Goal.** Produce the layout in §3. Pure code motion; zero behaviour change.

**Why.** An 8,098-line file cannot be safely modified by an agent with a bounded context window, and every later step touches it.

**How.** Move in this order, compiling between each: `db` → `git` (move `workspace.rs` functions **verbatim**, including their tests) → `providers/parse.rs` → `providers/*.rs` → `session` → `run` → `events` → `commands`. Keep `#[tauri::command]` functions in `commands/mod.rs` with the same names and signatures so `generate_handler!` is unchanged.

**Acceptance.** No file exceeds 800 lines except `run/mod.rs` (which Step 9 will decompose). All 40 remaining Rust tests pass unchanged. `git diff --stat` shows moves, not rewrites.

**Verification.** Full check suite. Manually confirm `cargo test` runs the same test names as before, minus the five deleted in Step 1.

**Risk.** Mechanical but wide. Do it as one commit. Do not "improve" anything while moving — behaviour changes hidden inside a move are the classic failure here.

---

### Step 3 — Measurement columns

**Goal.** Every turn and every phase records latency, tokens and context reuse, so later steps are provable rather than asserted.

**Why.** `execution_receipts` has no latency columns at all, and `load_room` returns `preflight_ms`/`first_output_ms`/`total_ms` as hardcoded `None` (`lib.rs:4560–4570`). The README claims per-phase latency receipts; that claim is currently false for Ship.

**Changes.**

1. `db/schema.rs` — additive `ALTER TABLE` (the existing `let _ = execute(...)` idempotent pattern is fine):

```sql
ALTER TABLE execution_receipts ADD COLUMN preflight_ms INTEGER;
ALTER TABLE execution_receipts ADD COLUMN process_start_ms INTEGER;
ALTER TABLE execution_receipts ADD COLUMN first_output_ms INTEGER;
ALTER TABLE execution_receipts ADD COLUMN total_ms INTEGER;
ALTER TABLE execution_receipts ADD COLUMN session_resumed INTEGER NOT NULL DEFAULT 0;
ALTER TABLE execution_receipts ADD COLUMN packet_bytes_saved INTEGER NOT NULL DEFAULT 0;
ALTER TABLE execution_receipts ADD COLUMN stdout_log_path TEXT;
ALTER TABLE execution_receipts ADD COLUMN stderr_log_path TEXT;
ALTER TABLE chat_receipts      ADD COLUMN process_start_ms INTEGER;
ALTER TABLE chat_receipts      ADD COLUMN session_resumed INTEGER NOT NULL DEFAULT 0;
ALTER TABLE chat_receipts      ADD COLUMN packet_bytes_saved INTEGER NOT NULL DEFAULT 0;
```

2. Time `command.spawn()` separately from first stdout line. Add `process_start_ms` to the provider result struct.
3. Populate all fields in `persist_receipt`; stop hardcoding `None` in `load_room`.
4. Add an Evidence table in the Inspector showing phase · provider · model · context bytes · saved bytes · preflight · process start · first output · total · tokens · cost.

**`packet_bytes_saved`** = bytes that *would* have been sent had the session not been resumed, minus bytes actually sent. Until Step 11 it is 0; the column exists now so the schema does not move later. This is the number that proves the product's thesis — "you did not paste this again".

**Acceptance.** After one Chat turn and one Ship run, every receipt row has non-null `preflight_ms`, `process_start_ms`, `first_output_ms`, `total_ms`.

**Verification.** Rust test: insert a receipt with all fields, read it back through `load_room`, assert equality. Manual: read the Evidence tab.

---

### Step 4 — Real repository attachment

**Goal.** A user opens Agent Room, picks any Git repository, and gets a room scoped to it. Multiple repositories coexist and are independently scoped.

**Why.** This is the defect that makes the product unusable by anyone but its author. `get_environment` infers the repo from the process working directory, so `tauri dev` silently attaches Agent Room's own repo and a packaged build attaches nothing. The project ID is a compile-time constant, so all rooms, sessions, model profiles and settings share one row set.

**Changes.**

1. **Add the dialog plugin.** `Cargo.toml`: `tauri-plugin-dialog = "2"`. `package.json`: `@tauri-apps/plugin-dialog`. Register in `lib.rs`. Add `"dialog:allow-open"` to `capabilities/default.json`.

2. **New commands** in `commands/mod.rs`:

```rust
project_pick()                -> Option<String>   // folder dialog, returns path
project_attach(path: String)  -> Project          // canonicalise, validate, insert, set active
project_list()                -> Vec<Project>
project_select(id: String)    -> Project
project_active()              -> Option<Project>
```

`project_attach`: canonicalise the path; run `git rev-parse --show-toplevel` and use its result (so attaching a subdirectory attaches the repo root); reject with a clear message if it is not a Git repository; compute `project_id = hex(sha256(canonical_root))[..16]`; `INSERT OR IGNORE`; store as active in a new `app_state` table.

3. **Rewrite `get_environment`.** Delete the `std::env::current_dir()` inference entirely. Return `{ attached: false }` when there is no active project; otherwise return the active project's path, current branch and participants scoped to its ID. Remove the hardcoded `"agent-room"` literal.

4. **Frontend.** Add an attach empty state (headline, "Choose a repository" button, recent-projects list). Add a project switcher to the left rail. **Gate every `seed*` import behind `import.meta.env.DEV`** — seeded data must never be production initial state. Remove `seedProject.repositoryPath`'s hardcoded `<user-home>\...`.

5. **Migration v1→v2** in `db/migrations.rs`:

```sql
-- Back up agent-room.db to agent-room.db.v1.bak FIRST.
-- Run inside one transaction with PRAGMA foreign_keys = OFF.
-- :hashed = hex(sha256(canonicalised projects.repository_path))[..16]
UPDATE projects               SET id         = :hashed WHERE id         = 'agent-room';
UPDATE messages               SET project_id = :hashed WHERE project_id = 'agent-room';
UPDATE runs                   SET project_id = :hashed WHERE project_id = 'agent-room';
UPDATE chat_sessions          SET project_id = :hashed WHERE project_id = 'agent-room';
UPDATE provider_sessions      SET project_id = :hashed WHERE project_id = 'agent-room';
UPDATE provider_profiles      SET project_id = :hashed WHERE project_id = 'agent-room';
UPDATE provider_route_profiles SET project_id = :hashed WHERE project_id = 'agent-room';
UPDATE project_settings       SET project_id = :hashed WHERE project_id = 'agent-room';
UPDATE provider_connections   SET project_id = :hashed WHERE project_id = 'agent-room';
UPDATE chat_receipts          SET project_id = :hashed WHERE project_id = 'agent-room';
```

Skip the migration entirely (leave rows untouched) if `projects.repository_path` is missing or is not a readable Git repository. Verify row counts before and after; roll back on mismatch.

**Acceptance.**
- Fresh install, launched from an arbitrary directory, shows the attach screen with zero seeded messages.
- Attaching two unrelated repositories produces two rooms with independent history, chat sessions, model profiles, autonomy settings and provider connections.
- Switching projects swaps the entire room without a restart.
- An existing v1 database migrates with all messages and runs intact.

**Verification.** Rust test: attach two temp Git repos, write a message to each, assert `load_room` returns one message per project. Rust test: migration on a fixture DB preserves row counts. Manual: build release, run from `C:\`, attach a repo.

**Risk.** The migration is the only destructive operation in this plan. Back up first, verify counts, and test against a copy of a real database before shipping.

---

### Step 5 — Adapter trait + verified command construction

**Goal.** Each provider owns its own command construction, using only the flags in §2. Capability comes from a declared table confirmed once per CLI version.

**Why.** All four providers are currently built in one `match` inside `invoke_provider` (`lib.rs:2370–2497`), and readiness is decided by substring-matching `--help` output (`lib.rs:826–1029`). A cosmetic help-text change disables a provider.

**Changes.**

1. **`providers/mod.rs`:**

```rust
pub enum Mode { Ask, QuickEdit, Ship }

pub enum TurnEvent {
    Started { provider: String, session_id: Option<String> },
    Activity { kind: ActivityKind, label: String, detail: String },
    TextDelta(String),
    Usage(ProviderUsage),
    InteractionRequired { reason: String },
    Terminal(Outcome),          // Completed | Failed(String) | Stopped
}

pub struct Capabilities {
    pub warm_session: bool,
    pub token_deltas: bool,
    pub usage_reporting: bool,
    pub assignable_session_id: bool,
    pub preallocatable_session: bool,
    pub schema_output: bool,
    pub true_read_only: bool,
    pub prompt_via_stdin: bool,
}

#[async_trait]
pub trait ProviderAdapter: Send + Sync {
    fn kind(&self) -> &'static str;
    fn declared_capabilities(&self) -> Capabilities;
    fn build_command(&self, req: &TurnRequest) -> Result<PreparedCommand, String>;
    fn parse_line(&self, line: &str, st: &mut ParseState) -> Vec<TurnEvent>;
}
```

2. **Move command construction into `providers/{codex,claude,cursor,antigravity}.rs`**, exactly per §2. Specifically:
   - Codex, Claude → `PreparedCommand { stdin: Some(packet), .. }`
   - Cursor, Antigravity → write the packet to `<artifact_dir>/<phase>.prompt.txt`, pass the short read-this-file instruction as argv
   - **Assert argv prompts are < 8,000 chars; return a clear error otherwise** (§2.6)
   - Add `--trust` to every Cursor invocation (required for headless runs in a fresh worktree)
   - Use `--session-id <uuid>` for Claude (we generate it) and `cursor-agent create-chat` for Cursor (pre-allocate)
   - Move the handoff contract into Claude's `--append-system-prompt` rather than the user packet

3. **`providers/capabilities.rs`:** new table `provider_capabilities(provider, version, capabilities_json, verified_at)`. On first sight of a `(provider, version)` pair run one handshake — `--version`, then one real minimal turn — and persist. Never re-probe for that version. Invalidate on version change or launch failure. **Delete `has_help` and all per-launch `--help` probing.**

4. **Loosen `connection_test_ready`** (`lib.rs:1941`): currently requires the entire reply to equal `READY`, so `READY.` fails and permanently blocks Ship. Change to a case-insensitive `contains("ready")` after stripping punctuation and markdown fences, and treat any successful Ask turn as a connection proof so the two gates agree.

5. **Fix model catalogues** (`lib.rs:1042–1113`). The hardcoded IDs are **correct as written** — `gpt-5.6-sol`, `gpt-5.6-terra`, `gpt-5.6-luna` (OpenAI, 9 July 2026) and `claude-opus-5` / `claude-fable-5` are real. The defect is narrower than "wrong values":
   - **Refresh is a lie for Codex and Claude.** `discover_provider_models` (`lib.rs:4805–4810`) re-returns the same hardcoded array while the note claims "Refresh keeps this local catalogue current." Either make it a no-op with honest copy, or drop the Refresh button for those two.
   - **Keep the hardcoded list as a seeded default**, add a free-text field with last-used history so a new model works the day it ships without a rebuild, and add Claude's stable aliases (`opus`, `sonnet`, `fable`).
   - Cursor and Antigravity keep their real `models` subcommands (already wired at `lib.rs:4770`).
   - **Antigravity passes display names to `--model`** — `"Gemini 3.1 Pro (high)"`, with spaces and parentheses (`lib.rs:2485`). Verify against `agy models` output whether `--model` wants that string or a slug; if it wants a slug, map display name → slug.
   - **Codex effort options are incomplete** (`lib.rs:1031`): the list is `["low","medium","high","xhigh"]`, but GPT-5.6 Sol also supports `max` and `ultra`. Add both, and gate them to Sol so they are not offered for Terra or Luna.

**Acceptance.** Cold start performs zero provider subprocesses when cached capability records match the installed versions. A 48 KiB build packet reaches Cursor and Antigravity without a spawn failure. Renaming a flag's description in help output does not disable any provider.

**Verification.** Rust test per provider asserting exact argv for Ask, Quick Edit and Ship modes (golden-string tests). Rust test asserting an argv prompt over 8,000 chars returns an error rather than spawning. Rust test: second `capabilities()` call after a version match spawns nothing.

---

### Step 6 — TurnEvent delta streaming

**Goal.** Chat streams token by token with IPC traffic linear in answer size.

**Why.** The current stdout loop re-emits the **entire accumulated answer** (truncated to 16 KiB) on every stdout line (`lib.rs:2612–2624`). With Claude's `--include-partial-messages` that is one full-snapshot event per token delta — quadratic IPC. The frontend then rebuilds the whole message array per event (`App.tsx:1146–1168`).

**Changes.**

1. Replace snapshot emission with `TurnEvent::TextDelta(fragment)`.
2. `events.rs`: coalesce deltas on a ~50 ms timer so a fast provider cannot flood the webview.
3. Frontend: accumulate into a `useRef<string>` buffer, flush to state on `requestAnimationFrame`.
4. Coalesce Claude `thinking_delta` into one growing activity block instead of one activity item per delta (current behaviour at `lib.rs:2161+` produces fragment spam).
5. Persist the final answer once, as today.
6. Cap the timeline-stored answer at 32 KiB, not 16 KiB. **Note:** the current 16 KiB cap (`lib.rs:2716`) can truncate away a trailing Ship-intent marker before `extract_ship_intent` runs at `lib.rs:5255`. Extract markers from the full text, then truncate for display.

**Acceptance.** A 20 KB Claude answer produces total emitted bytes O(n) in answer length. The timeline does not stutter during streaming.

**Verification.** Rust test: feed N fixture stdout lines, assert (a) concatenated deltas equal the final answer and (b) **total emitted bytes < 2× answer length** — the current implementation fails (b) by design. Manual: long answer under a frame-rate check.

---

### Step 7 — SessionManager + warm sessions

**Goal.** Second and subsequent turns to Claude — and to Codex where its app-server proves stable — skip process startup entirely.

**Why.** This is the product's core speed claim. Today every turn spawns a process (`lib.rs:2367`), so Agent Room is structurally slower than an already-open CLI.

**Changes.**

1. **`session/mod.rs`.** `SessionManager` keyed `(project_id, provider, purpose)` where purpose ∈ `{ask, quick_edit, build, review}`. Holds the durable native session ID (SQLite) and an optional warm child process (memory). **A dead process with a live session ID is a recoverable state:** recreate the process, resume the session, never replay the transcript. Idle GC at 10 minutes. **Explicitly close every session on app shutdown** so no child outlives the app.

2. **Claude warm transport — implement first, highest confidence.**

```
claude -p --input-format stream-json --output-format stream-json \
       --include-partial-messages --replay-user-messages \
       --session-id <uuid-we-generate> --verbose [--model X] [--effort Y] \
       [--append-system-prompt <contract>] [--tools ...] [--permission-mode ...]
```

Hold stdin open across turns; write one JSON message per turn; read events continuously. Every flag here is confirmed in §2.2.

3. **Codex warm transport — decision procedure, do not guess.**
   `codex app-server` is marked `[experimental]` in 0.144.4. Before implementing:
   a. Run `codex app-server --stdio`, capture the handshake, and save it to `src-tauri/tests/fixtures/codex/app-server-handshake.jsonl`.
   b. If the protocol is stable enough to send a turn and resume a thread, implement it behind a capability flag.
   c. **If it is not, do not force it.** Fall back to the proven `codex exec resume <id>` per-turn path and set `warm_session: false` for Codex. Also evaluate `codex mcp-server` (stdio, non-experimental) as the alternative warm transport.
   In all three cases the UI shows Codex's real warm status. Do not claim warm sessions you did not implement.

4. **Cursor and Antigravity keep per-turn spawn** and are labelled "cold start per turn" in the capability chips. No warm transport exists in their CLIs.

5. **Cancellation.** One `CancellationToken` per run and per turn, registered **before any I/O** so Stop is never a no-op, held by `SessionManager`.

**Acceptance.**
- Warm turn 2 to Claude: `process_start_ms == 0` and first-output time ≥ 40 % below turn 1, read from the Step 3 receipts.
- Killing the CLI externally causes the next turn to recreate the process, resume the native session, and succeed without replaying the transcript.
- Ten turns leave exactly one child process per active session; closing the app leaves zero.

**Verification.** Fixture-driven adapter tests for stream-input framing. Timing assertion from receipts. Process-count check before/after ten turns and after app close.

**Risk.** Highest-risk step. Mitigations: implement Claude first and ship it independently of Codex; keep the per-turn path as a live fallback selected by capability, never by guess; every protocol assumption backed by a recorded fixture.

---

### Step 8 — Ask / Quick Edit / Ship modes

**Goal.** Questions can never modify the repository. Edits are always shown before they land.

**Why.** `chat_can_write` (`lib.rs:5078`) is true for every non-side-chat turn, so Codex runs `-s workspace-write`, Claude `--permission-mode auto`, Cursor `--force` and Antigravity `--dangerously-skip-permissions` **against the user's live checkout**, restrained only by a sentence of prompt text (`lib.rs:5088–5092`). A misread question can mutate the user's tree with no diff, no confirmation and no undo.

**Mode definitions.**

| Mode | Providers | Writes to | Gate | Enforcement |
|---|---|---|---|---|
| **Ask** (default) | Full tier only | nothing | none | Codex `-s read-only`; Claude `--tools "Read,Grep,Glob,WebSearch" --permission-mode plan`; Cursor `--mode ask --sandbox enabled` |
| **Quick Edit** | Full tier only | scratch worktree, then reviewed apply | user approves a rendered diff | provider write mode inside the worktree only |
| **Ship** | all four | managed worktree only | automatic; existing promotion policy | unchanged |

**Changes.**

1. Replace `allow_writes: bool` with `mode: Mode` throughout.
2. **Antigravity is not offered for Ask or Quick Edit** (§2.0). It has no read-only mode, so rather than bypassing permissions to fake one, it is absent from those surfaces. The composer's participant list and `selectChatParticipant` must exclude Ship-only providers; the capability chip reads "Ship only — no read-only mode".
3. **Remove `--dangerously-skip-permissions` from Antigravity's Review path** (`lib.rs:2474`). It is currently applied unconditionally. Build and Revise keep it (inside the managed worktree); Review runs `--sandbox --mode plan` without it, and Step 9's mutation guard catches any write.
4. **Quick Edit** commands: `quick_edit_start` (create a lightweight worktree from current HEAD, run the provider with writes enabled inside it, return a diff), `quick_edit_apply` (reuse the same `git apply --check` → `git apply` sequence as `promote_worktree`), `quick_edit_discard` (remove the worktree).
5. **Remove the auto-transition from Chat to Ship** (`App.tsx:1589–1597`). Today a parsed marker in model output starts unattended repository work with no human keystroke. Keep the armed-autonomy skill, but a valid intent **pre-fills the Ship composer with the proposed objective and requires one Enter**.

**Acceptance.** An Ask turn cannot modify the checkout under any prompt, for **all three Full-tier providers** — and the guarantee is unconditional, because no provider lacking a read-only mechanism is offered. A Quick Edit shows a diff and changes nothing until Apply. Discard leaves no trace on disk.

**Verification.** Rust test per Full-tier provider: run an Ask turn against a repo containing a tracked file with the prompt "delete every file in this repository"; assert file content and mtime unchanged. Rust test: Antigravity is absent from the Ask and Quick Edit participant lists. Extend `src/coordination.test.ts` with the Ship-only participant filter assertion. Manual: Quick Edit round trip.

---

### Step 9 — RunCoordinator + abandon + truthful stop

**Goal.** Run state is a persisted machine, every control corresponds to a real backend action, and no state is unrecoverable.

**Why.** `execute_room_run` is a 1,369-line function (`lib.rs:5321–6689`) containing seven near-identical 25-field return blocks. Run state is also re-derived client-side from event payloads (`App.tsx:1215–1230`). `handleStop` sets `stopped` regardless of what `stop_run` returned (`App.tsx:1630–1644`), and `stop_run` returns `false` when no cancellation sender is registered — so Stop during the provider-probe window shows "stopped" while the run continues. There is **no abandon control anywhere** (`grep -rn abandon` → no matches), so an exhausted run leaks its worktree forever.

**Changes.**

1. **`run/mod.rs`: `RunCoordinator`** with one `advance(state) -> next_state` per phase, replacing the monolith. Single `persist_and_return` helper replacing the seven duplicated blocks. Persist every transition **before** emitting.
2. **New `run_events` table** so mid-run progress is durable and survives a window reload.
3. **`abandon_run(run_id)`**: refuse while active; otherwise call the existing `discard_isolation` (`lib.rs:4047`) to remove the worktree or clone; set state `abandoned`; emit a snapshot.
4. **`stop_run` returns `{ cancelled: bool, reason: String }`** and the UI renders that result. Register the cancellation token before any I/O in both chat and run paths.
5. **`promoting` is never auto-resumable.** A run interrupted mid-promotion goes to `waiting` with "promotion state unknown — inspect the repository before continuing". Re-applying a partially applied patch is not idempotent.
6. **Startup worktree GC**: remove worktree directories under the cache root with no matching non-terminal run row; report what was removed in the timeline rather than deleting silently.
6b. **Universal post-review mutation guard.** Before the Review and Final Review phases, record `git rev-parse HEAD` plus a `git status --porcelain` hash of the worktree. After the phase, compare. If the reviewer changed anything, fail the phase with "the reviewer modified the worktree" and preserve the run. This applies to **all four providers**, not just Antigravity: it replaces trusting a read-only flag with measuring the outcome, and it is the reason no provider needs to prove read-only capability to hold the reviewer role.
7. **Backend emits a full `RunSnapshot` on every transition.** Delete all client-side run derivation and route computation. This is where `createRun`, `routeForPhase`, `selectParticipant`, `explicitAgent`, `participantCanChat`, `participantIsRunnable`, and `selectChatParticipant` are deleted from `src/coordination.ts`, together with their App imports and call sites.
8. **Refuse a second concurrent Ship run explicitly.** Today `submitObjective` (`App.tsx:1513`) silently redirects to side chat whenever a run is active, so the composer can become permanently unable to start a run.

**Acceptance.** Stop pressed at any point either cancels or reports truthfully that it could not. Every non-active run with a worktree offers Abandon, and abandoning removes the directory. Starting Ship while a run is active produces a clear refusal, not a silent redirect. A run interrupted during promotion is not offered as resumable.

**Verification.** Rust tests: `abandon_run` removes the worktree and sets state; `abandon_run` on an active run is refused; stop during preflight returns `cancelled: false` with a reason; a `promoting` run reconciles to `waiting` and is not resumable; a review phase that mutates the worktree fails and preserves the run. Manual: press Stop within one second of starting a run.

---

### Step 10 — Verification that can actually run

**Goal.** Ship's verification stage reflects the code, not the absence of `node_modules`.

**Why.** `detect_verification` (`lib.rs:3776–3818`) emits `npm run test/build/lint` and `cargo check`, which run in a fresh worktree where `node_modules` and `target/` are gitignored and therefore absent, with no install step anywhere. On this repository `npm run build` is `tsc && vite build` and fails immediately, forcing the single allowed revision — which cannot install dependencies — and parking the run in `waiting`. Meanwhile `verification_passed` (`lib.rs:3910`) returns true when every result is `not-run`, so a Python or Go repo reaches **automatic promotion with zero verification** while the UI reports checks completed without failures.

**Changes.**

1. **New `verification_config` table** per project: detected commands, editable in Settings, an enable flag, and an optional `prepare` command. Detect once on attach; never silently re-detect over user edits.
2. **Prepare step**, run before checks with its own timeout and its own reported outcome:
   - `npm ci --prefer-offline` when `package-lock.json` exists (`npm install` otherwise)
   - `cargo fetch --locked` for Rust
   - user-specified command otherwise
3. **Third status `unavailable`** — tooling missing, or prepare failed — distinct from both `not-run` and `passed`.
4. **`verification_passed` must return false when no check actually ran.** A run with no configured verification reaches `waiting` with "no verification is configured for this project", never automatic promotion.
5. Make the 10-minute limit **per phase** with prepare excluded, since `npm ci` in a cold worktree is slow.
6. *Optional, measure before adopting:* reuse the base repository's dependency directories via a directory junction where the package manager tolerates it. Do not adopt on assumption.

**Acceptance.** A Ship run on this repository completes `npm run build` and `cargo check` inside the worktree. A Python repository with no configured checks stops at `waiting` and says why. A legitimately failing check still routes to revision.

**Verification.** Rust test: a fresh worktree from a fixture with a lockfile passes verification after prepare. Rust test: empty verification config yields `waiting`, not `complete`.

---

### Step 11 — Context deduplication

**Goal.** A resumed session receives only what it does not already have.

**Why.** The revision phase resumes the builder's native session (`lib.rs:6191`) and **also** resends the objective, repository instructions, skills, changed-file list and the full 32 KiB diff (`lib.rs:6145–6157`). The final review resumes the reviewer's session (`lib.rs:6369`) and **also** resends the objective, the full prior review text and the delta (`lib.rs:6320–6335`). This directly contradicts the project's own contract in `docs/V1_REVISED_PLAN.md:435`.

**Changes.**

1. `run/context.rs`: `ContextBuilder` takes `resumed: bool` and a per-session `last_seen_marker`.
   - **Revision, resumed:** findings + verification results only.
   - **Final review, resumed:** delta since *that reviewer's* last review + post-revision checks only.
   - **Ship skill, armed autonomy:** inject the full `SKILL.md` once per session, then a one-line reminder. Currently the whole 1,818-byte file is appended to **every** Chat turn while armed (`lib.rs:5098–5101`).
2. **Fix cross-provider handoff selection.** `recent_chat_handoff` (`lib.rs:4391`) decides by looking at the sender of the most recent agent message; when providers interleave it wrongly suppresses the handoff. Replace with a per-session watermark: each session records the last room message index it has seen; the handoff carries exactly the messages after that index, capped at 4 KiB.
3. **Reorder packets for prompt-cache stability:** stable content first (instructions, skills, memory), volatile last (delta, findings). Add Claude's `--exclude-dynamic-system-prompt-sections`.
4. **Always verify the resume actually succeeded** before trusting it. If the provider reports a new/unknown session, fall back to a full packet and note it in the timeline.
5. Populate `packet_bytes_saved` from Step 3.
6. Per-phase budgets: Ask 8 KiB · Quick Edit 16 KiB · Build 48 KiB · Review 32 KiB · Revise 16 KiB · Final review 12 KiB.

**Acceptance.** Receipts show revision and final-review packets ≥ 60 % smaller than baseline when the session resumed, with identical run outcomes on a fixture objective.

**Verification.** Rust tests asserting resumed packets exclude objective, instructions and skills. Before/after byte comparison from Step 3 receipts on the same fixture objective.

**Risk.** Under-sending to a provider that silently lost its session — mitigated by (4).

---

### Step 12 — UI corrections

**Goal.** The interface tells the truth, stays readable during long conversations, and shows Ship progress as it happens.

**Why.** `load_room` fetches the **oldest** 200 messages (`lib.rs:4476` — `ORDER BY created_at ASC LIMIT 200`), so past 200 messages the room reloads to old content and looks frozen. `refreshRoom` only runs after a Ship run *ends* (`App.tsx:1482`), so mid-run messages already in SQLite are invisible; progress is a 4-item ticker. Message bodies render through `<p>{message.body}</p>` (`App.tsx:403`) with no markdown, so code in an answer is unreadable.

**Changes.**

1. **Paging:** `ORDER BY created_at DESC LIMIT 100` with a cursor for older pages; reverse for display. Add index `messages(project_id, created_at DESC)`. Virtualise the timeline.
2. **Markdown rendering** with fenced-code support and copy buttons.
3. **Ship run card** pinned above the composer: stage, elapsed time, current owner, context bytes, revision/review/recovery counters, one live activity line. **Stream persisted run messages into the timeline as they are written.**
4. **Controls always visible with real actions behind them:** Stop (active only) · Resume (recoverable only, with attempts remaining) · Abandon (any non-active run with a worktree, with confirmation).
5. **Side chat is a control on the run card**, not a takeover of the main composer.
6. **Failure states** show what failed, at which stage, the preserved worktree path with a reveal-in-file-manager action, and the exact recovery options. Never a state with no available action.
7. **Session badges** per turn: `resumed` / `new session` / `handoff 3.1 KiB`. The user should be able to see they did not pay to re-paste.
8. **Capability chips** stating truthful differences: **"Ship only — no read-only mode" for Antigravity**, "cold start per turn" for Cursor and Antigravity, "completion stream, not token deltas" for Antigravity, "usage not reported" for Cursor and Antigravity. Ship-only providers must be filtered out of the Ask and Quick Edit participant lists and the `@mention` targets, not merely greyed out.
9. **Small fixes:** `App.tsx:141` contains a mojibake separator `Â·` (UTF-8 read as Latin-1) — fix the encoding. `App.tsx:117` hardcodes `/ 48 KiB` — read the real budget from the backend.
10. **Notification fallback:** `notify` swallows every error (`lib.rs:4080`) and Windows notifications from an unpackaged build often fail silently. Add an in-app attention banner as the primary channel so the desktop notification is an enhancement, not the only signal.

Preserve the Liquid Glass direction in `docs/LIQUID_GLASS_UI_PLAN.md`: glass on the control layer only, quiet content layer, monochrome icons, colour for semantic state and the primary action.

**Acceptance.** A 500-message room reloads to the most recent messages and scrolls smoothly. A 20 KB streamed answer renders with code blocks intact. During a Ship run, builder summary and verification evidence appear as they are written. Closing and reopening mid-run restores the same view. No horizontal overflow at 1024, 1280, 1440, 1800 px.

**Verification.** Manual at each width; mid-run window reload; 500-message seeded room in dev mode.

---

### Step 13 — Acceptance run

Execute §6 in full on the target machine and record results in `docs/V1_ACCEPTANCE.md` with dates and CLI versions. Any `n/a` must be a truthful capability limit that is **displayed in the UI**, not a silent degradation.

---

## 6. v1 acceptance checklist

Every line is pass/fail on observable behaviour.

**Attachment**
1. Fresh install launched from an arbitrary directory shows the attach screen with no seeded content.
2. Two unrelated repositories produce two independent rooms (history, sessions, model profiles, settings).
3. No developer's absolute path appears anywhere in the shipped UI.
4. An existing v1 database migrates with all messages and runs intact.

**Speed**
5. Warm turn 2 to Claude: `process_start_ms == 0`, first output ≥ 40 % faster than turn 1, from receipts.
6. Cold app start performs zero provider subprocesses when cached capability records match installed versions.
7. Local send acknowledgement visible within 150 ms for all four providers.

**Token efficiency**
8. A resumed same-provider turn sends the new message only — verified via `packet_bytes`.
9. Revision and final review, when resumed, send ≥ 60 % fewer bytes than baseline on the same fixture objective.
10. A provider switch sends ≤ 4 KiB of handoff and the receiving agent can answer a dependent follow-up.

**Autonomy**
11. A Ship run completes end to end on a JavaScript repo and on a Rust repo with no user input.
12. A repo with no configurable verification reaches `waiting` with an explicit reason — never auto-promotes.
13. Independent review is used whenever a second CLI is installed; same-provider review is visibly labelled a downgrade.

**Cancellation**
14. Stop at any point in any phase either cancels or reports truthfully that it could not.
15. Closing the app during a run leaves zero orphaned CLI processes (verify in Task Manager).

**Recovery**
16. Force-quit during a run, relaunch: run shows as stopped, recoverable, worktree intact.
17. Resume continues from the last completed phase, at most twice; the third is refused with an explanation.
18. Abandon removes the worktree and marks the run abandoned; the cache directory has no leftover.
19. A run interrupted during promotion is never offered as auto-resumable.

**Safety**
20. An Ask turn cannot modify the checkout under any prompt, for all three Full-tier providers. Antigravity is absent from Ask and Quick Edit entirely.
21. A Quick Edit changes nothing until Apply is clicked.
22. No provider runs with permission bypass outside a managed worktree, and no provider runs with permission bypass during Review.
23. A reviewer that mutates the worktree fails its phase and preserves the run.
24. Promotion is refused when base branch, HEAD or workspace fingerprint changed during the run.

**Usability**
25. A 500-message room reloads to the most recent messages and scrolls smoothly.
26. A 20 KB streamed answer renders without stutter, code blocks intact.
27. Every failure state offers at least one action.
28. No horizontal overflow at 1024, 1280, 1440, 1800 px.
29. Ship-only providers are visibly labelled as such and never appear in Ask or Quick Edit.

### Four-provider matrix

`n/a` cells are truthful capability limits that must be **displayed in the UI**, not silently degraded.

| Capability | Codex | Claude | Cursor | Antigravity |
|---|---|---|---|---|
| Tier | Full | Full | Full | **Ship-only** |
| Ask turn, read-only proven | ☐ | ☐ | ☐ | n/a — not offered |
| Quick Edit → diff → apply | ☐ | ☐ | ☐ | n/a — not offered |
| Token-delta streaming | ☐ | ☐ | ☐ | n/a — completion stream |
| Warm process, turn 2 | ☐ or documented fallback | ☐ | n/a — per-turn spawn | n/a — per-turn spawn |
| Two turns, native session resumed | ☐ | ☐ | ☐ | ☐ (Ship phases) |
| Ship as builder, end to end | ☐ | ☐ | ☐ | ☐ |
| Ship as reviewer | ☐ | ☐ | ☐ | ☐ |
| Reviewer mutation guard fires on a forced write | ☐ | ☐ | ☐ | ☐ |
| Cancel mid-turn, no orphan | ☐ | ☐ | ☐ | ☐ |
| Usage reported | ☐ | ☐ | n/a — not exposed | n/a — not exposed |
| 48 KiB packet delivered | ☐ stdin | ☐ stdin | ☐ via prompt file | ☐ via prompt file |

---

## 7. Out of scope for v1

Do not build these. Each was considered and deferred.

- Installer signing, auto-update, distribution.
- Cursor and Antigravity warm transports — their CLIs do not expose them.
- Provider model discovery beyond the existing `models` subcommands.
- Multiple simultaneous Ship runs in one repository.
- Everything in `docs/V1_REVISED_PLAN.md`'s non-goals list — those exclusions are correct and should hold: no plugin SDK, no cloud/mobile/web control plane, no issue tracker or task DAG, no semantic memory, no automatic push/PR/deploy, no unbounded repair loops.

---

## 8. Known unknowns

These could not be resolved statically. Resolve each with a live test before the step that depends on it, and record the outcome in `docs/providers/<provider>.md` with the date and CLI version.

| # | Unknown | Blocks | How to resolve |
|---|---|---|---|
| 1 | Is `codex app-server` stable enough in 0.144.4 for warm turns and thread resume? | Step 7 | Run `codex app-server --stdio`, capture the handshake, attempt one turn + resume. If not, fall back to `exec resume` or evaluate `codex mcp-server`. |
| 2 | Does `agy --sandbox --mode plan` complete a **review** without `--dangerously-skip-permissions`, or hang on a prompt? | Step 8 (non-blocking) | Run it against a worktree and observe. If it hangs, Antigravity keeps the bypass for Review and relies solely on Step 9's mutation guard — note this in the capability chip. This no longer blocks Ask, because Antigravity is Ship-only (§2.0). |
| 3 | Does `cursor-agent` read a prompt from stdin when no positional prompt is given? | Step 5 | Test `echo "hi" \| cursor-agent -p`. If yes, prefer stdin over the prompt-file workaround for Cursor. |
| 4 | Does `--sandbox enabled` block writes that `--force` is meant to allow in Cursor? | Step 8 | Run a Quick Edit with both flags in a scratch worktree and check whether the file changed. |
| 5 | Does `codex exec resume` accept `--json`/`-o` before the session ID? | Step 5 | Run `codex exec resume --help` and confirm argument order. |
| 6 | Do Windows notifications fire from an unpackaged dev build? | Step 12 | Trigger one and observe. The in-app banner is required regardless. |

---

## 9. Order of work, restated

```
run  step
 1   [1]  delete dead code            ~600 lines removed, 2 crates dropped
 2   [3]  measurement columns         everything after this is provable
 3   [4]  repository attachment       the product becomes usable by a second person
 4   [5]  adapter trait + real flags  fixes the Windows argv bug, kills help-string probing
 5   [6]  delta streaming             O(n) instead of O(n²)
 6   [7]  session manager + warm      the speed claim becomes true
 7   [8]  Ask / Quick Edit / Ship     the safety claim becomes true (Antigravity Ship-only, §2.0)
 8   [9]  run coordinator             the state claim becomes true
 9  [10]  verification prepare        Ship stops failing for environmental reasons
10  [11]  context dedup               the token claim becomes provable
11  [12]  UI corrections              the interface stops lying
12   [2]  split lib.rs                pure code motion, deferred to here
13  [13]  acceptance run              record it
```

Steps 1, 3 and 4 are the foundation and should land before anything else is attempted. Step 7 is the highest-risk step and should be shipped as Claude-only first, with Codex following once its fixture is captured. Step 2 is deliberately last-but-one: it is pure code motion, nothing depends on it, and running it early blocked this build twice.
