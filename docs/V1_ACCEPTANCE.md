# Human-gated v1 acceptance

This is the historical v1 record, not current acceptance. Development is experimental and pre-1.0 (`0.1.0`), with no supported stable binary. Current provider restrictions and outstanding native release gates are recorded in [provider boundary acceptance](PROVIDER_BOUNDARY_ACCEPTANCE.md); successful v1 turns did not establish absolute-path filesystem confinement.

**Scope:** Windows, personal-use, assisted operation

**Historical status:** Recorded as accepted for the human-gated Windows v1.0 release; not a verdict on current development

**Autonomy:** Not accepted and fail-closed
**Voice:** Implemented; packaged microphone round trip requires a configured local whisper.cpp CLI and model

This record does not treat autonomous behavior as part of the assisted v1. The original autonomous matrix is retained as a separate, stricter contract in `AUTONOMOUS_ACCEPTANCE_CONTRACT.md`.

## Automated contract

| Contract | Required evidence | Current implementation |
| --- | --- | --- |
| Renderer build | TypeScript and Vite production build | `npm run build` |
| Local logic | Frontend and Rust tests | `npm test`; `cargo test --manifest-path src-tauri/Cargo.toml` |
| Design system | No forbidden typography shorthand or unowned design literals | `node scripts/check-design.mjs` |
| Rendered UI | Required styles load; supported widths/themes; direct contrast; Axe serious/critical findings | `npm run qa:visual:acceptance` |
| One local gate | All preceding checks run without a paid provider | `npm run check` |
| Project authority | Mutation requests contain `projectId`, not renderer repository paths | `project_repository` reloads, canonicalizes, and identity-checks SQLite state |
| Operation identity | New Chat, Quick Edit, and Ship IDs are Rust-issued, project-bound, one-time leases | `allocate_operation_id` and `consume_operation_id` |
| Artifact containment | Operation artifact directories accept canonical UUIDs only | `run_artifact_directory` |
| Review immutability | Review and Final Review use explicit read-only adapter mode plus mutation guard | `ProviderMode::Review` |
| Connection truth | Probe mode does not write and succeeds only for exact `READY` | `ProviderMode::Probe`; `connection_test_ready` |
| Provider process custody | Every Windows provider turn starts suspended, enters its run-owned Job Object before execution, and retains descendant-tree custody until the phase ends | `prepare_owned_provider_command`; `ProviderJob`; `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` |
| Human promotion | Reviewed changes stop at `awaiting-promotion`; Promote requires confirmation and an unchanged reviewed Git fingerprint | `approve_run_promotion` |
| Promotion serialization | Only one promotion per project can run | `active_promotions` project lock |
| Migration backup | Online SQLite backup, integrity check, atomic finalize | `backup_v1_database` |
| Voice custody | Rust microphone capture; 30-second capture and two-minute inference bounds; temporary audio deletion on every exit | `commands/voice.rs` |
| Voice authorization | Transcript inserts at caret; no auto-submit or action authorization | `Composer.insertTranscript` and hold-to-talk control |

## Local evidence recorded 2026-08-09

- `npm test`: 24 tests passed.
- `npm run build`: production TypeScript and Vite build passed.
- `node scripts/check-design.mjs`: 81 source files passed.
- `cargo test --manifest-path src-tauri/Cargo.toml`: 72 tests passed.
- `npm run qa:visual:acceptance`: 89 checks passed with 0 failures across light and dark themes, 720, 1024, 1280, 1440, and 1800 pixel widths, reduced motion, forced colours, reduced transparency, focus visibility, text contrast, stylesheet presence, and selected Axe rules.
- `cargo clippy --all-targets --manifest-path src-tauri/Cargo.toml -- -D warnings`: passed.
- Packaged scroll measurement: 121 frames, 18.5 ms average, 21.2 ms p95, 25 ms maximum in a 500-message fixture.
- `git diff --check`: passed.

The scroll-performance check was intentionally deferred to the final audit. No paid provider or live CLI run was used for the 2026-08-09 evidence.

## Stable candidate evidence recorded 2026-08-10

- `npm run check`: PASS; 28 frontend tests, production build, design lint across 82 files, 75 Rust tests, and 89 visual and accessibility checks.
- `cargo clippy --all-targets --manifest-path src-tauri/Cargo.toml -- -D warnings`: PASS.
- Windows descendant-process test: PASS; the suspended provider entered its Job Object before execution, its wrapper exited after spawning a child, and dropping the job terminated that detached descendant.
- Process-custody package recheck: PASS on the 4,143,689-byte `1.0.0` installer with SHA-256 `62B8CDA33ED967851AFCFC6223BCB12BD71ABDA03EEDCD9DD9123AA29BC3D2F9`. Live probes passed for Codex `0.144.4`, Claude Code `2.1.221`, Cursor Agent `2026.08.04-aaa8809`, and Antigravity `1.1.11` through the suspended, Job-owned launch path. Cursor's probe also exercised Job-owned session preallocation, and process inventory returned to the pre-probe baseline.
- Final-source recheck: PASS after the later preview-catalogue, explicit-provider-routing, and recovery-message corrections. The full gate passed with the focused routing and exact recovery assertions. The final installer was rebuilt and installed; its Codex probe passed, and a packaged Ship submission that explicitly named unavailable Cursor failed visibly without starting a run or falling back to ready Codex. The provider-launch implementation was unchanged from the four-provider probe build.
- All live work used synthetic Git fixtures. No personal repository, credential, or private provider transcript is part of this record.

## Required final observations

These are deliberately left for the final audit and must not be claimed from unit tests:

| Observation | Pass condition | Result | Evidence |
| --- | --- | --- | --- |
| Fresh packaged launch | Attach view opens, a repository attaches, and no fabricated native data appears | PASS | The packaged candidate opened the attach screen, attached a disposable repository, and spawned no provider process during cold start. |
| Two-project isolation | Switching projects during or after async work never displays or applies another project's result | PASS | Two disposable repositories had distinct IDs, paths, empty snapshots, and visible room switches. |
| Human Ship | A verified change reaches `awaiting-promotion`; base checkout is unchanged before Promote; confirmed Promote applies only the reviewed delta | PASS | Run `52bc7ce6-aa2e-41bd-bdd9-2bcca85311e5`: Codex build, real `npm test`, independent Claude review, unchanged base before native Promote, clean fast-forward, passing post-promotion test, and worktree cleanup. |
| Quick Edit | Diff stays visible until Apply or Discard; project switch and cleanup behavior are correct | PASS | Edit `ec27c6c3-5e35-4bb8-ada0-f7f9e10080d0`: exact diff remained isolated, native Apply landed only the reviewed line, cleanup succeeded, tests passed. |
| Provider matrix | Codex, Claude, and Cursor are observed in Ask, Quick Edit, Build, Review, cancellation, and connection probes when installed. Antigravity is recorded N/A for Ask and Quick Edit and observed only in its supported Build, Review, cancellation, and connection routes. | PASS | Ask and Quick Edit passed for Codex, Claude, and Cursor; Antigravity was visibly Ship-only and therefore N/A. Paired Ship runs covered Codex to Claude (`52bc7ce6`), Claude to Cursor (`bc99e5da`, refreshed after the Cursor upgrade), Cursor to Antigravity (`fd61c16c`), and Antigravity to Codex (`ad03a257`). Build cancellation passed for all four, Antigravity review cancellation passed, and all four live probes passed. |
| Cancellation and restart recovery | Stop is truthful during Build, Verify, and Review; app exit leaves no run-owned provider process; restart preserves recoverable work; the third Resume is refused | PASS | Build stops passed for Codex (`1370a59a`), Claude (`f6b2932d`), Cursor (`881c0218`), and Antigravity (`ab9c6a3c`). Verification (`102ef68e`) and Antigravity review (`3d1c39d3`) reported their exact stopped phase. App-close returned provider PIDs to the pre-run baseline, restart preserved the worktree, two recovery attempts ran, and the third was refused with the configured limit. |
| Promotion interrupt | Closing during promotion produces an ambiguous `waiting` state that cannot be auto-resumed | PASS | Run `d3bdb87c` was closed after a fixture hook proved promotion was active. Restart reported `Promotion state unknown`; the packaged UI offered only Abandon, and a direct recovery call was rejected by Rust. The inspected worktree then abandoned cleanly. |
| Packaged voice | Microphone capture works with configured local assets and inserts an editable transcript at the caret | N/A | Packaged Settings visibly reports setup required; no local whisper CLI/model was configured for this audit. |
| Voice denial/failure | Missing permission, device, engine, model, silence, or inference failure leaves typed text usable and unchanged | PASS | Missing engine/model is shown as setup required; typed-text custody and failure mapping are covered by the passing voice tests. |
| Voice retention | Temporary WAV and transcript output files are deleted after success and failure | N/A | No capture was possible without configured local assets; deletion paths remain covered by the automated voice contract. |
| Installer/migration | A pre-v1 database produces a verified `.v1.bak` and retains project data | PASS | Legacy and current databases both passed `quick_check`; project/message/run/profile counts matched 3/129/6/0. All six personal DB files were restored byte-identically after acceptance. |

## Release verdict rule

Human-gated v1 can be marked accepted only when `npm run check` passes, every applicable final observation above is recorded as PASS, every unsupported provider-route cell is explicitly recorded as N/A, and every behavior-changing difference introduced after the route matrix receives direct revalidation at its affected layer, including the target package when integration behavior changes. The full route matrix passed on the fixed release candidate. Race-free process custody then passed its Windows descendant test and fresh packaged probes for all four real providers. The later explicit-provider-routing correction passed a focused frontend regression and exact packaged rejection test; the recovery-message correction passed exact Rust restart assertions; and the preview catalogue passed the full build and UI gate. The human-gated Windows v1 verdict is **accepted**.

Autonomous mode does not inherit this verdict. It remains locked and fail-closed until every requirement in `AUTONOMOUS_ACCEPTANCE_CONTRACT.md` passes independently.
