# Cursor and provider context RCA

Status: implemented and verified on `feature/cursor-context-and-git-fix`.

## Reported symptoms

The room showed three related provider failures:

1. Cursor displayed its internal instruction to read a Staff Room prompt file as if it were the answer.
2. Cursor returned `git rev-parse HEAD failed ... ambiguous argument 'HEAD'` while working in the `fixture` room.
3. Antigravity showed `Connection failed` even though model refresh had just reported that it fetched the signed-in CLI account.

## Evidence

### Cursor instruction rendered as output

The durable run artifact at
`C:/Users/example/AppData/Local/com.staffroom.desktop/runs/<run-id>`
contains the normal The Staff Room prompt in `chat.prompt.txt`. The first Cursor user event in
`chat.stdout.log` contains this as the provider input:

`Read the file at C:/Users/example/AppData/Local/com.staffroom.desktop/runs/<run-id>/chat.prompt.txt in full. It contains your complete assignment. Follow it exactly.`

That text is the intentional short argv prompt used to avoid Windows command-line length limits.
The Cursor event is a `type: "user"` event with `message.role: "user"`. The provider-specific
chat parser ignores it, but the generic `parse_result_text` fallback extracts every
`message.content[].text` value and replaces the visible chat snapshot with it.

### Git `HEAD` failure

The attached synthetic `C:/Users/example/Projects/unborn-fixture` repository has `.git/HEAD` pointing to
`refs/heads/master`, but it has no `refs/heads/master`, index, or commit object. It is an unborn
branch. Git therefore rejects `rev-parse HEAD` with the same ambiguous `HEAD` diagnostic visible
in the room. The repository can still be attached because the attachment check only requires a
Git worktree root, and the provider runner had no Cursor-specific preflight before starting the
CLI.

### Antigravity connection probe

The recent connection-test artifacts `<run-id-a>`,
`<run-id-b>`, and `<run-id-c>`
have an empty stdout log. Their stderr says:

`jetski: no output produced - a tool required the "read_file" permission that headless mode cannot prompt for, so it was auto-denied.`

The probe prompt is fixed and read-only, but the Antigravity adapter uses `--mode plan` without
`--dangerously-skip-permissions` for `Mode::Probe`. The CLI therefore cannot approve its own
required read before it can answer `READY`.

## Root causes

1. A generic parser fallback treated a provider's user-input event as assistant output.
2. Cursor was launched against an unborn Git repository even though Cursor requires an initial
   commit for its workspace Git context.
3. The Antigravity connection probe was configured as read-only but not as headless-safe. Its
   required read permission was denied before the readiness response.

## Implemented solution

- User-role and user-type provider events never replace the visible assistant response. Assistant
  fragments and terminal result events continue to render normally.
- Cursor checks for an initial commit before session allocation or provider spawn. An unborn
  repository receives an actionable message to create an initial commit, and The Staff Room does not
  create that commit on the user's behalf.
- Antigravity connection probes retain `--mode plan` and gain permission auto-approval only for
  the fixed probe, allowing the headless CLI to complete its read-only readiness check.
- Focused regression tests cover all three provider-boundary behaviors.

The existing prompt-file transport remains unchanged. The parser now rejects user events before
the generic message fallback, the provider runner checks Cursor repositories before session
allocation, and the Antigravity adapter adds the permission bypass only for `Mode::Probe`.

## Verification

- Rust tests pass: 61 tests.
- Frontend tests pass: 24 tests.
- The production frontend build, Rust formatting check, design lint, and `git diff --check` pass.

## Scope and limitation

This fix keeps the existing prompt-file transport because it is required for large Windows
prompts. It does not synthesize an initial Git commit, since doing so would mutate a user's
repository without an explicit request. A repository with no initial commit must be initialized
by the user before Cursor can provide its full workspace-aware response.
