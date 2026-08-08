---
name: autonomous-ship
description: Use when The Staff Room is armed and a substantial or unattended repository change needs isolation, verification, review, and promotion. Do not use for questions, explanations, bounded quick edits, reviews without requested edits, brainstorming, or ambiguous conversation.
---

# Autonomous Ship

The Staff Room Ask route is read-only. Small, explicit edits belong in Quick Edit, which works in managed isolation and shows a diff before Apply can update the attached checkout. Do not trigger Ship for a bounded update to one or a few known files that can be completed safely in Quick Edit.

Trigger only when the user's current request clearly requires substantial or unattended repository work, contains enough information to state one concrete engineering objective, and materially benefits from The Staff Room's isolated build, verification, independent review, and promotion route. Never infer a code-changing objective from an information-only question.

When the request is a bounded quick edit, use the current Quick Edit route and answer normally. Emit no Ship markers.

When Ship is required, do not modify files in Chat. Briefly acknowledge the transition and return exactly one JSON intent between these markers:

```text
STAFF_ROOM_SHIP_INTENT_START
{"schemaVersion":1,"objective":"One concrete repository objective","reason":"Why the current request requires repository changes"}
STAFF_ROOM_SHIP_INTENT_END
```

Keep the objective self-contained, bounded, and faithful to the user's request. Do not add deployment, publishing, external communication, destructive migration, or unrelated cleanup.

The intent is a proposal to The Staff Room, not permission to write or merge. The Staff Room owns the isolated worktree, verification, independent review, bounded revision, recovery, and promotion gates.

If the skill does not trigger, answer normally and emit no markers.
