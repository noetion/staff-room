# Cursor chat timeout RCA

Status: implemented and verified on `feature/cursor-chat-timeout-fix`.

## Symptom

The room shows `Chat produced no output for 300 seconds and was stopped.` after an `@cursor` message. Settings can still show Cursor as Connected while the selected model is `cursor-grok-4.5`.

## Evidence

The recent durable run artifacts under the Agent Room local-data directory show:

- `chat.stdout.log` is empty.
- `chat.stderr.log` begins with `Cannot use this model: cursor-grok-4.5` and includes the current available identifiers, including `cursor-grok-4.5-high`, `cursor-grok-4.5-medium`, and `cursor-grok-4.5-low`.
- The command adapter passes the saved Cursor model unchanged through `--model`.
- Cursor profiles are free-text inputs. The datalist labels unavailable saved values, but the save path did not reject an unavailable value.
- The process runner only used stderr to reset the idle timer. It did not classify a fatal Cursor model/authentication line as a terminal provider failure, so the process remained under the five-minute idle watchdog.

## Root cause

An obsolete or incomplete Cursor model identifier was persisted as a valid profile. The provider rejected it immediately, but the runner had no early-failure path for Cursor's fatal stderr diagnostics. The visible timeout was therefore a secondary symptom, and the saved `Connected` status did not prove that the selected model could execute.

## Solution acceptance

- Cursor model profiles cannot save a value absent from a successful authoritative catalogue.
- A stale saved Cursor model is cleared when Cursor reports that it cannot use that model, so the next attempt can use the provider default or a refreshed exact identifier.
- Fatal Cursor model and authentication diagnostics stop the owned process promptly and produce an actionable message instead of a five-minute timeout.
- Valid exact Cursor identifiers continue to pass through unchanged.
- Unit coverage verifies model validation, fatal-diagnostic classification, and the existing exact-command contract.

## Scope

The fix stays within Cursor profile validation, chat failure handling, and the provider process boundary. It does not change the five-minute general idle policy for providers that are still producing legitimate progress or warnings.

## Implemented solution

- The settings save path now rejects a non-empty model value when a successful authoritative catalogue is loaded and marks the field invalid so the user can choose an exact identifier.
- Model refresh continues to clear stale saved routes after a successful catalogue refresh. A refresh failure removes the failed catalogue from local state instead of treating it as authoritative.
- The provider runner classifies Cursor model, catalogue, and authentication failures from stderr as terminal diagnostics. It terminates the owned process immediately, so a rejected model cannot occupy the idle watchdog for five minutes.
- Chat extracts the rejected model identifier, clears the stale Cursor chat profile, records the failure, and tells the user to refresh models and select an exact identifier.

## Verification

- The frontend suite passes: 24 tests.
- The Rust suite passes: 59 tests, including fatal stderr classification, actionable model extraction, and exact Cursor command forwarding.
- The production frontend build, Rust formatting check, and `git diff --check` pass.

## Remaining limitation

The installed Cursor CLI returned `Failed to load models: [internal]` during this investigation, and its status/about commands reported inconsistent account details. That environment prevented a live confirmation of the current remote catalogue. The application now surfaces that discovery failure and avoids treating it as a valid catalogue; the CLI's account/catalogue service remains an external dependency.
