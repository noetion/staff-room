# Agent Room working v1

**Status:** Approved implementation direction  
**Revised:** 2026-07-26  
**Product boundary:** A local desktop build, review, revision, and verification room for Codex, Claude Code, Cursor Agent, and Antigravity CLI.

## Outcome

Agent Room removes the user from the role of message courier. The user states one engineering objective. Agent Room carries the relevant context, repository evidence, results, and findings between coding CLIs until the work is complete or a decision genuinely requires the user.

The working v1 is intentionally not an IDE, remote agent platform, generic workflow engine, issue tracker, or multi-user collaboration system.

## What changed

The original plans correctly identified durable rooms, provider adapters, selected context, review, and evidence as the solution. The revised direction narrows the implementation around lessons from Sandcastle, OpenClaw Code Agent, Superset, Orca, Aider, Sudocode, and Maestro:

- from Sandcastle: isolated execution, bounded iterations, native session custody, completion timeouts, and preserved failures;
- from OpenClaw Code Agent: delegated decisions, explicit worktree lifecycle states, one continuation path, and canonical completion summaries;
- from Superset: process-derived agent state and attention-based desktop notifications;
- from Orca: provider-specific session discovery and hooks, especially for Antigravity;
- from Aider: a measured repository-context budget instead of full transcript replay;
- from Sudocode and Maestro: compact durable handoffs and evidence-backed completion.

Agent Room adopts those mechanics without adopting their broader IDE, server, task-management, remote, mobile, or plugin architectures.

## Working v1 flow

```text
objective
  -> select builder
  -> create managed branch + worktree
  -> build in safest supported autonomous mode
  -> capture native session + repository delta
  -> run detected verification commands
  -> select an independent reviewer when available
  -> review a compact evidence packet
  -> one automatic revision when material findings exist
  -> rerun verification
  -> final review
  -> promote only if policy gates pass
  -> notify complete or attention required
```

The normal flow has no per-command human approval. User attention is reserved for:

- authentication or provider usage limits;
- a destructive or external action outside the repository policy;
- a material product decision the objective does not answer;
- a changed or dirty base checkout that prevents safe promotion;
- a merge conflict;
- failed verification after the bounded repair pass;
- failed or inconclusive final review;
- exhausted process, review, or revision limits.

The room also has an optional project-scoped autonomous Ship setting. When armed, a repository skill teaches the selected Chat agent to emit a small structured Ship intent only for an explicit implementation request. Agent Room validates that intent and starts the normal isolated route without another approval click. The provider never gains authority to skip verification, review, bounded recovery, or promotion policy.

## Autonomy policy

Every participant reports one truthful mode:

| Mode | Meaning |
| --- | --- |
| `isolated-auto` | The CLI runs without routine prompts inside repository/worktree restrictions. |
| `reviewed-auto` | The CLI has a native AI-mediated approval reviewer. |
| `unattended-bypass` | Permission bypass is used only inside an outer isolation boundary. |
| `manual` | The installed CLI cannot safely complete unattended work. |
| `unavailable` | The required automation CLI is not installed or not ready. |

Provider policy:

- Codex: `--ask-for-approval never` with `workspace-write` for build/revision and `read-only` for review.
- Claude Code: `--permission-mode auto` when its live help proves that choice. Unsupported historical flags are not passed.
- Cursor Agent: read-only Chat uses proved ask mode, sandboxing, and partial output; `--force` is used only for unattended writes inside the managed worktree boundary.
- Antigravity: installed `agy 1.1.7` proves sandboxed print mode, permission bypass, and exact conversation resume. Text-only output and unavailable usage reporting remain explicit downgrades.

The probe executes `--version` and `--help`, plus provider subcommand help where needed. An executable being present is not sufficient for a ready state. Every flag required for the claimed autonomy mode must be present in current help output. Known Windows installer locations are checked after `PATH`, without substituting a desktop editor executable for its automation CLI.

## Review independence

Agent Room chooses a reviewer in this order:

1. an installed participant different from the writer;
2. the configured project reviewer if different and available;
3. a fresh session of the writer provider, visibly labelled `same-provider review`.

Same-provider review is a functional fallback, not equivalent independence. It lets a one-provider installation run autonomously while preserving an honest capability signal. The room recommends installing a second CLI for independent review.

## Isolation and promotion

Each objective receives:

- a branch named `agent-room/<run>`;
- a managed Git worktree in the application cache;
- the base branch and base HEAD captured before execution;
- a preserved worktree on failure or unresolved attention state.

Promotion is automatic only when:

- the managed worktree has a committed delta or an intentional no-change result;
- required verification passes;
- review approves;
- no material finding remains;
- the base checkout is clean;
- the base HEAD has not changed;
- a fast-forward merge succeeds.

If any condition fails, Agent Room does not modify the base checkout. It preserves the branch/worktree and reports the recovery path.

## Compact context packet

Each activation receives only:

1. objective and current assignment;
2. applicable repository instructions;
3. compact project memory;
4. direct parent handoff;
5. changed-file list and focused diff;
6. verification results;
7. unresolved review findings;
8. remaining run limits;
9. required result contract.

Rules:

- never replay the complete room by default;
- never resend output already retained by the resumed native session;
- cap each source and the total packet;
- record UTF-8 byte size and an estimated token count;
- prefer changed-file names, diff stats, and focused hunks over prose;
- truncate with an explicit marker rather than silently dropping context.

Initial total budget: 48 KiB per activation.

### Instruction hierarchy and project skills

Agent Room discovers root and nested `AGENTS.md` and `CLAUDE.md` files. Root instructions enter the initial packet. Nested instructions are inventoried for the builder and included directly when changed files fall under their directory scope.

Repository-local skills are discovered under `.codex/skills`, `.agents/skills`, `.claude/skills`, and `skills`. A deterministic objective-to-name/description match selects at most three skills. The packet carries their paths and descriptions, then requires the provider to open each selected `SKILL.md` in full from the managed worktree. Skill bodies are not duplicated into the packet. Provider-global skills remain provider-owned.

### Structured handoff contract

Every provider final response must contain a schema-v1 JSON record between explicit Agent Room markers. Required fields are:

- `schemaVersion`;
- `status`;
- `summary`;
- `changedFiles`;
- `checks`;
- `findings`;
- `nextAction`.

Build and revision statuses are `completed`, `blocked`, or `failed`. Review statuses are `approved` or `changes_required`. Invalid or missing records fail the phase and preserve recovery state. Agent-reported files and checks are advisory; Git and coordinator-run verification remain authoritative.

### Bounds, logs, and recovery

- provider phase timeout: 20 minutes;
- provider idle-output timeout: 5 minutes;
- verification timeout: 10 minutes per command;
- captured stdout/stderr cap: 2 MiB per stream and phase;
- automatic revision: 1;
- review passes: 2;
- manual recovery attempts: 2.

Each phase writes durable capped stdout, stderr, and final-response artifacts in the application local-data run directory. A failed, stopped, or waiting run exposes `Resume recovery`. Recovery reuses the same worktree, branch, base HEAD, objective, and builder session when available. It reruns the bounded build, verification, review, and promotion route. Exhausted recovery attempts require manual inspection.

## Completion contract

An agent's completion statement is evidence, not authority. Agent Room owns completion.

A run is complete only when:

- the provider process exited successfully;
- repository evidence was captured;
- required verification passed;
- final review approved;
- promotion succeeded or the task intentionally produced no repository change;
- no approval or decision is pending.

## UI contract

The Handoff Lens remains the single expressive visual device. It shows:

- active phase and owner;
- build, verification, review, revision, and promotion route;
- autonomy mode and review independence;
- context packet size;
- review and revision limits;
- Stop while a process is running;
- the preserved worktree path when attention is required.

Timeline and inspector surfaces remain quiet, solid, and readable. Ordinary CSS glass provides the Liquid Glass character. Keyboard focus, labelled controls, semantic tab panels, accessible status/error regions, reduced-motion, increased-contrast, and solid-surface fallbacks are required. Agent Room is a desktop application with a supported minimum window size, not a phone app.

Desktop notifications fire only for:

- run complete;
- user attention required;
- run failed;
- run stopped.

## Persistence

SQLite remains operational truth. The v1 persists:

- projects;
- messages;
- runs and phase state;
- provider sessions;
- activation summaries;
- verification evidence;
- worktree and promotion state.

Provider transcripts remain provider-owned. Agent Room stores session identifiers and compact handoffs, not duplicate full transcripts.

Chat follows the same principle. A resumed native provider session receives only the new user turn. When the user switches providers, Agent Room sends only the most recent useful exchange, capped at 4 KiB, so the next agent can continue without replaying the room.

## Explicit non-goals

- full code editor or terminal multiplexer;
- cloud sandboxes, remote hosts, phone client, or web control plane;
- generic plugins or public adapter SDK;
- issue tracker, task DAG, backlog dispatcher, or agent organisation chart;
- multiple simultaneous writers in one repository;
- unconstrained LLM-selected routing for every phase;
- semantic vector memory;
- automatic push, pull request creation, deployment, or external side effects;
- unbounded repair or review loops.

## Acceptance criteria

The v1 is complete when:

1. all four CLIs are probed and report truthful autonomy and resume capabilities;
2. any installed participant can be explicitly selected;
3. a run creates and uses a managed worktree;
4. provider events stream into the room and Stop terminates only the owned process;
5. builder output, Git delta, and checks reach the reviewer without copying;
6. one revision can resume the builder session;
7. the final reviewer receives only the delta since its prior review plus current evidence;
8. promotion is policy-gated and safe;
9. failure preserves recoverable work;
10. completion and attention states produce desktop notifications;
11. restart restores the room, latest run, evidence, and recovery path;
12. instruction and selected-skill paths are persisted and visible;
13. every provider phase produces a valid schema-v1 handoff or preserves the run as failed;
14. an armed autonomous Ship skill can hand an explicit implementation request into the isolated route without a second approval click;
15. provider probes are cached on the Chat path and Chat receipts expose preflight, first-output, and total latency;
16. Chat, Build, and Review model choices are stored independently per provider and project;
17. raw bounded logs and final responses remain available after restart;
18. a preserved run can resume at most twice without creating a new worktree;
19. frontend tests, frontend build, Rust tests/check/lint, and the packaged desktop development flow pass.
