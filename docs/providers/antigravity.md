# Antigravity provider

## Local proof

- Antigravity Desktop: installed at `<user-home>\AppData\Local\Programs\antigravity\Antigravity.exe`
- `agy` automation executable: version `1.1.7`, installed at `<user-home>\AppData\Local\agy\bin\agy.exe`
- Live read-only smoke: `agy --print ... --sandbox --mode plan` returned the expected response
- Runtime state: ready with declared capability downgrades

The desktop application is not substituted for the automation CLI. Agent Room probes `agy` on `PATH` and the installer location above, so a terminal restart is not required for discovery.

## Implemented command contract

- Non-interactive turn: `agy --print`
- Terminal restriction: `--sandbox`
- Write mode: `--dangerously-skip-permissions`, used only for build/revision inside the managed worktree
- Session resume: `--conversation <id>`
- Review mode remains sandboxed and does not request permission bypass
- Cancellation: terminate only the child process owned by the run

The installed CLI print surface is text-first. Agent Room does not claim streaming, structured output, or usage reporting because version `1.1.7` does not expose those capabilities in live help. Provider-specific session history recovery remains out of scope.

## Capability result

`unattended-bypass`: live help proves print mode, sandboxing, permission bypass, and exact conversation resume. Permission bypass remains a visible downgrade and is paired with Antigravity sandbox mode plus Agent Room's managed worktree boundary.
