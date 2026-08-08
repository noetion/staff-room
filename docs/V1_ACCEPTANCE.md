# Human-gated v1 acceptance

**Scope:** Windows-first, personal-use, assisted operation

**Status:** Implementation complete; final packaged and live-provider audit pending

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
| Human promotion | Reviewed changes stop at `awaiting-promotion`; Promote requires confirmation and an unchanged reviewed Git fingerprint | `approve_run_promotion` |
| Promotion serialization | Only one promotion per project can run | `active_promotions` project lock |
| Migration backup | Online SQLite backup, integrity check, atomic finalize | `backup_v1_database` |
| Voice custody | Rust microphone capture; 30-second capture and two-minute inference bounds; temporary audio deletion on every exit | `commands/voice.rs` |
| Voice authorization | Transcript inserts at caret; no auto-submit or action authorization | `Composer.insertTranscript` and hold-to-talk control |

## Local evidence recorded 2026-07-28

- `npm test`: 12 tests passed.
- `npm run build`: production TypeScript and Vite build passed.
- `node scripts/check-design.mjs`: 80 source files passed.
- `cargo test --manifest-path src-tauri/Cargo.toml`: 51 tests passed.
- `npm run qa:visual:acceptance`: equivalent direct invocation passed 84 checks with 0 failures across light and dark themes, 720, 1024, 1280, 1440, and 1800 pixel widths, reduced motion, forced colours, reduced transparency, focus visibility, text contrast, stylesheet presence, and selected Axe rules.
- `git diff --check`: passed.

The scroll-performance check is intentionally deferred to the final audit. No paid provider or live CLI run was used for this evidence.

## Required final observations

These are deliberately left for the final audit and must not be claimed from unit tests:

| Observation | Pass condition | Result | Evidence |
| --- | --- | --- | --- |
| Fresh packaged launch | Attach view opens, a repository attaches, and no fabricated native data appears | NOT RUN | Record packaged-build observation |
| Two-project isolation | Switching projects during or after async work never displays or applies another project's result | NOT RUN | Record packaged-build observation |
| Human Ship | A verified change reaches `awaiting-promotion`; base checkout is unchanged before Promote; confirmed Promote applies only the reviewed delta | NOT RUN | Record run ID and repository observation |
| Quick Edit | Diff stays visible until Apply or Discard; project switch and cleanup behavior are correct | NOT RUN | Record run ID and repository observation |
| Provider matrix | Codex, Claude, and Cursor are observed in Ask, Quick Edit, Build, Review, cancellation, and connection probes when installed. Antigravity is recorded N/A for Ask and Quick Edit and observed only in its supported Build, Review, cancellation, and connection routes. | NOT RUN | Record provider versions and a PASS/N/A route matrix |
| Packaged voice | Microphone capture works with configured local assets and inserts an editable transcript at the caret | NOT RUN | Record configured CLI/model versions and observation |
| Voice denial/failure | Missing permission, device, engine, model, silence, or inference failure leaves typed text usable and unchanged | NOT RUN | Record exercised failure and observation |
| Voice retention | Temporary WAV and transcript output files are deleted after success and failure | NOT RUN | Record inspected local-data path and observation |
| Installer/migration | A pre-v1 database produces a verified `.v1.bak` and retains project data | NOT RUN | Record source and migrated database checks |

## Release verdict rule

Human-gated v1 can be marked accepted only when `npm run check` passes, every applicable final observation above is recorded as PASS on the target packaged build, and every unsupported provider-route cell is explicitly recorded as N/A. Autonomous mode cannot inherit that verdict.
