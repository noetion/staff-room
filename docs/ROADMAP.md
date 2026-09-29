# Roadmap

The roadmap focuses on strengthening the human-gated workflow and documenting the evidence required for larger capabilities. It does not assign release dates.

## Experimental pre-1.0 development

- Current source and package version: `0.1.0`. No supported stable binary is available.
- Complete current Windows installed-app and provider-boundary acceptance before recommending a release; see [the remaining gates](PROVIDER_BOUNDARY_ACCEPTANCE.md).
- Preserve earlier `v1.0.0` observations as historical evidence. They do not qualify the current implementation or its restricted provider matrix as stable.

## Human-gated hardening

- Extend Windows process-tree ownership from providers to verification commands.
- Minimize child-process environments and make verification-command authority explicit.
- Extend deterministic hostile-provider fixtures for malformed, oversized, silent, and racing behavior.
- Persist promotion leases so restart behavior is as explicit as in-process behavior.
- Improve packaged recovery and migration tests.

## Autonomous tier

Autonomous Ship is planned as a separate tier and remains unavailable while any mandatory row in [AUTONOMOUS_ACCEPTANCE_CONTRACT.md](AUTONOMOUS_ACCEPTANCE_CONTRACT.md) is incomplete. Work toward that tier must produce current evidence for process ownership, repository trust, hook and filter control, environment minimization, output bounds, review fingerprints, atomic cancellation and promotion, verification policy, destructive cleanup, deterministic coordinator E2E, packaged Windows E2E, and live provider smoke tests.

The tier becomes available only when the full contract passes and returns to unavailable if a required control regresses.

## Outside the current development scope

- macOS and Linux support, which require a separately verified custody model;
- cloud speech or hosted transcript storage;
- automatic promotion;
- a generic provider plugin system, which would require an active use case and an enforceable capability contract.
