# Cursor Agent provider

## Local proof

- Cursor editor launcher: installed
- Cursor Agent executable: `%LOCALAPPDATA%\cursor-agent\cursor-agent.cmd`
- Version tested: `2026.08.04-aaa8809`
- Historical v1 runtime state: ready; current write routes are disabled
- Live-help proof: print mode, stream JSON, force write, sandbox flag, and resume
- Sandbox result, rechecked for the current Windows route: Cursor Agent rejects `--sandbox enabled` because sandboxing requires macOS or Linux.

The Staff Room searches `PATH` first and then the standard Windows Cursor Agent installer directory. The editor launcher is never substituted for the automation CLI.

## Implemented command contract

- Non-interactive turn: `cursor-agent --print`
- Structured stream: `--output-format stream-json`
- Windows sandbox: omit `--sandbox`; do not run `agent sandbox disable` and do not modify Cursor's global configuration
- Repository trust: `--trust` suppresses a prompt after repository validation; it does not establish containment
- Read-only Ask: `--mode ask`, Cursor's native read-only mode
- Write mode: disabled before dispatch; Staff Room does not launch `--force` turns
- Review mode: `--mode ask` with no `--force`, followed by The Staff Room's mutation guard
- Model selection: run `cursor-agent models`, present its returned compound identifiers verbatim, and pass the selected identifier unchanged with `--model`. Do not synthesize bracket parameters: this installed version rejects them. Effort, thinking, and speed are encoded in the identifier, so the Cursor Effort control is disabled.
- Session resume: `--resume <chat-id>`
- Project permission rules remain authoritative; deny rules win
- Cancellation: close the run-owned Windows Job Object so the direct process and every descendant terminate together

## Capability result

`manual`: native ask mode remains available for the recorded version with complete help. Windows lacks Cursor sandbox support, so Quick Edit and Ship writes are rejected at the native dispatch boundary. Other platforms do not have accepted write-boundary evidence in this Windows application. Worktrees and project deny rules do not establish OS containment. See [boundary acceptance](../PROVIDER_BOUNDARY_ACCEPTANCE.md).
