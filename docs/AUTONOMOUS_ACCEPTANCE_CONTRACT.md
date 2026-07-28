# Autonomous Ship acceptance contract

**Current verdict:** NOT ACCEPTED
**Runtime posture:** Locked and fail-closed
**Unlock point:** `AUTONOMOUS_ACCEPTANCE_COMPLETE` must remain `false` until every mandatory row below has current evidence.

Autonomous mode is a separate product tier. Passing the human-gated v1 contract does not authorize unattended writing or automatic promotion.

## Mandatory gates

| Gate | Acceptance evidence | Current state |
| --- | --- | --- |
| Provider capability proof | Versioned, repeatable proof for the exact installed CLI version and every mutating/read-only flag | Missing live versioned matrix |
| Read-only review | Forced-write fixtures prove every reviewer is denied mutation before provider execution, not merely detected afterward | Partial: read-only modes and post-run mutation guard exist |
| Process ownership | Entire provider and verification process trees terminate on Stop, timeout, crash, and app exit | Missing Windows Job Object or equivalent ownership |
| Operation registry | Every active operation is typed, project-bound, collision-safe, and cleaned on all exit paths | Partial: Rust-issued one-time IDs and project-bound cancellation exist |
| Repository trust | No unconditional provider trust; managed Git trust is scoped to the operation | Partial: Cursor unconditional `--trust` removed |
| Hook suppression | Coordinator Git actions cannot run repository hooks or user-defined filters unexpectedly | Missing complete proof |
| Environment minimization | Child processes receive an explicit allowlist rather than the full parent environment | Missing |
| Output bounds | stdout, stderr, structured events, logs, and final responses are independently capped under hostile output | Partial |
| Review fingerprint | Full reviewed content, file modes, symlinks, untracked files, and relevant metadata are fingerprinted and revalidated | Partial: human promotion revalidates the reviewed Git HEAD and clean status |
| Atomic cancellation/promotion | Cancellation and promotion share a durable state transition; no cancellation can be lost at the boundary | Partial pre-promotion check only |
| Promotion lock | One durable per-project promotion lease survives concurrent IPC and restart ambiguity | Partial in-memory project lock |
| Verification policy | Commands are approved or sandboxed, bounded, and cannot escape the managed worktree | Missing |
| Destructive cleanup | Abandon, Quick Edit cleanup, and interrupted promotion are idempotent and path-contained | Partial |
| Deterministic coordinator E2E | Fake CLIs exercise success, malformed output, huge output, timeout, cancellation, revision, review, and promotion races | Missing |
| Packaged Windows E2E | Signed candidate passes attach, restart, cancellation, recovery, and promotion tests | Missing |
| Live provider smoke | Current Codex, Claude, Cursor, and Antigravity versions pass their supported route matrix | Missing |
| Original acceptance matrix | Every applicable autonomous row has dated evidence; no silent scope exclusions | Missing |

## Unlock procedure

1. Implement the missing mandatory controls.
2. Run deterministic hostile-provider tests.
3. Run packaged Windows E2E.
4. Run explicit, opt-in live-provider smoke tests and record versions.
5. Complete the original autonomous matrix.
6. Obtain a fresh code review focused on custody and concurrency.
7. Only then set `AUTONOMOUS_ACCEPTANCE_COMPLETE` to `true` and expose the arming control.

Any regression in a mandatory gate returns the constant to `false`. The UI and README must never describe autonomous mode as available while this contract is incomplete.
