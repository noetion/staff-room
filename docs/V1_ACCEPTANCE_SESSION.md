# The Staff Room v1 — Manual Acceptance Session

Cost-ordered. Phase A spends no provider tokens at all. Phases B–D are ordered so
that if something fails you stop before the expensive part.

Record results in `docs/V1_ACCEPTANCE.md`, replacing the "not observed" rows.
Write PASS, FAIL, or n/a with one line of evidence. An `n/a` must be a truthful
capability limit that is **visible in the UI**, not a silent degradation.

**Estimated total: ~90 minutes, and low single-digit dollars of provider spend if
you follow the setup below. Ten to twenty times that if you don't.**

---

## Setup — do this first, it is where the savings come from

### S1. Build a throwaway fixture repo

Never run acceptance against The Staff Room repository. Its packets are 48 KiB, its diff
surface is 7,000 lines, and its verification runs `npm ci` plus `cargo check`.

```powershell
$fix = Join-Path $env:TEMP "staff-room-fixture"
New-Item -ItemType Directory -Force $fix | Out-Null
Set-Location $fix
git init -q
Set-Content package.json @'
{
  "name": "staff-room-fixture",
  "version": "1.0.0",
  "scripts": { "test": "node test.js" }
}
'@
Set-Content greet.js  "export function greet(name) { return 'Hello ' + name; }"
Set-Content test.js   "import { greet } from './greet.js'; if (greet('x') !== 'Hello x') { process.exit(1) } console.log('ok')"
Set-Content AGENTS.md "# Fixture`n`nKeep changes minimal. Do not add dependencies."
git add -A; git -c user.email=a@b -c user.name=t commit -qm "fixture"
```

`npm test` here finishes in under a second and needs no `node_modules`. That single
fact removes the environmental failure mode that Step 10 exists to fix, so Ship runs
test the pipeline instead of your toolchain.

### S2. Set every provider to its cheapest model and lowest effort

In Settings, for all four providers and all three routes:

| Provider | Model | Effort |
|---|---|---|
| Codex | `gpt-5.6-luna` | low |
| Claude | `fable` | low |
| Cursor | cheapest from `Models` | low |
| Antigravity | `gemini-3.5-flash-low` | low |

No acceptance criterion measures answer quality. Restore your real models afterwards.

### S3. Baseline

```powershell
Get-Process | Where-Object { $_.Name -match 'codex|claude|cursor|agy' } | Select Name,Id
```

Expect nothing. You will re-run this after the cancellation tests.

---

## Phase A — zero provider tokens (~30 min, £0)

Do all of these before spending anything.

| # | Check | How | Pass when |
|---|---|---|---|
| 1 | Fresh attach screen | With the app closed, hold aside both `%APPDATA%\com.staffroom.desktop\staff-room.db*` and any `%APPDATA%\com.agentroom.desktop\agent-room.db*`, then relaunch | Attach screen, no seeded or migrated conversation |
| 4 | Migration | Restore the held databases, relaunch | Prior messages and runs intact, no error |
| 2 | Two independent repos | Attach the fixture too, switch between them | Separate history, sessions, model profiles, settings |
| 3 | No developer paths | Read every visible string in a release build | No absolute developer path appears anywhere |
| 29 | Ship-only labelling | Open the composer participant list | Antigravity absent from Ask and Quick Edit; chip reads "Ship only" |
| 6 | Cold-start probe cost | Close app, relaunch, run the S3 process check during startup | No provider subprocesses spawned |
| 28 | Layout widths | Resize to 1024, 1280, 1440, 1800 | No horizontal overflow at any width |
| 25 | 500-message room | Seed the DB directly (below), reload | Most **recent** messages shown, smooth scroll |
| 27 | Failure states | Look at Cursor's failed card, and any failed run | Every failure names a cause and offers an action |
| 18 | Abandon | On any non-active run with a worktree, click Abandon | Worktree gone from `%LOCALAPPDATA%\...\worktrees`, run marked abandoned |

Seed 500 messages without a provider only after the fixture is attached. Close The Staff Room first, then use the helper's explicit opt-in. It resolves the exact fixture repository path and refuses to insert into any other project:

```powershell
.\scripts\phase-a.ps1 -SkipFixture -FixturePath $fix -SeedMessages
```

**Stop here if Phase A fails.** Nothing downstream is worth paying for until the
shell is sound.

---

## Phase B — single cheap turns (~20 min, a few thousand tokens)

| # | Check | How | Pass when |
|---|---|---|---|
| 7 | Ack latency | Send an Ask turn, watch the composer | Local acknowledgement visible within 150 ms |
| 20 | Ask immutability | To each Full-tier provider: "delete every file in this repository" | Nothing changes. Verify with `git status` |
| 22 | Permission boundaries | Same turns | No provider ran with permission bypass; check the receipt |
| 26 | Long answer render | "Write a 600-line JS file to the screen, do not save it" | Renders without stutter, code block intact |
| 5 | Second-turn truth | Two Ask turns to Claude, read Evidence | `process_start_ms` is recorded both times and the capability receipt truthfully reports a fresh session while `warm_session` is false |
| 8 | Resumed packet | Same two turns | Turn 2 `context_bytes` far below turn 1 |
| 10 | Cross-provider handoff | Ask Codex a question, then ask Claude "what did you just say?" | Claude answers using the ≤4 KiB handoff |
| 21 | Quick Edit | "Add a comment to the top of greet.js" | Diff appears; file unchanged until Apply; Apply lands it |

Item 5 is a PASS when the receipt honestly reports the implemented fresh-session behavior. It does not require a warm transport that the product does not claim.

---

## Phase C — cancellation and recovery (~15 min, near-free)

Cancel two seconds in. You pay for almost nothing.

| # | Check | How | Pass when |
|---|---|---|---|
| 14 | Cancel every phase | Start a Ship run on the fixture, Stop during build; repeat stopping during verify and review | Stop reports truthfully every time |
| 15 | No orphans | Start a run, close the app mid-run, run the S3 process check | Zero provider processes remain |
| 16 | Restart recovery | Relaunch after the above | Run shows stopped, recoverable, worktree intact |
| 17 | Resume limit | Resume the same run three times | Third refused with an explanation |
| 19 | Promotion interrupt | Close the app during promotion | Run reaches `waiting`, is **not** offered as auto-resumable |
| 23 | Reviewer mutation guard | Covered by `review_mutation_guard_detects_a_review_write` | Already PASS — cite the test |

---

## Phase D — the expensive part (~25 min, the bulk of the spend)

One clean Ship run first. Only proceed to the matrix if it passes.

**D1 — one full Ship run on the fixture.** Objective: *"Add a farewell function to greet.js and a test for it."*

This single run should settle:

| # | Check | Pass when |
|---|---|---|
| 11 | Human-gated Ship pipeline end to end | Build, verification, and review complete without mid-run input; promotion still requires human confirmation |
| 12 | Empty verification cannot promote | Already PASS by test; confirm the fixture's real `npm test` ran |
| 13 | Independent review | Reviewer differs from builder, not labelled degraded |
| 9 | Packet-byte savings | Revision and final-review `context_bytes` well below build |
| 24 | Promotion guards | Already PASS by test |

**D2 — the four-provider matrix in four runs, not eight.** Every run has a builder
and a reviewer, so four paired runs cover all eight cells:

| Run | Builder | Reviewer |
|---|---|---|
| 1 | Codex | Claude |
| 2 | Claude | Cursor |
| 3 | Cursor | Antigravity |
| 4 | Antigravity | Codex |

Use the same one-line objective each time, reverting the fixture between runs
(`git reset --hard` in the fixture repo). Record the 48 KiB-packet-delivered row
per provider while you are there — that is the Windows argv fix being proven.

---

## Cost discipline

- If a Ship run exceeds five minutes on the fixture, stop it. Something is wrong
  with the pipeline, not the model, and letting it run only burns quota.
- Do not re-run a passing check to "be sure."
- If Phase D run 1 fails, fix and re-run **that one run**. Do not start the matrix.
- Restore your real models in Settings when finished.

## Afterwards

Rewrite `docs/V1_ACCEPTANCE.md` from what you observed. Every row gets PASS, FAIL,
or a truthful `n/a`. Then the honest v1 question is answerable: not "is the code
written" — it is — but "does it do what it says on the box."
