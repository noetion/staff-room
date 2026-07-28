# Agent Room

Agent Room is a local-first Windows desktop room for working with installed coding-agent CLIs against an attached Git repository.

The v1 safety model is assisted and human-gated:

- Ask runs read-only and does not create a worktree.
- Quick Edit runs in an isolated worktree and shows an open diff review. Nothing reaches the attached checkout until the user presses Apply.
- Ship runs Build, Verify, read-only Review, one bounded revision when needed, and Final Review in managed isolation.
- A successful Ship stops at `awaiting-promotion`. Nothing reaches the attached checkout until the user presses Promote and confirms the action.
- Autonomous Ship is locked until the separate autonomous acceptance contract passes.
- Local push-to-talk inserts editable text at the composer caret. Dictation never sends a message or authorizes Apply, Promote, Discard, or Abandon.

## Trust boundary

Rust owns attached repository roots and operation IDs. Agent commands no longer accept repository paths chosen by the renderer. Every repository action reloads the project from SQLite, canonicalizes its Git root, and verifies that its identity still matches the attached project.

Review uses an explicit read-only provider mode. Connection tests use a non-writing probe and accept only an exact `READY` response. Cursor is not granted unconditional workspace trust.

SQLite is backed up through SQLite's online backup API before the legacy migration. The backup is integrity-checked before it is atomically finalized.

## Voice setup

Voice is optional and local. Open Settings, then choose:

1. a trusted `whisper-cli` executable built from whisper.cpp;
2. a compatible whisper.cpp ggml `.bin` model.

The files are copied into Agent Room's local application-data directory. Microphone capture is capped at 30 seconds and local transcription at two minutes. Audio is written only to a temporary WAV for the local CLI invocation and is deleted after success, failure, or timeout. The transcript remains in the composer until the user reviews and sends it.

Debug builds also accept `AGENT_ROOM_WHISPER_CLI` and `AGENT_ROOM_WHISPER_MODEL` paths for local development and test fixtures. Release builds require assets selected through Settings so inherited environment variables or `PATH` cannot silently choose an executable.

Agent speech, text-to-speech, per-agent voices, and realtime voice conversation are not part of v1. See [docs/VOICE.md](docs/VOICE.md).

## Run

```powershell
npm install
npm run tauri dev
```

Frontend-only preview:

```powershell
npm run dev
```

The browser preview never starts provider CLIs or microphone capture.

## Verify

The aggregate local gate is:

```powershell
npm run check
```

It runs frontend tests and build, design-token checks, Rust tests, and the visual QA harness without paid provider calls.

## Release boundaries

- Windows-first personal-use v1.
- Existing local coding CLIs provide model access and authentication.
- No cloud speech service or speech API key.
- No automatic promotion.
- No accepted autonomous mode until [docs/AUTONOMOUS_ACCEPTANCE_CONTRACT.md](docs/AUTONOMOUS_ACCEPTANCE_CONTRACT.md) is fully evidenced.
- Live provider and packaged microphone observations remain part of the final audit.

See [docs/V1_ACCEPTANCE.md](docs/V1_ACCEPTANCE.md) for the assisted contract and [docs/V1_IMPLEMENTATION_PLAN.md](docs/V1_IMPLEMENTATION_PLAN.md) for the implementation decision.
