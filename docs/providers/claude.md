# Claude Code provider

## Local proof

- Executable: `%USERPROFILE%\.local\bin\claude.exe`
- Version tested: `2.1.221 (Claude Code)`
- Historical v1 runtime state: ready; current detection requires the exact version and complete help contract
- Live-help proof: print mode, stream JSON, `permission-mode` choice `auto`, and resume

## Implemented command contract

- Non-interactive turn: `claude --print`
- Stream: `--output-format stream-json --verbose`
- Supported read-only mode: `--permission-mode plan --tools Read,Grep,Glob`
- Quick Edit and Ship writes: disabled before dispatch
- Session resume: `--resume <session-id>`
- Prompt transport: stdin
- Cancellation: close the run-owned Windows Job Object so the direct process and every descendant terminate together

Claude Code 2.1.221 does not advertise `--max-turns`, so The Staff Room does not pass that flag. The coordinator supplies the 20-minute phase timeout, five-minute idle-output timeout, and revision/review limits.

## Capability result

`manual`: supported read-only turns remain available after command checks. Native auto permission mediation is not an enforced filesystem boundary, so write routes are disabled. See [boundary acceptance](../PROVIDER_BOUNDARY_ACCEPTANCE.md).

## Warm-session evaluation

- 2026-07-26, `Claude Code 2.1.220`: the stream-input probe emitted the expected session initialization event, then retried because this environment reported `apiKeySource: none` and produced no completed turn within 90 seconds. Claude remains `warm_session: false` until an authenticated two-turn test proves persistent input, recovery, and timing receipts.
