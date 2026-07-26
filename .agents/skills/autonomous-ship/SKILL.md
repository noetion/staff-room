---
name: autonomous-ship
description: Use in an armed Agent Room when a substantial or unattended repository change needs isolation, verification, review, and promotion. Do not use for questions, explanations, bounded quick edits, reviews without requested edits, brainstorming, or ambiguous conversation.
---

# Autonomous Ship

Agent Room Chat may perform small, explicit edits directly in the attached checkout. Do not trigger Ship for a bounded update to one or a few known files that can be completed safely in the current supervised chat.

Trigger only when the user's current request clearly requires substantial or unattended repository work, contains enough information to state one concrete engineering objective, and materially benefits from Agent Room's isolated build, verification, independent review, and promotion route. Never infer a code-changing objective from an information-only question.

When the request is a bounded quick edit, make the change in Chat and answer normally. Emit no Ship markers.

When Ship is required, do not modify files in Chat. Briefly acknowledge the transition and return exactly one JSON intent between these markers:

```text
AGENT_ROOM_SHIP_INTENT_START
{"schemaVersion":1,"objective":"One concrete repository objective","reason":"Why the current request requires repository changes"}
AGENT_ROOM_SHIP_INTENT_END
```

Keep the objective self-contained, bounded, and faithful to the user's request. Do not add deployment, publishing, external communication, destructive migration, or unrelated cleanup.

The intent is a proposal to Agent Room, not permission to write or merge. Agent Room owns the isolated worktree, verification, independent review, bounded revision, recovery, and promotion gates.

If the skill does not trigger, answer normally and emit no markers.
