# Roadmap

The roadmap focuses on strengthening the human-gated workflow and documenting the evidence required for larger capabilities. It does not assign release dates.

## Public v1.0

- Complete packaged Windows acceptance for the human-gated Ask, Quick Edit, Ship, migration, and local voice flows.
- Validate the supported route matrix against current installed Claude Code, Codex, Cursor, and Antigravity versions.
- Publish source, reproducible verification evidence, and the unsigned-installer caveat.

## Human-gated hardening

- Own complete provider process trees during cancellation, timeout, crash, and application exit.
- Minimize child-process environments and make verification-command authority explicit.
- Extend deterministic hostile-provider fixtures for malformed, oversized, silent, and racing behavior.
- Persist promotion leases so restart behavior is as explicit as in-process behavior.
- Improve packaged recovery and migration tests.

## Autonomous tier

Autonomous Ship is planned as a separate tier and remains unavailable while any mandatory row in [AUTONOMOUS_ACCEPTANCE_CONTRACT.md](AUTONOMOUS_ACCEPTANCE_CONTRACT.md) is incomplete. Work toward that tier must produce current evidence for process ownership, repository trust, hook and filter control, environment minimization, output bounds, review fingerprints, atomic cancellation and promotion, verification policy, destructive cleanup, deterministic coordinator E2E, packaged Windows E2E, and live provider smoke tests.

The tier becomes available only when the full contract passes and returns to unavailable if a required control regresses.

## Outside the current v1 scope

- macOS and Linux support, which require a separately verified custody model;
- cloud speech or hosted transcript storage;
- automatic promotion;
- a generic provider plugin system, which would require an active use case and an enforceable capability contract.
