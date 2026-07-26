# Cursor Agent provider

## Local proof

- Cursor editor launcher: installed
- Cursor Agent executable: `<user-home>\AppData\Local\cursor-agent\cursor-agent.cmd`
- Version tested: `2026.07.23-e383d2b`
- Runtime state: ready
- Live-help proof: print mode, stream JSON, force write, sandbox, and resume

Agent Room searches `PATH` first and then the standard Windows Cursor Agent installer directory. The editor launcher is never substituted for the automation CLI.

## Implemented command contract

- Non-interactive turn: `cursor-agent --print`
- Structured stream: `--output-format stream-json`
- Write mode: `--force`, used only for build/revision inside the managed worktree
- Review mode: print mode without `--force`
- Session resume: `--resume <chat-id>`
- Project permission rules remain authoritative; deny rules win
- Cancellation: terminate only the child process owned by the run

## Capability result

`isolated-auto`: unattended writes are confined to Agent Room's managed branch and worktree, with Cursor project deny rules still enforced.
