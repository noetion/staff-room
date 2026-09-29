# Build and release verification

These instructions reproduce the source verification gate and unsigned Windows installer. Run them from a fresh clone on Windows.

Current development is experimental, pre-1.0 (`0.1.0`). There is no supported stable binary. A successful build produces a local evaluation artifact, not release acceptance. Use throwaway repositories; the preserved `v1.0.0` records do not qualify this version.

## Prerequisites

- Windows 10 or 11 with WebView2
- Git
- Node.js 22 and npm
- Rust stable with the MSVC target
- Visual Studio Build Tools with **Desktop development with C++**
- PowerShell

Provider CLIs and whisper.cpp are not required for automated verification. They are required only for the corresponding opt-in manual acceptance rows.

## Clean installation

```powershell
npm ci
npx playwright install chromium
```

`npm ci` uses the committed lockfile and fails if package metadata and the lockfile disagree.

## Automated gate

```powershell
npm run check
```

Equivalent individual commands:

```powershell
npm test
npm run build
node scripts/check-design.mjs
cargo test --manifest-path src-tauri/Cargo.toml
npm run qa:visual:acceptance
```

The visual acceptance command starts a local frontend preview and writes generated evidence under `docs/redesign/qa/`. Those files are intentionally ignored.

## Dependency and history audits

Install `cargo-audit` once, then run both package audits and the full-history secret scan:

```powershell
cargo install cargo-audit --locked
npm audit
cargo audit --file src-tauri/Cargo.lock
gitleaks git --redact
```

Release audit results and any justified exception belong in [PUBLIC_RELEASE_CHECKLIST.md](PUBLIC_RELEASE_CHECKLIST.md). A clean working tree is not evidence that history is safe to publish.

## Unsigned NSIS installer

```powershell
npm run bundle:windows
```

The release script remaps builder-local Rust source paths, strips native symbols, and fails if the compiled executable still contains the repository, user-profile, Cargo-home, or Rustup-home path. The installer is produced below `src-tauri/target/release/bundle/nsis/`, using the current Tauri package version in its filename. The current bundle target is NSIS only; the script name and release build profile do not imply a stable public release.

The installer is unsigned. Windows SmartScreen may display an unknown-publisher warning, and users must inspect the source and decide whether to continue. The project does not instruct users to disable SmartScreen or weaken system policy.

## Packaged acceptance

Automated checks cannot prove microphone permission behavior, installed CLI compatibility, or the final human promotion flow. Use a synthetic fixture repository and record every observation in [V1_ACCEPTANCE_SESSION.md](V1_ACCEPTANCE_SESSION.md). Use only synthetic code and data for release capture.

Before publishing a release, complete [current provider boundary acceptance](PROVIDER_BOUNDARY_ACCEPTANCE.md) and rerun every applicable publication gate from [the historical release checklist](PUBLIC_RELEASE_CHECKLIST.md). Record fresh evidence against the new candidate; do not reuse historical PASS verdicts. The separate human acceptance matrix must also pass before creating a stable tag.
