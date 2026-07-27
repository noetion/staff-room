# V1 acceptance run

**Date:** 2026-07-27
**Target:** Windows development machine, `<repo>`
**Result:** **FAIL — release acceptance is not complete.** The automated checks below passed, but this repository has no desktop or provider end-to-end acceptance harness and the required interactive observations were not performed in this run. A failed row means the specified behaviour was not observed, not that the product has been proven to behave incorrectly.

## Tooling recorded on the target

| Tool | Version |
| --- | --- |
| Node.js | v22.14.0 |
| npm | 10.9.2 |
| Cargo | 1.97.1 |
| Rust | 1.97.1 |
| Codex CLI | 0.144.4 |
| Claude Code | 2.1.220 |
| Cursor Agent | 2026.07.23-e383d2b |
| Antigravity (`agy`) | 1.1.7 |

## Required build gates

| Gate | Result | Evidence |
| --- | --- | --- |
| `npm test` | PASS | 1 Vitest test passed. |
| `npm run build` | PASS | TypeScript compilation and Vite production build passed. |
| `cargo test --manifest-path src-tauri/Cargo.toml` | PASS | 45 Rust tests passed. |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` | PASS | Completed with no denied warnings. |

## Acceptance checklist

| # | Result | Observation |
| --- | --- | --- |
| 1 | FAIL | Fresh-install attach-screen behaviour was not observed in a desktop build. |
| 2 | FAIL | Two independent repositories were not exercised end-to-end. |
| 3 | FAIL | A shipped UI was not inspected for absolute paths. |
| 4 | FAIL | A real pre-v1 database migration was not run. |
| 5 | FAIL | A two-turn Claude timing receipt was not captured. |
| 6 | FAIL | A cold application launch with cached capability records was not observed. |
| 7 | FAIL | Provider acknowledgement latency was not measured. |
| 8 | FAIL | A resumed same-provider packet capture was not taken. |
| 9 | FAIL | Revision and review packet-byte baselines were not measured. |
| 10 | FAIL | A cross-provider dependent follow-up was not run. |
| 11 | FAIL | An unattended Ship run was not completed on JavaScript and Rust repositories. |
| 12 | PASS | Rust test `empty_verification_config_cannot_pass` confirms an empty verification configuration cannot auto-promote. |
| 13 | FAIL | A two-CLI independent review was not observed. |
| 14 | FAIL | Cancellation was not exercised at every phase. |
| 15 | FAIL | The app was not closed mid-run and Task Manager was not inspected for orphaned processes. |
| 16 | PASS | Rust test `interrupted_runs_become_recoverable_on_restart` passed. |
| 17 | FAIL | Resume limit and third-attempt refusal were not exercised on a live run. |
| 18 | FAIL | Abandon cleanup was not exercised on a live run. |
| 19 | FAIL | Promotion interruption and auto-resume availability were not observed. |
| 20 | FAIL | Ask immutability was not proven for all Full-tier providers. |
| 21 | FAIL | Quick Edit was not exercised through Apply. |
| 22 | FAIL | Permission boundaries were not observed for every provider and route. |
| 23 | PASS | Rust test `review_mutation_guard_detects_a_review_write` passed. |
| 24 | PASS | Rust tests for detached HEAD, branch switch, and dirty checkout promotion guards passed. |
| 25 | FAIL | A 500-message room reload was not observed. |
| 26 | FAIL | A 20 KiB streamed answer was not rendered. |
| 27 | FAIL | Failure-state recovery actions were not inspected in the UI. |
| 28 | FAIL | Desktop layouts at 1024, 1280, 1440, and 1800 px were not inspected. |
| 29 | FAIL | Ship-only provider labelling and route exclusion were not exercised in the UI. |

## Four-provider matrix

No provider behaviour was executed in this run. The capability-limit cells below are product requirements rather than observed results; each must be visibly represented in the shipped UI before release.

| Capability | Codex | Claude | Cursor | Antigravity |
| --- | --- | --- | --- | --- |
| Tier | FAIL — not observed | FAIL — not observed | FAIL — not observed | FAIL — not observed |
| Ask turn, read-only proven | FAIL — not observed | FAIL — not observed | FAIL — not observed | Capability limit: Ship-only, not offered |
| Quick Edit → diff → apply | FAIL — not observed | FAIL — not observed | FAIL — not observed | Capability limit: Ship-only, not offered |
| Token-delta streaming | FAIL — not observed | FAIL — not observed | FAIL — not observed | Capability limit: completion stream |
| Warm process, turn 2 | FAIL — not observed | FAIL — not observed | Capability limit: per-turn spawn | Capability limit: per-turn spawn |
| Two turns, native session resumed | FAIL — not observed | FAIL — not observed | FAIL — not observed | FAIL — not observed |
| Ship as builder, end to end | FAIL — not observed | FAIL — not observed | FAIL — not observed | FAIL — not observed |
| Ship as reviewer | FAIL — not observed | FAIL — not observed | FAIL — not observed | FAIL — not observed |
| Reviewer mutation guard fires on a forced write | FAIL — not observed | FAIL — not observed | FAIL — not observed | FAIL — not observed |
| Cancel mid-turn, no orphan | FAIL — not observed | FAIL — not observed | FAIL — not observed | FAIL — not observed |
| Usage reported | FAIL — not observed | FAIL — not observed | Capability limit: not exposed | Capability limit: not exposed |
| 48 KiB packet delivered | FAIL — not observed | FAIL — not observed | FAIL — not observed | FAIL — not observed |

## Evidence retained by the automated suite

The Rust suite additionally passed fixture or unit coverage for capability discovery, bounded context packets, handoffs, warm capability caching, model profiles, token-delta stream coalescing, project-scoped settings, recovery state, review mutation protection, worktree preservation, and promotion checks. Those checks are not substitutes for the end-to-end observations above.
