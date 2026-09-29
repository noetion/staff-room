# Provider boundary acceptance

The v1 release records describe historical observations, not acceptance of the current safety changes. Installed, command-compatible, signed in, and live-connected are separate states. Discovery alone never sets runtime state to ready. Unknown versions and incomplete or failed help probes disable execution; updating a CLI requires reviewing its command contract before extending the exact-version table.

| Recorded version | Ask / Probe / Review | Quick Edit / Ship write | Reason |
| --- | --- | --- | --- |
| Codex `codex-cli 0.144.4` | Required help plus read-only sandbox | Required help plus workspace-write sandbox | Retains the documented provider-managed filesystem sandbox and never-ask approval policy |
| Claude `2.1.221 (Claude Code)` | Required help plus plan mode and `Read,Grep,Glob` tools | Disabled | Auto permission mediation does not prove filesystem containment |
| Cursor `2026.08.04-aaa8809` | Required help plus native ask mode | Disabled | Windows sandbox is unavailable; other platforms have no accepted write-boundary record in this Windows application |
| Antigravity `1.1.11` | Disabled | Disabled | Terminal sandbox plus permission bypass proves neither filesystem confinement nor a trustworthy read-only route |

Each local version/help/status probe has a five-second deadline and a 256 KiB combined output limit. Stdout is preferred over stderr; unsuccessful exits and invalid UTF-8 fail closed. Codex checks root, exec, and resume help separately, so its four detection probes can take up to about twenty seconds. No model calls occur during detection. The Windows process Job Object owns and terminates probe descendants. Non-Windows probe cleanup only terminates the direct child; non-Windows support is not claimed.

The exact versions come from `docs/providers` and the v1 acceptance record. Help checks validate required syntax, not the semantic behavior of arbitrary executables claiming those versions. The installed executable remains a trust boundary. Native read-only modes and filesystem sandboxes do not establish network or data-exfiltration protection.

## Acceptance still required

- Run the existing hosted Windows gate for this revision, including Rust tests and frontend checks.
- In a throwaway repository with the pinned, authenticated Codex binary, verify workspace writes succeed and attempted absolute-path writes to the attached original checkout and a sibling fixture are denied. A fake CLI cannot establish that real provider sandbox guarantee.
- Exercise the installed Windows binary: provider status/selection, Ask, permitted Quick Edit, changed-diff rejection, cancellation, and human Apply/Promote. Repeat after any command-contract change.

These native and installed-binary checks have not been performed for this change. They remain release blockers. Historical v1 process-custody and successful-turn records do not satisfy the absolute-path containment test. Autonomous mode remains locked under its separate acceptance contract.
