# Public v1.0 release checklist

**Target outcome:** publish `jonathanjasare/staff-room` as a source-first, human-gated Windows v1.0 whose safety claims are reproducible and traceable to Rust enforcement.

**Current candidate:** `1.0.0-rc.1`

**Non-goals:** new product features, autonomous unlock, macOS or Linux support, code signing, automatic promotion, or publication without the final human gate.

## P0 gates

| Gate | Required evidence | Status |
| --- | --- | --- |
| Full-history secrets and privacy scan | `gitleaks git --redact` has no unresolved findings; generated history containing local paths is removed or explicitly approved | PASS on the new private `staff-room` repository and a fresh direct clone |
| Repository identity | Publication repository is `jonathanjasare/staff-room`; the original `rooms` repository remains private | PASS: new private repository created and verified |
| GitHub server-side history | Publish the clean graph to a new repository so old PR, fork, Actions, and cached-object state from `rooms` cannot become public | PASS: only the clean graph was pushed to `staff-room` |
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
| GitHub metadata | Fixed tagline description and topics `tauri`, `rust`, `ai-agents`, `developer-tools`, `local-first` | PASS on the private publication repository |

## History and repository-size audit

The pre-publication scan found generated redesign driver JSON and QA screenshots in the remote Git history. Those artifacts contained private absolute paths and commit-shaped values that triggered the Sourcegraph-token detector. The current release tree had already deleted the artifacts, so a bounded history rewrite was required to make their old blobs unreachable.

The approved rewrite was prepared on 2026-08-08 from an exact mirror of every GitHub ref. A complete private backup bundle was verified before mutation; its SHA-256 is `AD7A9319E48C52C75EC95E81CEA1894A69C2DBB0572FB56B44E887DC817BECB4`. `git filter-repo` removed `docs/redesign/**` from history and replaced private workstation paths and run-specific values in retained text. Fresh review found additional private context after the first pass, so the final mirror was rebuilt from the untouched backup with stricter replacements.

The rewritten release commit differs from the pre-rewrite candidate only in `docs/CURSOR_PROVIDER_CONTEXT_RCA.md` and `docs/providers/antigravity.md`, where a private room label, three diagnostic run identifiers, and a cross-project codename were replaced with neutral public examples. A subsequent release-record commit updates this checklist only. No product code or README media changed.

The rewritten source repository retained only `main` and `chore/staff-room-public-v1`; the six removed branches contained no commit beyond `main`, and the remote had no tags. The new publication repository contains only `main` at `28cad326bcd531dce8c7863a9031f4ae9e9d40b3` and has no tags. Full-ref scans confirm that removed redesign artifacts, attachment paths, private username and project-root variants, and identified run-specific values are absent from the rewritten graph. The remaining Windows home paths are explicitly synthetic examples (`example`, `<you>`, or `CodexSandboxOffline`). `git fsck --full --strict` passes and the all-history secrets scan, including archive depth 3, reports no findings. A direct clone of the new private publication repository resolves to tree `293844b646244f3b7facd6600e0bfc722ca74c5d` and passes the working-tree, history, and integrity checks. The original `rooms` repository and the backup remain private.

A clean clone proves the advertised Git graph, but it cannot prove that GitHub has purged old commit pages, pull-request refs, forks, or cached views in the original repository. GitHub also states that [Actions history and logs become public with a visibility change](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/managing-repository-settings/setting-repository-visibility). Following [GitHub's sensitive-data removal guidance](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/removing-sensitive-data-from-a-repository), `staff-room` was created as a new repository from the clean graph and `rooms` remains private. The original repository must not be used as the public target.

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
2. Completed 2026-08-08: create the private `jonathanjasare/staff-room` publication repository from the clean graph and keep `rooms` private.
3. Completed 2026-08-08: verify that the publication target advertises only `main`, has no tags, and resolves to the reviewed Staff Room candidate.
4. Complete packaged and opt-in live-provider acceptance against synthetic repositories.
5. Change candidate versions from `1.0.0-rc.1` to `1.0.0`, rerun the full gate, and create the signed source tag.
6. At the visibility transition, enable and verify private vulnerability reporting. The fixed description, required topics, vulnerability alerts, and automatic security-fix pull requests are already configured.
7. Change visibility to public only after every P0 publication gate is complete.
8. Review the public landing page and clone it into a clean directory. Confirm README media, license detection, checks, and history scan.
9. Pin the repository in profile position 3, after `evalseal` and `tripwire`.

## Rollback

If the public clone, history scan, metadata, or README differs from the reviewed candidate, return the repository to private visibility, remove the profile pin, and stop distribution while the discrepancy is corrected. Do not overwrite the private backup ref created before history rewrite.

## Release record

Record exact command versions, exit status, advisory decisions, installer filename and hash, manual observation date, provider versions, rewritten commit ID, public commit ID, and final tag here or in an attached release artifact. Do not record credentials, private paths, or provider transcripts.
