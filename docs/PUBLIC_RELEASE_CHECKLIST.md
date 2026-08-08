# Public v1.0 release checklist

**Target outcome:** publish `jonathanjasare/staff-room` as a source-first, human-gated Windows v1.0 whose safety claims are reproducible and traceable to Rust enforcement.

**Current candidate:** `1.0.0-rc.1`

**Non-goals:** new product features, autonomous unlock, macOS or Linux support, code signing, automatic promotion, or publication without the final human gate.

## P0 gates

| Gate | Required evidence | Status |
| --- | --- | --- |
| Full-history secrets and privacy scan | `gitleaks git --redact` has no unresolved findings; generated history containing local paths is removed or explicitly approved | PASS on rewritten private GitHub refs and direct clone; server-side history gate still blocks visibility |
| Repository identity | Publication repository is `jonathanjasare/staff-room`; a new-repository route keeps `rooms` private and clearly superseded, while a reuse route renames it and verifies the old-URL redirect | Human publication step |
| GitHub server-side history | Publish the clean graph to a new repository, or audit PR, fork, and Actions exposure and obtain GitHub Support confirmation that cached views and old refs are purged | Human publication step; a force-push alone is insufficient |
| Product rename | Case-insensitive search leaves only documented legacy migration identifiers and changelog history | PASS on RC branch |
| Data migration | Tests prove the old SQLite database and voice assets are copied, integrity-checked, and not deleted | PASS: 2 focused migration tests plus aggregate Rust gate |
| License | Root MIT license plus npm and Cargo metadata | PASS |
| Demo | Synthetic-path GIF is at most 60 seconds and 5 MB, ends at `awaiting-promotion`, and passes frame review | PASS: 12.16 seconds, 2.55 MiB, frame-reviewed |
| README and architecture | First screen carries name, fixed tagline, demo, and invariant; enforcement symbols map to code | PASS on RC branch |
| Automated verification | Clean install and `npm run check` pass on Windows | PASS on 2026-08-08 |
| Dependency audit | `npm audit` and `cargo audit` have no unresolved advisory | PASS for Windows RC; warning disposition recorded below |
| Repository-size audit | Largest 20 historical objects classified; generated outputs removed from publishable history | PASS on rewritten private GitHub refs; removed paths are unreachable from every retained ref |
| Packaged build | `npm run tauri build` produces an installer named for The Staff Room | PASS: unsigned NSIS installer produced |
| Human-gated acceptance | Every applicable observation in `V1_ACCEPTANCE.md` is recorded PASS and every unsupported provider-route cell is recorded N/A against the candidate | Human acceptance step |
| Autonomous honesty | `AUTONOMOUS_ACCEPTANCE_COMPLETE` is `false`; README links the NOT ACCEPTED contract | PASS on RC branch |
| Security reporting | GitHub private vulnerability reporting enabled | Human publication step |
| GitHub metadata | Fixed tagline description and topics `tauri`, `rust`, `ai-agents`, `developer-tools`, `local-first` | Human publication step |

## History and repository-size audit

The pre-publication scan found generated redesign driver JSON and QA screenshots in the remote Git history. Those artifacts contained private absolute paths and commit-shaped values that triggered the Sourcegraph-token detector. The current release tree had already deleted the artifacts, so a bounded history rewrite was required to make their old blobs unreachable.

The approved rewrite was prepared on 2026-08-08 from an exact mirror of every GitHub ref. A complete private backup bundle was verified before mutation; its SHA-256 is `AD7A9319E48C52C75EC95E81CEA1894A69C2DBB0572FB56B44E887DC817BECB4`. `git filter-repo` removed `docs/redesign/**` from history and replaced private workstation paths and run-specific values in retained text. Fresh review found additional private context after the first pass, so the final mirror was rebuilt from the untouched backup with stricter replacements.

The rewritten release commit differs from the pre-rewrite candidate only in `docs/CURSOR_PROVIDER_CONTEXT_RCA.md` and `docs/providers/antigravity.md`, where a private room label, three diagnostic run identifiers, and a cross-project codename were replaced with neutral public examples. A subsequent release-record commit updates this checklist only. No product code or README media changed.

Only `main` and `chore/staff-room-public-v1` are retained. The six removed remote branches contained no commit beyond `main`, and the remote had no tags. Full-ref scans confirm that removed redesign artifacts, attachment paths, private username and project-root variants, and identified run-specific values are absent from the rewritten graph. The remaining Windows home paths are explicitly synthetic examples (`example`, `<you>`, or `CodexSandboxOffline`). `git fsck --full --strict` passes and the all-history secrets scan, including archive depth 3, reports no findings. The private GitHub rewrite and a direct post-push clone are verified; the backup remains private until the final public repository and default branch are verified.

A clean clone proves the advertised Git graph, but it cannot prove that GitHub has purged old commit pages, pull-request refs, forks, or cached views. GitHub also states that [Actions history and logs become public with a visibility change](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/managing-repository-settings/setting-repository-visibility). Following [GitHub's sensitive-data removal guidance](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/removing-sensitive-data-from-a-repository), the recommended publication route is a new `staff-room` repository created from this clean graph while `rooms` remains private. Reusing the existing repository requires a PR, fork, Actions-run, log, and artifact audit plus GitHub Support confirmation of old-object cleanup before visibility changes.

## RC verification record — 2026-08-08

- `npm run check`: PASS — 24 frontend tests, production build, design lint across 81 files, 64 Rust tests, and 89 visual/accessibility checks.
- `npm audit`: PASS — 0 vulnerabilities.
- `cargo audit --file src-tauri/Cargo.lock`: PASS — 0 vulnerabilities. RustSec reported 18 allowed warnings: 16 unmaintained transitive crates and two unsound advisories that are absent from the supported Windows target dependency tree. Reassess before adding non-Windows support.
- `gitleaks dir --redact`: PASS — no findings in the candidate tree.
- `gitleaks git --redact --max-archive-depth 3`: PASS on the rewritten retained refs - no findings. Historical path and unreachable-object checks also pass.
- Private GitHub ref update: PASS - the atomic guarded push updated `main` and `chore/staff-room-public-v1`, deleted the six redundant branches, and left no advertised tags or pull-request refs. A direct GitHub clone at abbreviated pre-record release tip `b85cd7b9a7f8` passed the history, working-tree, path, and integrity scans.
- Demo: PASS — selected Remotion cut, 1920×1080 at 30 fps, 12.05 seconds; README GIF is 960×540 at 12 fps, 12.16 seconds, and 2.55 MiB. The opening product state visibly includes Codex, Claude, Cursor, and Antigravity, and the final hold leaves Promote and Abandon untouched.
- Installer: PASS — `The Staff Room_1.0.0-rc.1_x64-setup.exe`, 3.93 MiB, SHA-256 `040E0125B89FA9DB03AEC680C966CA39926561561E0DE65C89983A98A61BEB60`, Authenticode status `NotSigned` as documented.

## Manual rollout

1. Completed 2026-08-08: atomically update the two retained refs on the still-private `rooms` repository, delete the six redundant remote refs, and verify a direct clean clone and all-ref history scan.
2. Choose the publication target: preferably create a new `staff-room` repository from the clean graph and keep `rooms` private; otherwise complete the GitHub old-object cleanup gate above.
3. Review and merge `chore/staff-room-public-v1`, then verify that the publication target's `main` is the reviewed Staff Room candidate before any visibility change.
4. Complete packaged and opt-in live-provider acceptance against synthetic repositories.
5. Change candidate versions from `1.0.0-rc.1` to `1.0.0`, rerun the full gate, and create the signed source tag.
6. Enable private vulnerability reporting and set the fixed description and topics.
7. Change visibility to public only after every P0 publication gate is complete.
8. Review the public landing page and clone it into a clean directory. Confirm README media, license detection, checks, and history scan.
9. Pin the repository in profile position 3, after `evalseal` and `tripwire`.

## Rollback

If the public clone, history scan, metadata, or README differs from the reviewed candidate, return the repository to private visibility, remove the profile pin, and stop distribution while the discrepancy is corrected. Do not overwrite the private backup ref created before history rewrite.

## Release record

Record exact command versions, exit status, advisory decisions, installer filename and hash, manual observation date, provider versions, rewritten commit ID, public commit ID, and final tag here or in an attached release artifact. Do not record credentials, private paths, or provider transcripts.
