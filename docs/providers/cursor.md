# Cursor Agent provider

## Local proof

- Cursor editor launcher: installed
- Cursor Agent executable: `%LOCALAPPDATA%\cursor-agent\cursor-agent.cmd`
- Version tested: `2026.07.23-e383d2b`
- Runtime state: ready
- Live-help proof: print mode, stream JSON, force write, sandbox flag, and resume
- Sandbox result, 2026-07-27: Cursor Agent `2026.07.23-e383d2b` rejects `--sandbox enabled` on Windows because sandboxing requires macOS or Linux.

The Staff Room searches `PATH` first and then the standard Windows Cursor Agent installer directory. The editor launcher is never substituted for the automation CLI.

## Implemented command contract

- Non-interactive turn: `cursor-agent --print`
- Structured stream: `--output-format stream-json`
- Windows sandbox: omit `--sandbox`; do not run `agent sandbox disable` and do not modify Cursor's global configuration
- Read-only Ask: `--mode ask`, Cursor's native read-only mode
- Write mode: `--force`, used only for build/revision inside the managed worktree
- Review mode: print mode without `--force`
- Model selection: run `cursor-agent models`, present its returned compound identifiers verbatim, and pass the selected identifier unchanged with `--model`. Do not synthesize bracket parameters: this installed version rejects them. Effort, thinking, and speed are encoded in the identifier, so the Cursor Effort control is disabled.
- Session resume: `--resume <chat-id>`
- Project permission rules remain authoritative; deny rules win
- Cancellation: terminate only the child process owned by the run

## Capability result

`isolated-auto`: unattended writes are confined to The Staff Room's managed branch and worktree, with Cursor project deny rules still enforced. Capability chip: **"sandbox unavailable on Windows, read-only enforced by ask mode"**.
