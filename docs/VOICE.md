# Voice design and operation

## V1: user dictation

Voice is an input convenience layered on the existing composer:

`microphone -> Rust capture -> temporary 16 kHz mono WAV -> local whisper.cpp CLI -> editable composer text`

It does not create a new agent route. The same reviewed text goes through Ask, Quick Edit, or Ship only after the user presses the existing send/action control.

### Safety and privacy

- Capture is push-to-talk and capped at 30 seconds.
- Local transcription is capped at two minutes.
- Audio stays on the local machine.
- No speech API key or cloud request exists.
- Temporary WAV and CLI transcript files are deleted after success, failure, or timeout.
- Known deviation: because whisper.cpp reads a file, captured audio does briefly reach disk in the
  app cache directory. It is deleted on every exit path, and `sweep_voice_cache` removes anything
  older than an hour before each transcription so a process killed mid-inference cannot leave audio
  behind. An in-process binding would remove this deviation entirely.
- Cancelled captures have their sample buffer overwritten before it is dropped.
- The current typed draft remains usable when permission, device, model, engine, silence, or inference fails.
- Voice never auto-sends and never authorizes Apply, Promote, Discard, Abandon, or provider selection.

### Local runtime

Settings accepts a trusted whisper.cpp `whisper-cli` executable and compatible ggml `.bin` model. Agent Room copies them to its local application-data voice directory. Debug builds also support these development and test-fixture overrides:

- `AGENT_ROOM_WHISPER_CLI`
- `AGENT_ROOM_WHISPER_MODEL`

Release builds ignore those overrides and inherited `PATH` lookup. The executable and model are not bundled in this repository. Their provenance and license must be verified before distribution.

## Can agents have voices?

Yes, technically. Final agent text can be sent to a local or cloud text-to-speech engine, with a stable voice mapped to each agent. That is not part of v1 because it adds autoplay, interruption, queueing, accessibility, privacy, latency, and cost decisions without improving repository custody.

A later TTS implementation should:

1. speak only finalized agent messages, never hidden reasoning, tool logs, diffs, secrets, or streaming fragments;
2. default off and require an explicit Play control;
3. stop immediately on user request, project switch, or new playback;
4. expose captions and preserve full keyboard/screen-reader access;
5. store per-agent voice mappings locally;
6. use a local engine by default, or disclose and separately consent to cloud transfer;
7. cap text length and never treat spoken output as an authorization channel.

Realtime voice conversation should remain a separate feature with its own interruption and consent model.
