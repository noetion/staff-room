# Agent Room — audit triage and route to v1

**Reviewed:** `FULL_PROJECT_AUDIT_20260728.md` against the working tree at `<repo>`
**Method:** every P0 and most P1 claims re-checked directly in source. Labels below are mine, not the audit's.

---

## 1. Verdict on the audit

**It is unusually accurate.** I could not find a fabricated finding. Every P0 I checked reproduces in the code, and several were confirmed at the exact line the audit cited. Two claims are overstated in severity, one is wrong in scope, and the entire market/pricing section is unverified opinion. Details below.

The audit's *diagnosis* is trustworthy. Its *release plan* is not the plan I'd follow — see section 4.

---

## 2. Claim-by-claim verification

### Confirmed exactly as written

| ID | Claim | What I found |
|---|---|---|
| **UI-01** | `primitives.css` never loaded | `grep -rn "primitives.css" src/` returns **nothing**. `src/design/index.css` imports only `tokens/glass/base`. `primitives/index.ts` exports components but no stylesheet. 13.5 KB of button/chip/segmented/sheet/monogram/forced-colors CSS is dead. **True.** |
| **UI-02** | Human message text 1:1 contrast | `.message-bubble[data-side="out"] { background: var(--action) }`; `.markdown-body p { color: var(--ink) }`. Light theme: `--action: var(--ink)` → both `#0C0C0E`. Dark: `--action: #F4F2F7`, `--ink: #F4F2F7`. **Exactly 1:1 in both themes. True, and the worst bug in the repo** — the user's own messages are invisible. |
| **SEC-01** | Renderer IDs become path components | `StartRunRequest.run_id`, `ChatRequest.run_id`, `QuickEditRequest.edit_id` are all `String` from IPC. `run_artifact_directory` does `.join("runs").join(run_id)` with zero validation. **True.** |
| **SEC-02** | Project ID and repo path independently trusted | `chat.rs`: `PathBuf::from(&request.repository_path)`, validated only by `git rev-parse --show-toplevel`. Same in `edit.rs`, `run/mod.rs`, `providers.rs`. Nothing ties the path to the persisted `project_id`. **True.** |
| **SEC-03** | `csp: null` | Confirmed verbatim in `tauri.conf.json` `app.security`. **True.** |
| **SEC-05** | Cursor Ship `--force` unsandboxed on Windows | `#[cfg(not(windows))] args.extend(["--sandbox", "enabled"])` — Windows gets no sandbox, then `--force`. **True.** *And worse than reported:* `--trust` is passed unconditionally on every platform. |
| **RUN-01** | Stop kills only direct child | Only `child.kill()` and `kill_on_drop(true)`. No Job Object, no process group, no `CREATE_NEW_PROCESS_GROUP`. **True.** |
| **RUN-02 (a)** | No cancellation guard before promotion | `update_run(..., "promoting")` → `git rev-parse HEAD` → `promote_worktree(...)`. No cancellation re-check, no promotion lock. **True.** |
| **PROV-02** | Antigravity can never connect | `test_provider_connection` hardcodes `Phase::Chat, ProviderMode::Ask`; `antigravity.rs` returns `Err` for `phase == "chat"`. Ship only admits providers persisted as connected. **True — Antigravity is dead on arrival.** |
| **SEC-04** | Verification runs repo text through a shell | `cmd /C <command_text>` on Windows, `sh -lc` elsewhere, with `apply_provider_environment` and inherited env. Codex adapter sets `shell_environment_policy.inherit=all`. Git calls inject `-c safe.directory=<repo>` unconditionally. **True.** |
| **OPS-01** | No CI | No `.github` directory. `package.json` has `dev/build/test/tauri/qa:visual` — no aggregate gate. **True.** |
| **QA-01** | Acceptance record says FAIL | `docs/V1_ACCEPTANCE.md` line 5 opens with **"FAIL — release acceptance is not complete."** Rows 1–8+ all FAIL. **True.** |
| **UI-04** | Project switching crosses async boundaries | `activateProject` awaits five loads then fires nine `setState` calls. No abort, no generation token, no project key. **True.** |
| Typography | `font: var(--t-body)` is invalid | `--t-body: 14px`. The CSS `font` shorthand requires a family — `font: 14px` is **discarded entirely**. This pattern is used ~20× in `inspector.css` alone. **True, and I'd rate it higher than the audit did.** |

### Overstated

| ID | Audit says | Reality |
|---|---|---|
| **UI-03** | Apply/Discard "trapped in a closed disclosure", unreachable | `<Disclosure className="quick-edit-preview">` with no `summary` and no `defaultOpen` → a closed `<details>`. But Chromium renders a **default UA disclosure triangle** for `<details>` without `<summary>`, and it toggles. So the buttons are *reachable* — just unlabeled, collapsed, and undiscoverable. Real defect, bad UX, **not a P0 dead end.** |
| **DATA-01 (migration)** | Discarded `ALTER TABLE` errors = P0 | `let _ = connection.execute(stmt, [])` over 38 `ADD COLUMN` statements. Swallowing errors here is the standard idiom because re-runs legitimately fail with "duplicate column name". It *does* mask disk/corruption failures, but it is not a release blocker on its own. **P1.** The WAL-unsafe backup in the same finding **is** serious — `std::fs::copy` of the main file only, with `PRAGMA journal_mode = WAL` set. That can silently lose committed data. Keep that at P0. |

### Wrong in scope

| ID | Audit says | Reality |
|---|---|---|
| **RUN-02 (b)** | "Both review phases launch providers in write-capable Ship mode" → all four providers can mutate | `ProviderMode::Ship` is passed for `review`/`final-review` — confirmed. But `antigravity.rs` independently checks `matches!(request.phase, "review" \| "final-review")` and forces `--mode plan`. So Antigravity's reviewer *is* read-only. Codex (`workspace-write`), Claude (`--permission-mode auto`), and Cursor (`--force`) are exposed. **3 of 4, not 4 of 4.** Conclusion unchanged; the fix is the same. |

### Unverified (can't confirm from here)

- The competitor table, the "voice is table stakes" argument, and OpenAI pricing. All plausible, all time-sensitive, none checkable from the repo. Treat as opinion.
- The `npm outdated` table (TypeScript 7.0.2, Vite 8.1.5, lucide-react 1.27.0). Your `package.json` pins `typescript ~5.7.2`, `vite ^6.0.5`, `lucide-react ^0.468.0` — the gaps are real in direction, the exact target versions I did not confirm.
- All "NOT RUN" rows (packaged E2E, live providers, signing, microphone). Correctly labelled as missing evidence.

### One thing the audit missed

`--trust` is passed to Cursor on **every** platform, not just Windows. Combined with `--force` and the missing Windows sandbox, that's the single most dangerous invocation in the codebase.

---

## 3. The real problem with the audit

It scopes v1 as **"autonomous cross-provider Ship on your real repositories."** Under that definition its 14-PR, 8-gate plan is right, and you are months out.

But nothing forces that definition. Most of the P0 list exists *only* because an agent runs unattended with write authority and then promotes without a human looking. Remove the "unattended" and "auto-promote" properties and roughly half the blockers become P2 hardening instead of release gates.

That's the decision in front of you, and it's a product decision, not an engineering one.

---

## 4. The route to v1

**Scope decided: safe personal-use v1.** Fix correctness and the interface, gate the unsafe autonomy paths behind explicit warnings, ship to yourself and a handful of testers. No CI-signed installer, no host sandbox, no packaged E2E harness in this cycle — those are named and deferred at the end of this section, not forgotten.

### v1 is **assisted, not autonomous**

> Agent Room runs one agent to build a change in an isolated worktree, has a second agent review it, runs your checks, and shows you the delta. **You** press Promote.

What that buys you immediately:

- **RUN-02's promotion race disappears** — promotion is already gated on a human click.
- **SEC-05 becomes a labelling fix** — Cursor unattended Ship is out of scope for v1; keep it behind a flag.
- **SEC-01/SEC-02 drop from "host safety" to "correctness"** — still fix them, but they're not blocking distribution to yourself and a handful of testers.
- **PROV-01's capability handshake becomes P2** — you're not claiming unattended safety, so you don't need to prove it.
- **Gate F shrinks** — no packaged signed installer needed for a private beta you run from `cargo tauri dev`.

Ship autonomous Ship as v2, after the benchmark in the audit's Gate H tells you it's worth the latency and token cost.

### The v1 work, in order

**Week 1 — make it usable.** This is the highest value-per-hour work in the entire audit and it's about a day of actual coding.

1. Import `primitives.css` from `src/design/index.css`. Fix the sheet/inspector z-index collision it exposes.
2. `.markdown-body, .markdown-body p { color: inherit }`. Give code/evidence blocks explicit colors since they set their own backgrounds.
3. Replace every `font: var(--t-*)` with `font-size: var(--t-*)`. Add a `check-design.mjs` rule that rejects `font:` shorthand with a single custom property.
4. Move `.inspector-sheet`/`.inspector-scrim` to a higher-specificity selector or reorder imports so `.glass { position: relative }` stops clobbering `position: fixed`.
5. Give Quick Edit a real open review panel with a labelled summary, Apply, and Discard — drop the `<Disclosure>`.
6. Add a contrast assertion to `qa:visual`: for every visible text node, compute foreground against the *painted* ancestor background. The existing Axe pass missed a 1:1 failure; that's the gap it needs to close.

**Week 2 — honest state.** Cheap, and it stops v1 from lying.

7. Hide Antigravity behind "not yet supported" until PROV-02 is fixed (or add `Phase::Connection` + `ProviderMode::Probe` and give it a `--mode plan` probe — it's ~30 lines).
8. Require exact normalized `READY` in `connection_test_ready`; today `NOT READY` can pass a substring match.
9. Remove `--trust` from the Cursor adapter unless the repo is explicitly marked trusted. Disable Cursor Ship on Windows or put a distinct high-risk confirmation in front of it.
10. Rewrite `README.md` to match `V1_ACCEPTANCE.md`. One provider matrix, generated from evidence, stating "tested with version X on date Y".

**Week 3–4 — the boundaries that actually matter for a human-gated v1.**

11. `ProviderMode::Review` mapped to read-only per provider (Codex `read-only`, Claude `plan` + `--tools Read,Grep,Glob`, Cursor `--mode ask`). Antigravity already does this — copy its phase check.
12. Rust-generated `OperationId(Uuid)`; one `ManagedOperationPath::new(root, id)` helper that parses a UUID, requires a single `Component::Normal`, canonicalizes, and verifies containment. Remove `run_id`/`edit_id` from the IPC request structs.
13. `ProjectContext` loaded from SQLite by `project_id` only. Delete `repository_path` from `ChatRequest`, `QuickEditRequest`, `StartRunRequest`, `ConnectionTestRequest`.
14. WAL-safe backup: `VACUUM INTO` instead of `std::fs::copy`. Add `PRAGMA user_version` and `busy_timeout`. Wrap migrations in one transaction per version.
15. `RequestContext { projectId, operationId, kind }` on every async mutation in `App.tsx`; discard any result whose project doesn't match current. Simplest v1 shortcut: block project switching while Chat or Quick Edit is in flight.

**Week 5 — enough proof for personal use.**

16. Four fake provider executables (shell scripts are fine) covering malformed JSONL, oversized lines, auth failure, timeout, descendant process holding stdout, attempted review mutation, `READY`/`NOT READY`. Run the real coordinator against them. This is the one test investment worth making now — it's what lets you change the run lifecycle without fear, and it needs no paid tokens.
17. A single `npm run check` script chaining `npm test`, `npm run build`, `check-design`, `cargo fmt --check`, `cargo test --locked`, `cargo clippy -D warnings`, fake-provider suite. Run it by hand. GitHub Actions can wrap the same script later at near-zero cost.
18. Rerun `V1_ACCEPTANCE.md`. Rows deliberately out of scope for a personal-use v1 get struck out with a reason, not left as silent FAILs. A record that says "12 pass, 17 out of scope because v1 is human-gated" is honest; 29 FAILs is not.

### Explicitly deferred, with the reason

| Deferred | Why it's safe to defer for personal use | What re-opens it |
|---|---|---|
| Windows Job Objects (RUN-01) | You're watching the run; an orphaned `npm test` is an annoyance, not a hazard | Turning autonomy back on |
| Strict CSP (SEC-03) | No remote content is loaded and there's no raw HTML sink today | Any remote content, or voice (it's a Gate V0 prerequisite) |
| Repository trust screen (SEC-04) | You only attach your own repos | Anyone else attaching a repo they didn't write |
| Versioned capability handshake (PROV-01) | v1 claims no unattended safety, so there's nothing to prove | Advertising provider support publicly |
| Host sandbox, packaged E2E, signing, updater | No distribution in this cycle | Distribution |
| Agent voice output — TTS, per-agent voices, realtime (Gate G V2) | Nothing in the product needs to speak for v1 to be useful | After voice input proves itself (§8) |

*Voice **input** is no longer on this list — see §8. Transcribing locally removes its dependency on CSP and Rust-side secrets, which is what let it move to Week 6.*

Write these into `README.md` as known limitations. A deferred risk that's documented is a scope decision; an undocumented one is a bug.

### Why Job Objects are deferred

Process-tree ownership is genuinely broken, and in an unattended product it's a P0. In a human-gated v1 where you're watching the run, an orphaned `npm test` is an annoyance. It's also fiddly Windows FFI work that will eat a week. Do it before you turn autonomy back on — not before you can see your own messages.

### On voice

The audit's analysis is sound: chained STT → existing composer → existing route is the right first architecture, and no adapter needs to change. But it is correctly sequenced last. Every hour spent on voice before the interface renders correctly is an hour spent on a product nobody can use. The Gate G prerequisites (CSP, Rust-side secrets) are real and none of them exist yet.

---

## 5. The critical path, if the weeks slip

1. Import `primitives.css`.
2. `color: inherit` on `.markdown-body`.
3. Fix `font:` shorthand.
4. Open the Quick Edit review panel.
5. Hide Antigravity; drop Cursor `--trust`; disable unattended Windows Ship.
6. Make Promote a human click and call that v1.

That's a few days of work and it takes you from "the acceptance record says FAIL" to "a thing you can hand to five people."

---

## 6. Definition of done for this v1

Not the audit's 11-point beta bar — that's v2. This v1 is done when:

1. You can read your own messages in both themes, and a computed-contrast test asserts it.
2. The primitive design system is loaded and the Inspector sits where it's meant to.
3. Quick Edit can be reviewed, applied, and discarded from a visible, labelled surface.
4. Every provider shown in the UI can actually reach `connected`, or is hidden.
5. No provider runs a review phase with write authority.
6. No agent writes to your checkout without you clicking Promote.
7. Operation IDs and repository paths come from Rust, not the renderer.
8. A database backup taken by the app can actually be restored.
9. The fake-provider suite passes and `npm run check` is one command.
10. `README.md`, `V1_ACCEPTANCE.md`, and the UI tell the same story about what's supported.

Ten items, mostly small. That's a shippable personal tool.

---

## 7. What the 5-week plan leaves on the table (Gates A–H)

The audit's roadmap is eight gates. The plan above is not a way of doing all eight faster — it's a decision to complete two of them, most of a third, and consciously stop. Here's the honest coverage map.

| Gate | Covered by the 5-week plan | Left |
|---|---|---|
| **A — freeze unsafe autonomy** | **Complete.** Cursor Windows Ship gated (9), Antigravity hidden (7), capability claims corrected (10), pre-release status preserved (18), worktree-is-not-a-sandbox documented | — |
| **B — authority boundaries** | Rust-generated typed operation IDs + managed-path helper (12), Rust-loaded `ProjectContext` (13) | **Strict CSP** · **explicit repository trust** · **coordinator Git hooks disabled** · **minimal environment policy** · run/project-bound recovery in Rust (item 15 only fixes the renderer half) |
| **C — atomic execution & promotion** | Read-only Review mode (11). The cancellation-before-promotion race is *dissolved* by the human click, not fixed | **Job Objects / process groups** · typed operation *registry* (12 gives IDs, not the registry) · **full content fingerprint** · **approved/sandboxed verification plan** · **per-project promotion lock + state transaction** · idempotent Quick Edit / abandon cleanup |
| **D — versioned contracts** | Exact `READY` matching (8), Antigravity probe (7), transactional migrations + WAL-safe backup (14) | **Real version/help/handshake capability cache** · **bounded parser & file inputs** (the fake-provider suite in 16 *detects* the oversized-line allocation; it doesn't fix it) · phase-level persistence transactions and indexes |
| **E — restore the interface** | Primitive styles + overlay ownership (1, 4), outgoing contrast (2), Quick Edit actions (5), typography (3), project-keyed async state (15) | Event-listener / run-ID races · composer semantics and live regions · native boot & error state · scroll theft · catalogue auto-mutation |
| **F — executable acceptance** | Fake CLIs + coordinator integration suite (16), one `npm run check` (17), acceptance record reconciled *as scoped* (18) | **Renderer interaction suite** · **packaged Windows Tauri E2E** · **CI** · **live versioned provider smoke** · **signed draft installer + migration/update rehearsal** · the 29-row record actually *passing* rather than being scoped out |
| **G — voice** | Nothing in weeks 1–5. **Week 6 (§8) covers packaged microphone feasibility, editable push-to-talk transcript, and privacy/retention/latency controls** — and voids the Rust-only STT key/network item by never creating a key or a network path | Final-response TTS · per-agent voice mapping · budget controls (all moot until a paid backend exists) |
| **H — product proof** | **Nothing.** | 30-task benchmark · 10 user interviews · custody receipt · positioning rewrite · realtime/platform decision |

Roughly: **A done, E mostly done, B and D about half, C about a quarter, F about a third, G's input half done in Week 6, H untouched.**

Against the audit's 14-PR sequence, the plan delivers PRs 1, 2, 3, 9, 10, most of 6, 7, 8, and part of 11. **PRs 4, 5, 12, 13 and 14 are entirely outstanding.**

### The four items I'd watch most closely in what's left

These are the deferrals that are easiest to forget and most expensive to discover late:

1. **Minimal environment policy.** `codex.rs:17` sets `shell_environment_policy.inherit=all`, and `apply_provider_environment` passes the inherited env to both provider and verification processes. Every secret in your shell environment reaches every agent. Cheap to fix, easy to never think about again.
2. **Coordinator Git hooks.** Nothing sets `core.hooksPath` to an empty path — `git/context.rs` only injects `safe.directory`. A repository's own `.git/hooks` can execute during coordinator Git calls. This is Gate B's quietest item and it's a real code-execution path.
3. **Per-project promotion lock.** There is no promotion mutex anywhere in `src-tauri` — the only `Mutex` instances are the SQLite handle. A human click serialises promotion *in practice* for a single user, which is why it's deferred; it is not a guarantee.
4. **Bounded parser inputs.** A single oversized JSONL line can allocate before the aggregate cap applies. Human gating doesn't help here — a runaway provider still does it.

None of these block a personal-use v1. All four are Gate B/C/D items that need to be closed before anyone else runs the app on a repository they didn't write.

---

## 8. Voice input — Week 6 (v1.1)

**Scope: dictation into the composer. Nothing else.** Agent speech (TTS), per-agent voice mapping, and realtime conversation are Gate G V2 and stay deferred.

Starting point: there is currently **zero voice code in the repository.** A search across `src/` and `src-tauri/src/` for microphone, `getUserMedia`, speech, STT, TTS, transcription, or voice returns **no matches**, and `Cargo.toml` has no keyring or credential crate. This is a greenfield addition, which is good news — it means the design is still free.

### The decision that unblocks it: transcribe locally

The audit sequences voice last because it assumed **cloud STT**, which drags in two prerequisites you don't have: a Rust-side secret store for the API key, and strict CSP to contain the new network path. Those are the reason Gate G sits behind Gate B.

**Run the model locally and both prerequisites disappear.** No API key means no secret store. No network egress means voice adds *zero* CSP surface, so deferring CSP stays safe. It's also the choice consistent with the rest of the product — the entire custody argument is undermined if your spoken description of a private repository is shipped to a third party by default.

| | Local (recommended) | Cloud STT |
|---|---|---|
| Secret storage | **Not needed** | Required — blocks on Gate B |
| CSP prerequisite | **None** | Required — blocks on Gate B |
| Marginal cost | Zero | Per-minute, forever |
| Offline | Yes | No |
| Accuracy on jargon | Good, model-dependent | Better |
| Binary size | +60–190 MB model | Negligible |

Accuracy is the one real trade. It's recoverable: put transcription behind a trait and cloud becomes an opt-in setting later, not a rewrite.

### Capture in Rust, not in the webview

The riskiest unknown in Gate G is "packaged microphone feasibility" — whether `getUserMedia` behaves inside WebView2 in a packaged Windows build. **Don't find out.** Capture audio in Rust with `cpal` (WASAPI) and the webview never touches the microphone: no `getUserMedia`, no `media-src` CSP directive, no WebView2 permission prompt to fight, and audio bytes never enter the renderer at all. The renderer sends `start`/`stop` and receives a level meter and a final string.

This also means audio never crosses the IPC boundary in either direction, and never touches disk. The strongest privacy disclosure is the one you can implement in a sentence.

**Data flow:**

```
Composer (hold to talk)
  → invoke("start_dictation")     → Rust: cpal stream, 16 kHz mono f32, ring buffer
  ← emit("dictation-level", rms)  → level meter, ~20 Hz
  → invoke("stop_dictation")      → Rust: whisper-rs on the buffer, buffer zeroed
  ← returns { text }              → onObjectiveChange(value + text)
```

The transcript lands in `Composer.tsx`'s existing controlled `value` via the existing `onObjectiveChange`. **No provider adapter changes, no new route, no change to the run lifecycle.** That's the audit's own architecture recommendation and it's right — it keeps voice entirely outside the custody-critical code.

### The work

**W6.1 — Rust capture.** `cpal` input stream, default device, resample to 16 kHz mono, fixed-capacity ring buffer with a hard 120-second cap (bounded input, per Gate D's lesson). Emit RMS on a `dictation-level` event. Zero the buffer on stop, on error, and on drop.

**W6.2 — Transcription behind a trait.**

```rust
trait SpeechToText {
    fn transcribe(&self, pcm: &[f32], sample_rate: u32) -> Result<String, SttError>;
}
```

Primary implementation: `whisper-rs` with a bundled `base.en` quantized model (~60 MB) in app data. `FakeStt` returning canned text for the test suite. A cloud implementation can be added behind this trait later without touching anything else.

*Build risk, named honestly:* `whisper-rs` uses bindgen and needs cmake plus a C++ toolchain on Windows. You have MSVC already for Tauri, but this can still fight you. **Timebox it to half a day.** Fallback: bundle the `whisper.cpp` CLI as a Tauri sidecar and reuse the process-spawning machinery you already have — it's less elegant and inherits RUN-01's process-tree gap, but the process is short-lived and you'd get it working in an afternoon.

**W6.3 — Composer UI.** Mic button with three states: idle, listening (level meter), transcribing (spinner). `Ctrl+Shift+Space` hold-to-talk while the window is focused — **no global shortcut**, so the mic can never open when you're in another app. Offer a click-to-toggle mode in settings too; hold-to-talk alone is poor motor accessibility. Transcript **inserts at the caret** rather than replacing, so dictation and typing mix freely. `aria-live` announcement on state change.

**W6.4 — Windows permission reality.** Windows 10/11 gate microphone access for desktop apps under Settings → Privacy → Microphone, and `cpal` will simply fail if it's off. Detect that specific failure and render an actionable error with a link to `ms-settings:privacy-microphone` — not a generic "recording failed". Also handle: no input device, device removed mid-capture, device busy.

**W6.5 — Tests and acceptance.** `FakeStt` unit tests for insert-at-caret, cancel-mid-capture, buffer cap, and permission-denied error surface. Add four rows to `V1_ACCEPTANCE.md`: microphone capture in a *packaged* build, hold-to-talk round trip, permission-denied error state, and measured transcription latency for a 10-second utterance. Latency should be roughly 1–2 seconds on a modern desktop CPU, but **measure it, don't assume it** — the whole point of the acceptance record is that claims come from evidence.

### Hard rules

1. **Voice never sends.** It fills the composer. You read it, you press send. This is the same principle as the human-gated Promote, and it's non-negotiable — a misheard sentence must never start a run.
2. **Voice never confirms.** No dictation on Promote, Apply, Discard, Abandon, or any destructive confirmation. Voice produces text, never authority.
3. **No audio persisted.** Not to disk, not to the database, not to logs. The buffer is zeroed after transcription.
4. **No network.** If a cloud backend is ever added, it ships disabled, with explicit opt-in and a visible indicator while active.

### Why Week 6 and not Week 1

Two reasons, one of them concrete: **your composer text currently renders at 1:1 contrast against its own background.** Dictated text would land in the composer literally invisible. Week 1 item 2 is a hard prerequisite for even *testing* this.

The second is that ~5 days of voice work is ~5 days not spent on the ten things in section 6 that stand between you and a usable tool. Voice makes a working product nicer; it does not make a broken one work.

### Effort

About **5 days**, assuming `whisper-rs` builds. It does not consume any deferred Gate B item, does not touch the run lifecycle, and can slip a week without affecting anything else in the plan — which is exactly the property you want in a feature added on top of a release scope.

---

## 9. One strategic note

I can't verify the audit's competitive claims from the repo, so treat section 16 of it as opinion. But the underlying point holds regardless of whether Emdash and Conductor look exactly as described: **the differentiated thing you've built is the Git preservation and guarded promotion work** — the dirty-checkout snapshot handling, the clean-worktree branch/HEAD checks, the recovery reconciliation, the `.git`-marker validation before recursive delete. That is careful, hard-won code and it is the best thing in the repository.

The liquid-glass redesign, the rooms metaphor, and voice are all copyable in a sprint. When you come back to this after v1 ships, that's the asset to build the product story around.
