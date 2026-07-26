# Agent Room plan evaluation

## Revised verdict

Proceed with a narrow autonomous desktop product.

Agent Room exists to remove manual transfer of project context and coding-agent responses. It coordinates four existing CLIs; it does not replace them or grow into an IDE.

The implementation source of truth is [V1_REVISED_PLAN.md](V1_REVISED_PLAN.md).

## Retained decisions

- One local project and repository are the room boundary.
- Explicit `@agent` selection always wins.
- The coordinator owns routing, limits, verification, promotion, and completion.
- Only one writer operates on a managed worktree at a time.
- A different provider reviews when available.
- Same-provider review is allowed only as a visible capability downgrade.
- One automatic revision and two reviews are the hard maximum.
- Context packets contain selected deltas, not complete transcripts.
- Native provider sessions are resumed rather than replayed.
- SQLite is operational truth.
- Provider capability and autonomy modes are displayed truthfully.
- Tauri commands and events remain the only desktop boundary.

## Removed or deferred

- generic adapter/plugin SDK;
- LLM routing on every transition;
- task classifiers and workflow builders;
- cloud, mobile, remote control, PWA, and HTTP/WebSocket layers;
- issue tracking and task DAGs;
- multi-human collaboration;
- semantic memory and analytics;
- automatic push, pull request, deployment, or external integrations;
- decorative GPU effects or non-CSS glass.

## Working v1 gate

```text
objective
-> isolated builder
-> verification
-> independent reviewer or honest same-provider fallback
-> one bounded revision
-> final review
-> safe promotion
-> completion or attention notification
```

Nothing beyond this loop is required to solve the product's primary problem.
