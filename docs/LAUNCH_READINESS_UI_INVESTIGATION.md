# Launch-readiness UI investigation

Status: investigation recorded before implementation, with implementation results captured below, on `codex/launch-readiness-ui-fixes`.

## Baseline

- The worktree was clean on `main` before the branch was created.
- `npm test`: 5 files, 21 tests passed.
- `cargo test --manifest-path src-tauri/Cargo.toml`: 56 tests passed.
- `npm run qa:visual:acceptance`: 84 checks passed. The harness did not cover the inspector tab or the stale Ship-card state, so those remain targeted acceptance cases.
- The reported UI failures are not covered by the current unit suites, so the acceptance checks below include rendered and interaction-level verification.

## Issue inventory and proposed acceptance

### 1. Unwanted element in the chat window

Observed cause: `src/App.tsx` renders `RunProgressCard` inside `Conversation` for every run state except `ready`. The latest persisted Ship run is loaded independently of the chat flow, so a completed or abandoned historical run can appear below ordinary chat messages. The supplied screenshot shows an abandoned Ship card presented as if it were current chat content.

Solution implemented: `shouldShowRunProgress` now allows only active, promotion, or recoverable stopped/failed/waiting states. Completed and abandoned historical runs remain available through room evidence and the context inspector without occupying the chat timeline. A frontend regression test covers the state gate.

Acceptance: an abandoned or completed latest run does not mount a `Current Ship run` card in the room; an active or recoverable run still does.

### 2. Excessive horizontal whitespace, especially beside chat

Observed cause: the main chat column, composer, progress card, and message groups all cap themselves at 760px while the room fills the remaining window. At wide desktop sizes this leaves large unused gutters on both sides of the conversation.

Solution implemented: introduced one shared `--content-max` measure for the room, conversation column, composer, progress card, activity content, and room header. Message bubbles retain a separate readable inner measure.

Acceptance: wide layouts use the available room width without horizontal overflow, while message text and controls remain readable and compact at narrow widths.

### 3. Clicking Evidence crashes the application

Observed cause: Rust serializes optional receipt fields as JSON `null`, while the renderer treats them as either a number or `undefined`. Evidence rendering calls `toLocaleString()` through `millisecondsLabel` and `toFixed()` for cost when those fields are `null`, so a normal receipt with no provider-reported timing or cost can throw during tab mount.

Solution implemented: receipt formatting and provider labels now tolerate nullable legacy values, including missing timing and cost. The browser acceptance harness opens Room context, selects Evidence, and asserts that the inspector remains mounted without a page error.

Acceptance: opening Room context and selecting Evidence keeps the app mounted and shows meaningful empty-state values when no evidence or receipts exist.

### 4. Search bar should be larger and more prominent

Observed cause: the title bar gives the search control an intrinsic `max-content` grid column and a compact minimum width. The control therefore competes visually with the window controls instead of acting as the main navigation/search affordance.

Solution implemented: the title bar now reserves a deliberate responsive search column, and the button/input fills that column while preserving the existing shortcut and filtering behavior.

Acceptance: the search affordance is visibly wider at desktop sizes, remains usable at tablet/mobile widths, and Ctrl/Cmd+K still focuses it.

### 5. Settings page has excessive whitespace

Observed cause: `.settings-view` is capped at 1080px inside a much wider scrollable room, and the provider grid is forced to two columns below 1400px. With four providers, the resulting second row leaves a large unused region beside the final card.

Solution implemented: widened the settings content measure to 1440px and let the provider grid use a stable 320px card minimum, collapsing only when the viewport requires it. The wide desktop acceptance check verifies that all four provider cards share one row.

Acceptance: settings content uses the available desktop width, provider cards form the appropriate number of columns, and small screens still collapse cleanly.

### 6. Settings layout should use horizontal space better

Observed cause: provider cards stack each route as a full-width label/input pair, and the outer grid breakpoint is based on viewport width rather than the actual content measure. The layout leaves horizontal capacity unused while making the page unnecessarily tall.

Solution implemented: provider cards now use the available horizontal measure, and each route presents Model and Effort side by side on desktop while collapsing to one column on small screens. No new settings architecture was introduced.

Acceptance: all installed/uninstalled provider cards remain legible, route controls align consistently, and no card or form control overflows at the supported widths.

### 7. Scrolling beyond the end of the chat area

Observed cause: the app shell and room use a height-constrained grid, but the room does not explicitly clip its grid area. The chat and composer are allowed to participate in the page's outer scroll when their combined intrinsic size exceeds the grid track, so the window can scroll beyond the intended room boundary.

Solution implemented: the room now clips its grid area, the conversation owns vertical history scrolling, and overscroll chaining is contained at the conversation boundary. The acceptance harness checks both the document boundary and feed scrollability.

Acceptance: the document remains at `scrollTop === 0` after room load and wheel scrolling; only the chat feed scrolls, and the feed cannot scroll beyond `scrollHeight - clientHeight`.

### 8. Refresh Models must retrieve the latest available models

Observed cause: `discover_provider_models` runs `models` for Cursor and Antigravity, but routes Codex and Claude through `model_options`, which returns hard-coded IDs. The UI then treats that result as an authoritative catalogue and clears saved models absent from it. That can both show stale choices and erase a valid newly released model. The installed Codex CLI's app-server supports an account-backed `model/list` response; the installed Claude Code CLI exposes model aliases and full-name selection but no non-interactive catalogue command.

Solution implemented: Codex model refresh now initializes the installed app-server and reads its account-backed `model/list` response, while Cursor and Antigravity continue using their provider-native `models` commands. Claude now returns a truthful unsupported-catalog error and does not invalidate saved exact IDs. The Codex parser skips hidden entries, normalizes duplicates deterministically, and has Rust coverage.

Acceptance: a successful refresh is sourced from the provider's current account-backed command; unsupported catalog providers do not claim a current catalogue or delete saved exact IDs; provider output is normalized deterministically. Claude's lack of a non-interactive catalog remains an explicit limitation until its CLI exposes one.

## Remaining limitations

- Claude Code still has no non-interactive model catalogue in the installed CLI. Refresh reports that limitation, while users can enter a supported alias or exact identifier and saved choices remain intact.
- Account-backed refresh for Codex, Cursor, and Antigravity still depends on the provider CLI being installed, signed in, and permitted to return its catalogue. Provider errors are surfaced instead of being replaced with stale success data.
- The full `npm run qa:visual` performance pass remains above its 32ms frame budget in headless Chromium, measuring 104–148ms during the scroll probe. The launch-readiness interaction/layout checks pass, but this broader performance issue needs dedicated profiling rather than a speculative UI rewrite.
- The visual report retains 12 non-failing `axe-incomplete` contrast-analysis warnings from the existing theme/contrast review. They were not part of the reported defects and did not become test failures.

## Verification after implementation

- `npm test`: 5 files, 23 tests passed.
- `npm run build`: passed.
- `cargo test --manifest-path src-tauri/Cargo.toml`: 57 tests passed.
- `npm run qa:visual:acceptance`: 89 checks passed after the settings measure was corrected. The suite covers overflow, accessibility, focus, contrast, Evidence interaction, room boundary, wide content, prominent search, and wide settings layout.
- A live sequential Codex app-server probe returned the current signed-in model list, including `gpt-5.6-sol`, `gpt-5.6-terra`, `gpt-5.6-luna`, `gpt-5.5`, `gpt-5.4`, `gpt-5.4-mini`, and `gpt-5.3-codex-spark`.

## Scope guard

The implementation will stay within the existing React/Tauri boundaries. It will not redesign the navigation, change provider command contracts beyond model discovery, or add speculative model APIs that the installed CLIs do not expose.
