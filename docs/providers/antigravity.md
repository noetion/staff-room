# Antigravity provider

**Current policy:** execution, including connection probes and reviews, is disabled before dispatch. The historical contract below combines a terminal restriction with permission bypass and does not prove a filesystem write boundary or a trustworthy read-only route. A managed worktree is not containment. See [boundary acceptance](../PROVIDER_BOUNDARY_ACCEPTANCE.md).

## Local proof

- Antigravity Desktop: installed under `%LOCALAPPDATA%\Programs\antigravity\Antigravity.exe`
- `agy` automation executable: version `1.1.11`, installed under `%LOCALAPPDATA%\agy\bin\agy.exe`
- Live read-only smoke: `agy --print ... --sandbox --mode plan` returned the expected response
- Runtime state: ready with declared capability downgrades

The desktop application is not substituted for the automation CLI. The Staff Room probes `agy` on `PATH` and the installer location above, so a terminal restart is not required for discovery.

## Historical v1 command contract (disabled)

- Non-interactive turn: `agy --print`
- Terminal restriction: `--sandbox`
- Headless autonomy: `--dangerously-skip-permissions`, always paired with `--sandbox` because print mode cannot present a permission prompt
- Session resume: `--conversation <id>`
- Ask and Quick Edit are unavailable because Antigravity exposes no trustworthy non-writing route; the UI labels it Ship-only
- Review stays in plan mode with the sandboxed headless permission contract and a post-run mutation guard
- Cancellation: close the run-owned Windows Job Object so the direct process and every descendant terminate together

The installed CLI print surface is text-first. The Staff Room does not claim streaming, structured output, or usage reporting because version `1.1.11` does not expose those capabilities in live help. Provider-specific session history recovery remains out of scope.

## Capability result

`manual`: all routes are disabled. Historical help established syntax only; it did not establish filesystem containment.
## Stable live verification - 2026-08-10, agy 1.1.11

The signed-in `agy models` command returned exact account model identifiers, including `gemini-3.5-flash-low`. Build, Review, cancellation, and connection routes passed on a synthetic repository. Ask and Quick Edit remain visibly N/A.
