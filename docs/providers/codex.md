# Codex provider

## Local proof

- Executable: `<user-home>\AppData\Local\Programs\OpenAI\Codex\bin\codex.exe`
- Version tested: `codex-cli 0.144.4`
- Status: installed
- Live-help proof: exec, never-ask approvals, workspace sandbox, JSONL, final-output capture, and resume

## Working v1 contract

- Non-interactive turn: `codex exec`
- Repository scope: global `--cd <managed-worktree>`
- Structured events: `--json`
- Final response: `--output-last-message <durable-run-artifact-file>`
- Final handoff: Agent Room schema-v1 JSON record between explicit result markers
- Session resume: `codex exec resume <session-id> -`
- Build/revision policy: `--ask-for-approval never --sandbox workspace-write`
- Review policy: `--ask-for-approval never --sandbox read-only`
- Cancellation: terminate only the child process owned by the run

The `never` approval policy returns denied escalation failures to the model instead of asking the user. The filesystem sandbox remains active. Agent Room does not use `--dangerously-bypass-approvals-and-sandbox`.

## Capability result

`isolated-auto`: complete unattended repository work inside the managed worktree without routine approval prompts.
