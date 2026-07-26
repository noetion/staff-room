# Claude Code provider

## Local proof

- Executable: `<user-home>\.local\bin\claude.exe`
- Version tested: `2.1.220 (Claude Code)`
- Runtime state: ready
- Live-help proof: print mode, stream JSON, `permission-mode` choice `auto`, and resume

## Implemented command contract

- Non-interactive turn: `claude --print`
- Stream: `--output-format stream-json --verbose`
- Autonomous permission mediation: `--permission-mode auto`
- Session resume: `--resume <session-id>`
- Prompt transport: stdin
- Cancellation: terminate only the child process owned by the run

Claude Code 2.1.220 no longer advertises `--max-turns`, so Agent Room does not pass that flag. The coordinator supplies the 20-minute phase timeout, five-minute idle-output timeout, and revision/review limits.

## Capability result

`reviewed-auto`: Claude Code's native auto permission mode mediates tool approvals without requiring the user to watch the run.
