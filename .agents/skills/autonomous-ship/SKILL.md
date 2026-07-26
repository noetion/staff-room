---
name: autonomous-ship
description: Use in an armed Agent Room when the user explicitly asks to build, implement, fix, change, refactor, test, or otherwise modify the attached repository. Do not use for questions, explanations, reviews without requested edits, brainstorming, or ambiguous conversation.
---

# Autonomous Ship

Chat is read-only. Do not modify the repository while applying this skill.

Trigger only when the user's current request clearly requires repository changes and contains enough information to state one concrete engineering objective. Never infer a code-changing objective from an information-only question.

When triggered, briefly acknowledge the transition and return exactly one JSON intent between these markers:

```text
AGENT_ROOM_SHIP_INTENT_START
{"schemaVersion":1,"objective":"One concrete repository objective","reason":"Why the current request requires repository changes"}
AGENT_ROOM_SHIP_INTENT_END
```

Keep the objective self-contained, bounded, and faithful to the user's request. Do not add deployment, publishing, external communication, destructive migration, or unrelated cleanup.

The intent is a proposal to Agent Room, not permission to write or merge. Agent Room owns the isolated worktree, verification, independent review, bounded revision, recovery, and promotion gates.

If the skill does not trigger, answer normally and emit no markers.
