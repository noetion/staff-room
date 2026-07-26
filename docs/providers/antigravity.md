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
- Headless autonomy: `--dangerously-skip-permissions`, always paired with `--sandbox` because print mode cannot present a permission prompt
- Session resume: `--conversation <id>`
- Chat and Review remain instruction-level read-only while using the same sandboxed headless permission contract
- Cancellation: terminate only the child process owned by the run

The installed CLI print surface is text-first. Agent Room does not claim streaming, structured output, or usage reporting because version `1.1.7` does not expose those capabilities in live help. Provider-specific session history recovery remains out of scope.

## Capability result

`unattended-bypass`: live help proves print mode, sandboxing, permission bypass, and exact conversation resume. Permission bypass remains a visible downgrade and is paired with Antigravity sandbox mode plus Agent Room's managed worktree boundary.
# Step 5 live verification — 2026-07-26, agy 1.1.7

`agy models` could not return the signed-in model catalogue because the CLI reported that this desktop user is not signed in. The display-name-to-slug check therefore remains deferred to the existing account-backed `agy models` refresh fallback; no speculative slug mapping was added.
