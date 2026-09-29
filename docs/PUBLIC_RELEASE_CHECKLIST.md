# Public release checklist

**Historical record, not current acceptance.** Current development is experimental and pre-1.0 (`0.1.0`), with no supported stable binary. The earlier `v1.0.0` designation overstated maturity. The original observations, artifact identities, and publication history below are retained for traceability; their PASS verdicts do not carry forward. See [current provider boundary gates](PROVIDER_BOUNDARY_ACCEPTANCE.md).

**Historical target outcome:** publish `jonathanjasare/staff-room` as a source-first human-gated Windows v1.0 whose safety claims are reproducible and traceable to Rust enforcement.

**Historical release:** `1.0.0`

**Non-goals:** new product features, autonomous unlock, macOS or Linux support, code signing, or automatic promotion.

## Publication gates

| Gate | Required evidence | Status |
| --- | --- | --- |
| Full-history secrets and privacy scan | `gitleaks git --redact` has no unresolved findings; generated history containing local paths is removed or explicitly approved | PASS on the clean public `staff-room` history and a fresh anonymous `v1.0.0` clone |
| Repository identity | Publication repository is `jonathanjasare/staff-room`; the original `rooms` repository remains private | PASS: publication repository created from the clean graph and now public; `rooms` remains private |
| GitHub server-side history | Publish the clean graph to a new repository so old PR, fork, Actions, and cached-object state from `rooms` cannot become public | PASS: only the clean graph was pushed to `staff-room` |
| Product rename | Case-insensitive search leaves only documented legacy migration identifiers and changelog history | PASS on the stable release |
| Data migration | Tests prove the old SQLite database and voice assets are copied, integrity-checked, and not deleted | PASS: 2 focused migration tests plus aggregate Rust gate |
| License | Root MIT license plus npm and Cargo metadata | PASS |
| Demo | Synthetic-path GIF is at most 60 seconds and 5 MB, ends at `awaiting-promotion`, and passes frame review | PASS: 15 seconds, 4.96 MB, 1200×675, frame-reviewed; linked MP4 is 1920×1080 |
| README and architecture | First screen carries name, fixed tagline, demo, and invariant; enforcement symbols map to code | PASS on the stable release |
| Automated verification | Clean install and `npm run check` pass on Windows | PASS on the 2026-08-10 stable candidate |
| Dependency audit | `npm audit` and `cargo audit` have no unresolved advisory | PASS for Windows v1.0; warning disposition recorded below |
| Repository-size audit | Largest 20 historical objects classified; generated outputs removed from publishable history | PASS on rewritten private GitHub refs; removed paths are unreachable from every retained ref |
| Packaged build | `npm run bundle:windows` produces an installer named for The Staff Room and rejects builder-local paths in the compiled executable | PASS: clean candidate produced the unsigned installer and its compiled executable passed the path scan |
| Autonomous honesty | `AUTONOMOUS_ACCEPTANCE_COMPLETE` is `false`; README links the NOT ACCEPTED contract | PASS on the stable release |
| Security reporting | GitHub private vulnerability reporting enabled | PASS: enabled and verified after the visibility transition on 2026-08-09 |
| GitHub metadata | Fixed tagline description and topics `tauri`, `rust`, `ai-agents`, `developer-tools`, `local-first` | PASS on the public repository |

At the RC publication point, the public repository advertised only `main` at commit `c0516cd` and had no tags. The stable release branch and tag are handled by the rollout steps below.

## `1.0.0` release gates

| Gate | Required evidence | Status |
| --- | --- | --- |
| Human-gated acceptance | Every applicable observation in `V1_ACCEPTANCE.md` is recorded PASS and every unsupported provider-route cell is recorded N/A against the candidate | PASS on 2026-08-10 |
| Stable version | npm, Cargo, Tauri, changelog, installer, and source tag all identify the accepted `1.0.0` candidate | PASS: annotated tag `v1.0.0`, release assets, and commit `0f7306b338e1` identify the accepted `1.0.0` release |

The complete human-gated matrix passed and the stable release was published on 2026-08-10. Autonomous Ship remains a separate fail-closed tier and does not inherit the human-gated verdict.

## History and repository-size audit

The pre-publication scan found generated redesign driver JSON and QA screenshots in the remote Git history. Those artifacts contained private absolute paths and commit-shaped values that triggered the Sourcegraph-token detector. The current release tree had already deleted the artifacts, so a bounded history rewrite was required to make their old blobs unreachable.

The approved rewrite was prepared on 2026-08-08 from an exact mirror of every GitHub ref. A complete private backup bundle was verified before mutation; its SHA-256 is `AD7A9319E48C52C75EC95E81CEA1894A69C2DBB0572FB56B44E887DC817BECB4`. `git filter-repo` removed `docs/redesign/**` from history and replaced private workstation paths and run-specific values in retained text. Fresh review found additional private context after the first pass, so the final mirror was rebuilt from the untouched backup with stricter replacements.

The rewritten release commit differs from the pre-rewrite candidate only in `docs/CURSOR_PROVIDER_CONTEXT_RCA.md` and `docs/providers/antigravity.md`, where a private room label, three diagnostic run identifiers, and a cross-project codename were replaced with neutral public examples. A subsequent release-record commit updates this checklist only. No product code or README media changed.

The rewritten source repository retained only `main` and `chore/staff-room-public-v1`; the six removed branches contained no commit beyond `main`, and the remote had no tags. The new publication repository was initialized from release commit `28cad326bcd5`, contains only `main`, and has no tags. Full-ref scans confirm that removed redesign artifacts, attachment paths, private username and project-root variants, and identified run-specific values are absent from the rewritten graph. The remaining Windows home paths are explicitly synthetic examples (`example`, `<you>`, or `CodexSandboxOffline`). `git fsck --full --strict` passes and the all-history secrets scan, including archive depth 3, reports no findings. A direct clone of the new private publication repository resolved to release tree `293844b64624` and passed the working-tree, history, and integrity checks. The original `rooms` repository and the backup remain private.

A clean clone proves the advertised Git graph, but it cannot prove that GitHub has purged old commit pages, pull-request refs, forks, or cached views in the original repository. GitHub also states that [Actions history and logs become public with a visibility change](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/managing-repository-settings/setting-repository-visibility). Following [GitHub's sensitive-data removal guidance](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/removing-sensitive-data-from-a-repository), `staff-room` was created as a new repository from the clean graph and `rooms` remains private. The original repository must not be used as the public target.

## Candidate verification record - 2026-08-09

The exact candidate was assembled from base commit `ae03781981c4` plus the intended documentation and README GIF changes, without copying untracked files or maintainer build outputs.

- Clean `npm ci`: PASS; 107 packages installed and 0 vulnerabilities reported.
- Clean `npm run check`: PASS; 24 frontend tests, production build, design lint across 81 files, 72 Rust tests, and 89 visual and accessibility checks.
- `npm audit`: PASS; 0 vulnerabilities.
- `cargo audit --file src-tauri/Cargo.lock`: PASS; 0 blocking advisories and the same 18 allowed transitive warnings recorded for the Windows candidate.
- Candidate-only `gitleaks dir --redact`: PASS; 12.45 MB of tracked candidate content scanned with no findings.
- Base-history `gitleaks git --redact --max-archive-depth 3`: PASS; all 59 retained pre-candidate commits scanned with no findings.
- Hosted GitHub surfaces: PASS with the package-API limitation noted below. The private repository has six CI runs, zero Actions artifacts, releases, deployments, issues, or pull requests, and no Pages site. The current `main` and review-branch runs pass. All 768.62 KB of Actions logs, including the earlier failed run, passed secret and targeted private-path scans.
- Packages: the authenticated API token lacks `read:packages`, so the API check is UNVERIFIED. The directly inspected repository page reports no published packages, and the sole workflow has no package-publishing step. Recheck the public repository page immediately after the visibility transition.
- Demo: PASS; README GIF is 1200×675 at 5 fps, 15 seconds, and 4,958,825 bytes. Its SHA-256 is `77B377FB30B3170629B901BD0AA5CD09044D13F7C59532490FA69EEAC74B03DD`. The linked MP4 remains 1920×1080 at 30 fps.
- Installer: PASS; `The Staff Room_1.0.0-rc.1_x64-setup.exe`, 4,138,069 bytes, SHA-256 `917C03E2FA102DED7BF6940121188C26D4B84E40A3870B5E99469F99E2F57E69`, Authenticode status `NotSigned` as documented.

## Stable verification record - 2026-08-10

- Full gate: PASS; `npm run check` completed 28 frontend tests, the production build, design lint across 82 files, 75 Rust tests, and 89 visual and accessibility checks.
- Strict Rust lint: PASS; `cargo clippy --all-targets --manifest-path src-tauri/Cargo.toml -- -D warnings` completed without findings.
- Dependency audit: PASS; `npm audit` reported 0 vulnerabilities. `cargo audit --file src-tauri/Cargo.lock` reported 0 blocking advisories and the same 18 allowed transitive warnings: 16 unmaintained crates and two advisories outside the supported Windows target dependency tree.
- Candidate privacy scan: PASS; Gitleaks scanned 223 publishable working-tree files, 12.48 MB, with no findings. Ignored Rust build output was excluded from the publishable-tree scan.
- Human-gated acceptance: PASS; Ask, Quick Edit, the four paired Build/Review routes, all four connection probes, provider Build cancellation, Verify cancellation, Antigravity Review cancellation, app-close recovery, the two-attempt recovery limit, and promotion interruption were observed on synthetic repositories.
- Staged stable-package revalidation: PASS. The process-custody build, a 4,143,689-byte installer with SHA-256 `62B8CDA33ED967851AFCFC6223BCB12BD71ABDA03EEDCD9DD9123AA29BC3D2F9`, passed fresh packaged probes for all four providers through the suspended, Job-owned launch path; Cursor also exercised Job-owned session preallocation, and no new agent process remained. After the preview catalogue, explicit-provider-routing, and recovery-message corrections, the full gate passed again. The final package installed successfully, passed a Codex probe, and visibly rejected an explicit unavailable `@cursor` Ship request without starting a run or falling back to ready Codex. The provider-launch implementation was unchanged between those two builds.
- Provider versions: Codex `0.144.4`, Claude Code `2.1.221`, Cursor Agent `2026.08.04-aaa8809`, and Antigravity `1.1.11`.
- Packaged acceptance installer: PASS; `The Staff Room_1.0.0_x64-setup.exe`, 4,143,693 bytes, SHA-256 `F3DC6D656301377ABBF6BFF66611C2FF0DF0583FEE848E091382154297909538`, Authenticode status `NotSigned` as documented. Silent installation succeeded, the installed product reported `1.0.0`, and the packaged routing check passed.
- Publication installer: PASS; the exact fresh-clone candidate ran `npm run bundle:windows` from empty build outputs and produced `The Staff Room_1.0.0_x64-setup.exe`, 4,143,791 bytes, SHA-256 `74B01B0C2102A3AC8CEDB9E22A53B97F9DD9793A0DEE417362D8E89D84FCD333`, ProductVersion `1.0.0`, and Authenticode status `NotSigned`. The release builder remapped local Rust source roots, stripped native symbols, and found no repository, user-profile, Cargo-home, or Rustup-home path in the compiled executable. Silent installation returned success; the installed payload reported `1.0.0`, contained none of the audited private markers, and left the three personal SQLite custody files byte-identical. Tauri's NSIS bundle-type resource patch changes the installed executable hash from the pre-bundle binary, so the installer checksum is the distribution identity. Runtime source is unchanged from the packaged acceptance installer; the later source-only change records this evidence.
- Local custody: PASS; the three pre-acceptance SQLite files were restored byte-for-byte to their recorded SHA-256 values after the synthetic run matrix and final packaged probes.

## Stable publication record - 2026-08-10

- Source identity: PASS; `main`, `release/v1.0.0`, and annotated tag `v1.0.0` resolved to commit [`0f7306b338e1`](https://github.com/jonathanjasare/staff-room/commit/0f7306b338e12051f2fd96ec039e3dce62ac212e) at publication.
- Hosted verification: PASS; the [release-branch](https://github.com/jonathanjasare/staff-room/actions/runs/31398342021), [main](https://github.com/jonathanjasare/staff-room/actions/runs/31398944525), and [tag](https://github.com/jonathanjasare/staff-room/actions/runs/31400093178) CI runs completed successfully on the same commit.
- GitHub Release: PASS; [The Staff Room 1.0.0](https://github.com/jonathanjasare/staff-room/releases/tag/v1.0.0) was published as the latest non-prerelease release with exactly two assets.
- Public installer: PASS; `The.Staff.Room_1.0.0_x64-setup.exe` is 4,143,791 bytes with GitHub digest and independently downloaded SHA-256 `74B01B0C2102A3AC8CEDB9E22A53B97F9DD9793A0DEE417362D8E89D84FCD333`, ProductVersion `1.0.0`, and Authenticode status `NotSigned`.
- Checksum companion: PASS; `SHA256SUMS.txt` names the normalized public installer filename and contains the same SHA-256.
- Anonymous outsider verification: PASS; a credential-free clone of `v1.0.0` resolved to the exact release commit. Clean `npm ci` reported 0 vulnerabilities, and `npm run check` passed 28 frontend tests, the production build, design lint across 82 files, 75 Rust tests, and 89 visual and accessibility checks.
- Public surfaces: PASS; unauthenticated API checks identified `v1.0.0` as the latest release and found zero Actions artifacts and zero deployments. The repository was already pinned third on the maintainer profile, with Tripwire and EvalSeal ahead of it.
- Durable summary: [the concise `1.0.0` release record](releases/1.0.0.md) binds the public claims, boundaries, assets, and verification evidence.

## Public verification record - 2026-08-09

- Visibility and security: PASS; the repository is public, vulnerability alerts and automatic security fixes remain enabled, and private vulnerability reporting is enabled.
- Anonymous clone: PASS; a credential-free clone resolved to `c0516cd` on `main`, with no other branches or tags advertised.
- Public setup and verification: PASS; clean `npm ci` reported 0 vulnerabilities and `npm run check` passed 24 frontend tests, the production build, design lint, 72 Rust tests, and 89 visual and accessibility checks.
- Public privacy and integrity: PASS; the 60-commit public history and a tracked-tree archive passed gitleaks, and `git fsck --full --strict` passed.
- Public landing page: PASS; the repository API and page return public status, MIT license detection, the expected description, and README references to both demo files. No public Staff Room package was found on the owner's public Packages page.
- Hosted CI: PASS; both push-triggered CI runs for `c0516cd` completed successfully. Dependabot separately reported that its automatic `glib` update cannot resolve the fixed Linux-only transitive version within the current Tauri dependency range; the Windows candidate's documented Cargo audit disposition is unchanged.

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
4. Completed 2026-08-09: publish the repository as the explicitly labelled `1.0.0-rc.1` source candidate without creating a stable tag or claiming accepted v1.0 status.
5. Completed 2026-08-09: enable and verify private vulnerability reporting while retaining the fixed description, required topics, vulnerability alerts, and automatic security fixes.
6. Completed 2026-08-09: review the public landing page and run the documented setup, full verification gate, integrity check, and privacy scans from a fresh anonymous clone.
7. Completed 2026-08-10: packaged and opt-in live-provider acceptance passed against synthetic repositories.
8. Completed 2026-08-10: changed candidate versions from `1.0.0-rc.1` to `1.0.0`, reran the full gate, and obtained final release approval.
9. Completed 2026-08-10: added the fail-closed release builder, rebuilt the publication installer with remapped local paths, and verified the exact version, signing status, checksum, and compiled-binary path scan.
10. Completed 2026-08-10: pushed the accepted commit through successful release-branch, `main`, and annotated-tag CI gates, then published the GitHub Release with the installer and checksum companion.
11. Completed 2026-08-10: downloaded both public assets without authentication, verified their bytes and metadata, and passed the full gate from a fresh credential-free clone of `v1.0.0`.
12. Completed 2026-08-10: verified that the repository was pinned in profile position 3, with Tripwire and EvalSeal ahead of it.

## Rollback

If the public clone, history scan, metadata, or README differs from the reviewed candidate, return the repository to private visibility, remove the profile pin, and stop distribution while the discrepancy is corrected. Do not overwrite the private backup ref created before history rewrite.

## Release record

The exact command results, advisory decisions, installer identity, manual observation date, provider versions, public commit, final tag, and outsider verification are recorded above and in [the concise `1.0.0` release record](releases/1.0.0.md). Credentials, private paths, and provider transcripts are intentionally excluded.
