# Agent Room v1 implementation plan

**Decision date:** 28 July 2026
**Target:** Human-gated v1 with local voice dictation, plus an explicit autonomous acceptance contract
**Default safety posture:** Assisted. No change reaches the attached checkout until the user presses Promote.

## Outcome

Deliver a usable Windows-first v1 that:

1. fixes the confirmed interface and cross-project correctness defects;
2. binds filesystem and repository authority to Rust-owned state;
3. runs review providers read-only;
4. makes promotion an explicit human action by default;
5. supports local microphone dictation into the editable composer without auto-send;
6. keeps autonomous operation opt-in and unavailable unless its stricter acceptance contract is satisfied;
7. exposes one claim-focused verification command and truthful acceptance records.

## Non-goals

- Replacing the existing provider CLIs.
- Rewriting the application or Git isolation model.
- Cloud speech-to-text, text-to-speech, per-agent voices, or realtime conversation.
- Automatic voice submission or voice authorization of Promote, Apply, Discard, Abandon, or other consequential actions.
- Broad dependency upgrades unrelated to v1.
- macOS/Linux distribution claims before platform-specific acceptance exists.
- Paid provider runs during ordinary development verification.

## Acceptance contracts

### Assisted v1

- Human messages and core controls render correctly in both themes.
- Quick Edit is visible, reviewable, applicable, and discardable.
- Async results cannot cross project or run boundaries.
- Every displayed provider can connect or is visibly unavailable.
- Reviews are read-only.
- Operation IDs and repository roots are Rust-owned and path-contained.
- SQLite backup is WAL-safe and restorable.
- Promotion requires an explicit user action and revalidates the base checkout.
- Local dictation inserts an editable transcript at the caret and never sends.
- Provider adapter fixtures, repository-safety tests, and one local verification command pass.
- README, UI, and assisted acceptance evidence agree.

### Autonomous mode

Autonomous mode remains opt-in and cannot be described as accepted until the separate autonomous contract passes. That contract includes:

- versioned provider capability evidence;
- process-tree ownership and orphan-free cancellation;
- repository trust, coordinator hook suppression, and minimal environment inheritance;
- full content review fingerprinting;
- atomic cancellation/promotion state;
- bounded provider output;
- packaged Windows E2E and current live provider smoke evidence;
- truthful completion of the applicable original 29-row acceptance matrix.

## Implementation sequence

1. Restore UI foundations and strengthen visual assertions.
2. Repair Quick Edit, composer semantics, and project/run state ownership.
3. Freeze unsafe provider claims and introduce read-only review.
4. Move operation and repository authority into Rust.
5. Make migration/backup and promotion safe.
6. Fail-close autonomous mode and publish its remaining deterministic-provider acceptance gates.
7. Add local Rust microphone capture and local transcription.
8. Reconcile documentation and acceptance records.

## Risks

- Existing uncommitted QA and redesign-step files belong to the user and must be preserved.
- Loading the dormant primitive stylesheet may expose overlay and specificity conflicts.
- IPC request changes touch frontend and Rust together and require coordinated migration.
- Windows process-tree control and local transcription add platform-specific Rust dependencies.
- A bundled in-process whisper runtime required unavailable CMake/libclang tooling, so v1 uses Rust capture with a user-supplied local whisper.cpp CLI and model. Distribution provenance remains a final-audit item.
- Existing live-provider behavior cannot be claimed without opt-in authenticated checks.

## Rollback

- Keep changes in coherent, reviewable slices.
- Preserve current request shapes until each frontend/backend migration compiles together.
- Keep autonomous mode default-off.
- Keep voice behind an availability check; a transcription initialization failure must leave typed input fully functional.
- Do not migrate or delete user data without a pre-migration SQLite backup and restore test.

## Verification strategy

During implementation, run only the smallest check that proves each changed contract:

- focused Vitest or Rust tests for local logic;
- `npm run build` for renderer/type integration;
- focused browser checks for UI and contrast;
- fake-provider integration for coordinator behavior;
- packaged Windows smoke only when platform behavior is introduced.

Before declaring v1 accepted, run the aggregate local gate once and update the acceptance records. Paid live-provider runs and the broad final audit remain separate final activities.
