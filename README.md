# Agent Room

Agent Room is a local-first desktop room that carries one engineering objective, repository evidence, agent results, reviews, and durable project context between coding-agent CLIs.

The current vertical slice provides:

- a Tauri 2 desktop shell;
- truthful local probes for Codex, Claude, Cursor Agent, and Antigravity;
- direct, argument-array Codex execution in the attached repository;
- streamed provider events and cancellable runs;
- Git evidence captured after a run;
- SQLite-backed projects, messages, runs, and provider sessions;
- a responsive room UI with explicit mentions and bounded run state;
- a read-only browser preview that never fabricates agent execution.

## Run

```powershell
npm install
npm run tauri dev
```

If Cargo is not on `PATH` on Windows:

```powershell
$env:Path = "C:\Users\<you>\.cargo\bin;$env:Path"
npm run tauri dev
```

Frontend-only visual preview:

```powershell
npm run dev
```

## Verify

```powershell
npm test
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
```

See [docs/PLAN_EVALUATION.md](docs/PLAN_EVALUATION.md) for the product and architecture review.
