# Cursor Agent provider

## Local proof

- Cursor editor launcher: installed
- Cursor Agent executable: `%LOCALAPPDATA%\cursor-agent\cursor-agent.cmd`
- Version tested: `2026.08.04-aaa8809`
- Runtime state: ready
- Live-help proof: print mode, stream JSON, force write, sandbox flag, and resume
- Sandbox result, rechecked for the current Windows route: Cursor Agent rejects `--sandbox enabled` because sandboxing requires macOS or Linux.

The Staff Room searches `PATH` first and then the standard Windows Cursor Agent installer directory. The editor launcher is never substituted for the automation CLI.

## Implemented command contract

- Non-interactive turn: `cursor-agent --print`
- Structured stream: `--output-format stream-json`
- Windows sandbox: omit `--sandbox`; do not run `agent sandbox disable` and do not modify Cursor's global configuration
- Scoped repository trust: pass `--trust` only after The Staff Room has canonicalized the user-attached repository or created its managed isolation; writes remain confined to the managed worktree
- Read-only Ask: `--mode ask`, Cursor's native read-only mode
- Write mode: `--force`, used only for build/revision inside the managed worktree
- Review mode: `--mode ask` with no `--force`, followed by The Staff Room's mutation guard
- Model selection: run `cursor-agent models`, present its returned compound identifiers verbatim, and pass the selected identifier unchanged with `--model`. Do not synthesize bracket parameters: this installed version rejects them. Effort, thinking, and speed are encoded in the identifier, so the Cursor Effort control is disabled.
- Session resume: `--resume <chat-id>`
- Project permission rules remain authoritative; deny rules win
- Cancellation: close the run-owned Windows Job Object so the direct process and every descendant terminate together

## Capability result

`isolated-auto`: unattended writes are confined to The Staff Room's managed branch and worktree, with Cursor project deny rules still enforced. Capability chip: **"sandbox unavailable on Windows, read-only enforced by ask mode"**.
