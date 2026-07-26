# Antigravity provider

## Local proof

- Antigravity Desktop: installed at `<user-home>\AppData\Local\Programs\antigravity\Antigravity.exe`
- `agy` automation executable: not installed
- Runtime state: unavailable

The desktop application is not an automation CLI and is never invoked as a fallback.

## Implemented command contract

- Non-interactive turn: `agy --print`
- Terminal restriction: `--sandbox`
- Write mode: `--dangerously-skip-permissions`, used only for build/revision inside the managed worktree
- Session resume: `--conversation <id>`
- Review mode remains sandboxed and does not request permission bypass
- Cancellation: terminate only the child process owned by the run

The current documented CLI print surface is text-first. Agent Room does not claim streaming, structured output, or usage reporting until an installed version proves them. Provider-specific session history recovery remains out of scope.

## Capability result

When proven, `unattended-bypass`: permission bypass is a visible downgrade and is paired with Antigravity sandbox mode plus Agent Room's managed worktree boundary.
