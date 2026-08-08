# Public v1.0 release checklist

**Target outcome:** publish `jonathanjasare/staff-room` as a source-first, human-gated Windows v1.0 whose safety claims are reproducible and traceable to Rust enforcement.

**Current candidate:** `1.0.0-rc.1`

**Non-goals:** new product features, autonomous unlock, macOS or Linux support, code signing, automatic promotion, or publication without the final human gate.

## P0 gates

| Gate | Required evidence | Status |
| --- | --- | --- |
| Full-history secrets and privacy scan | `gitleaks git --redact` has no unresolved findings; generated history containing local paths is removed or explicitly approved | BLOCKED pending approved history rewrite |
| Repository identity | GitHub repository renamed to `staff-room`; old URL redirects; local remote updated | Human publication step |
| Product rename | Case-insensitive search leaves only documented legacy migration identifiers and changelog history | PASS on RC branch |
| Data migration | Tests prove the old SQLite database and voice assets are copied, integrity-checked, and not deleted | PASS: 2 focused migration tests plus aggregate Rust gate |
| License | Root MIT license plus npm and Cargo metadata | PASS |
| Demo | Synthetic-path GIF is at most 60 seconds and 5 MB, ends at `awaiting-promotion`, and passes frame review | PASS: 12.16 seconds, 2.55 MiB, frame-reviewed |
| README and architecture | First screen carries name, fixed tagline, demo, and invariant; enforcement symbols map to code | PASS on RC branch |
| Automated verification | Clean install and `npm run check` pass on Windows | PASS on 2026-08-08 |
| Dependency audit | `npm audit` and `cargo audit` have no unresolved advisory | PASS for Windows RC; warning disposition recorded below |
| Repository-size audit | Largest 20 historical objects classified; generated outputs removed from publishable history | BLOCKED pending approved history rewrite |
| Packaged build | `npm run tauri build` produces an installer named for The Staff Room | PASS: unsigned NSIS installer produced |
| Human-gated acceptance | Every applicable observation in `V1_ACCEPTANCE.md` is recorded PASS and every unsupported provider-route cell is recorded N/A against the candidate | Human acceptance step |
| Autonomous honesty | `AUTONOMOUS_ACCEPTANCE_COMPLETE` is `false`; README links the NOT ACCEPTED contract | PASS on RC branch |
| Security reporting | GitHub private vulnerability reporting enabled | Human publication step |
| GitHub metadata | Fixed tagline description and topics `tauri`, `rust`, `ai-agents`, `developer-tools`, `local-first` | Human publication step |

## History and repository-size audit

The pre-publication scan found generated redesign driver JSON and QA screenshots in existing Git history. The JSON contains absolute local paths and 40-character commit hashes that trigger the Sourcegraph-token detector. The current release tree deletes those generated artifacts, but deletion does not remove their historical blobs.

Before visibility changes, perform a reviewed `git filter-repo` rewrite against the exact generated paths, enumerate every remote branch and tag, and delete redundant refs or rewrite every retained ref so none keeps the old objects reachable. Rescan all rewritten refs, not only the future default branch, and force-push only after explicit approval. Preserve a private backup ref until the public repository and default branch are verified. This rewrite is intentionally not automated by the release branch because it changes existing commit identities and remote history.

The largest-object audit also found an accidentally committed Windows cache database in historical objects. Retained documentation revisions contain private absolute paths even though the candidate tree is sanitized. The approved rewrite therefore must cover generated redesign logs and QA, the cache database, and private-path-bearing historical revisions; deleting current files alone is insufficient.

## RC verification record — 2026-08-08

- `npm run check`: PASS — 24 frontend tests, production build, design lint across 81 files, 64 Rust tests, and 89 visual/accessibility checks.
- `npm audit`: PASS — 0 vulnerabilities.
- `cargo audit --file src-tauri/Cargo.lock`: PASS — 0 vulnerabilities. RustSec reported 18 allowed warnings: 16 unmaintained transitive crates and two unsound advisories that are absent from the supported Windows target dependency tree. Reassess before adding non-Windows support.
- `gitleaks dir --redact`: PASS — no findings in the candidate tree.
- `gitleaks git --redact`: BLOCKED — 284 Sourcegraph-token detector findings, all in generated redesign logs, plus separately confirmed private paths and generated binary history that require the approved rewrite.
- Demo: PASS — selected Remotion cut, 1920×1080 at 30 fps, 12.05 seconds; README GIF is 960×540 at 12 fps, 12.16 seconds, and 2.55 MiB. The opening product state visibly includes Codex, Claude, Cursor, and Antigravity, and the final hold leaves Promote and Abandon untouched.
- Installer: PASS — `The Staff Room_1.0.0-rc.1_x64-setup.exe`, 3.93 MiB, SHA-256 `040E0125B89FA9DB03AEC680C966CA39926561561E0DE65C89983A98A61BEB60`, Authenticode status `NotSigned` as documented.

## Manual rollout

1. Complete and record all machine-verifiable rows on the release branch.
2. Obtain explicit approval for the bounded history rewrite; back up refs, rewrite only identified generated paths, and rerun the secret and size audits.
3. Complete packaged and opt-in live-provider acceptance against synthetic repositories.
4. Change candidate versions from `1.0.0-rc.1` to `1.0.0`, rerun the full gate, and create the signed source tag.
5. Rename the GitHub repository to `staff-room`, update the local remote, and verify the old URL redirects.
6. Enable private vulnerability reporting, set the fixed description and topics, then change visibility to public.
7. Review the public landing page and clone it into a clean directory. Confirm README media, license detection, checks, and history scan.
8. Pin the repository in profile position 3, after `evalseal` and `tripwire`.

## Rollback

If the public clone, history scan, metadata, or README differs from the reviewed candidate, return the repository to private visibility, remove the profile pin, and stop distribution while the discrepancy is corrected. Do not overwrite the private backup ref created before history rewrite.

## Release record

Record exact command versions, exit status, advisory decisions, installer filename and hash, manual observation date, provider versions, rewritten commit ID, public commit ID, and final tag here or in an attached release artifact. Do not record credentials, private paths, or provider transcripts.
