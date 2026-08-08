# Roadmap

The Staff Room roadmap is a sequence of custody claims to prove, not a promise of dates or feature volume.

## Public v1.0

- Complete packaged Windows acceptance for the human-gated Ask, Quick Edit, Ship, migration, and local voice flows.
- Validate the supported route matrix against current installed Claude Code, Codex, Cursor, and Antigravity versions.
- Publish source, reproducible verification evidence, and the unsigned-installer caveat.

## Human-gated hardening

- Own complete provider process trees during cancellation, timeout, crash, and application exit.
- Minimize child-process environments and make verification-command authority explicit.
- Extend deterministic hostile-provider fixtures for malformed, oversized, silent, and racing behavior.
- Persist promotion leases so restart behavior is as explicit as in-process behavior.
- Improve packaged recovery and migration tests without widening platform scope.

## Autonomous tier

Autonomous Ship is a separate tier, not a hidden flag in v1. It remains unavailable while any mandatory row in [AUTONOMOUS_ACCEPTANCE_CONTRACT.md](AUTONOMOUS_ACCEPTANCE_CONTRACT.md) is incomplete. Work toward that tier must produce current evidence for process ownership, repository trust, hook and filter control, environment minimization, output bounds, review fingerprints, atomic cancellation and promotion, verification policy, destructive cleanup, deterministic coordinator E2E, packaged Windows E2E, and live provider smoke tests.

Passing part of that list does not unlock the tier. A regression returns it to the locked state.

## Not planned for the v1 line

- macOS or Linux support without a separately verified custody model;
- cloud speech or hosted transcript storage;
- automatic promotion;
- a generic provider plugin system without a current consumer and enforceable capability contract.
