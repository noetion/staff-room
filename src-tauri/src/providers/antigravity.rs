use super::{assert_argv_prompt, is_read_only, PreparedCommand, ProviderAdapter, TurnRequest};

pub(super) struct Adapter;

impl ProviderAdapter for Adapter {
    fn kind(&self) -> &'static str { "antigravity" }

    fn build_command(&self, request: &TurnRequest<'_>) -> Result<PreparedCommand, String> {
        if request.phase == "chat" {
            return Err("Antigravity is available for Ship only because it has no true read-only mode.".into());
        }
        assert_argv_prompt(request.prompt)?;
        let mut args = vec!["--sandbox".into(), "--add-dir".into(), request.repository.display().to_string(), "--mode".into(), if is_read_only(request.mode) { "plan" } else { "accept-edits" }.into()];
        if !is_read_only(request.mode) { args.push("--dangerously-skip-permissions".into()); }
        if let Some(model) = request.model { args.extend(["--model".into(), model.into()]); }
        if let Some(effort) = request.effort { args.extend(["--effort".into(), effort.into()]); }
        if let Some(session_id) = request.session_id { args.extend(["--conversation".into(), session_id.into()]); }
        args.extend(["--print".into(), request.prompt.into()]);
        Ok(PreparedCommand { args, stdin: None, assigned_session_id: None })
    }
}
