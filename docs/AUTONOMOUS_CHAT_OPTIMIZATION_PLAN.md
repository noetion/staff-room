# Autonomous Chat optimization

## Outcome

Make project Chat materially faster and context-efficient, allow an armed project room to transition from Chat into the existing Ship workflow through an agent-recognized skill, and keep Agent Room authoritative for every code-changing gate.

## Non-goals

- Do not let a provider write in the active checkout during Chat.
- Do not let a provider merge, bypass verification, or weaken review policy.
- Do not replay the full room or duplicate context already retained by a provider session.
- Do not depend on an embedded provider TUI.
- Do not require every provider to expose the same transport capability.

## Acceptance criteria

1. Warm Chat does not re-run version, help, and authentication probes for all providers.
2. A provider switch receives a bounded recent handoff; a resumed same-provider turn receives only the new message.
3. Agent Room records preflight, first-output, and total latency when observable.
4. Cursor Chat uses its proved ask, sandbox, and partial-stream modes.
5. Successful raw log paths stay in Evidence rather than the primary conversation.
6. Chat, builder, and reviewer model profiles can differ.
7. Autonomous Ship is a persistent per-project choice.
8. When autonomy is armed, an agent may emit a schema-validated Ship intent by applying the repository skill.
9. The intent starts the existing isolated Ship route without another approval, while all verification, review, retry, and promotion gates remain unchanged.
10. Ambiguous conversation and information-only questions remain Chat.

## Risks and rollback

- A false-positive Ship intent could start unwanted code work. Mitigation: only a valid explicit marker from the injected skill is accepted, autonomy must be armed, and all writes occur in a managed worktree.
- Cross-provider context could grow. Mitigation: include only the last relevant exchange and cap it at 4 KiB.
- Capability caching could become stale after a CLI update. Mitigation: explicit provider refresh and invalidation after a launch failure.
- Provider-specific flags may drift. Mitigation: use only locally proved flags and retain one-shot fallbacks.

Rollback is removal of the autonomous intent parser and project setting; the existing explicit Ship route remains intact.

## Verification

- Unit tests for bounded handoffs, same-provider deduplication, intent parsing, autonomy persistence, timing serialization, and provider command flags.
- Existing Rust and frontend tests, clippy, formatting, type-check, and production build.
- A no-quota cache test and code-path inspection proving warm Chat avoids the previously measured 2.67-second all-provider probe path.
- UI exercise of autonomy settings, route-specific models, and quiet diagnostics.
- Live provider and automatic-transition smoke tests remain opt-in because they consume account quota and can start repository work.
