# Codex provider report

## Local proof

- Executable: `<user-home>\AppData\Local\Programs\OpenAI\Codex\bin\codex.exe`
- Version: `codex-cli 0.144.4`
- Status: installed

## Adapter surface

- Non-interactive turn: `codex exec`
- Repository scope: `--cd <path>`
- Structured events: `--json` newline-delimited JSON
- Final response: `--output-last-message <path>`
- Structured final result: `--output-schema <path>`
- Session resume: `codex exec resume <session-id>`
- Sandbox: `--sandbox read-only|workspace-write|danger-full-access`
- Cancellation: owned child-process termination

## First-slice command shape

```text
codex exec
  --json
  --sandbox workspace-write
  --cd <repository>
  --output-last-message <app-cache-file>
  <objective>
```

The executable is launched directly with an argument array. No shell command string is evaluated.
