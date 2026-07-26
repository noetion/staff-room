use super::{is_read_only, PreparedCommand, ProviderAdapter, TurnRequest};
use uuid::Uuid;

pub(super) struct Adapter;

impl ProviderAdapter for Adapter {
    fn kind(&self) -> &'static str { "claude" }

    fn build_command(&self, request: &TurnRequest<'_>) -> Result<PreparedCommand, String> {
        let mut args = vec!["--print".into(), "--exclude-dynamic-system-prompt-sections".into(), "--permission-mode".into(), if is_read_only(request.mode) { "plan" } else { "auto" }.into()];
        if is_read_only(request.mode) {
            args.extend(["--tools".into(), "Read,Grep,Glob".into()]);
        }
        if request.structured_output {
            args.extend(["--verbose".into(), "--output-format".into(), "stream-json".into()]);
            if request.phase == "chat" { args.push("--include-partial-messages".into()); }
        }
        if let Some(model) = request.model { args.extend(["--model".into(), model.into()]); }
        if let Some(effort) = request.effort { args.extend(["--effort".into(), effort.into()]); }
        if let Some(contract) = request.handoff_contract { args.extend(["--append-system-prompt".into(), contract.into()]); }
        let assigned_session_id = if let Some(session_id) = request.session_id {
            args.extend(["--resume".into(), session_id.into()]);
            Some(session_id.to_owned())
        } else {
            let session_id = Uuid::new_v4().to_string();
            args.extend(["--session-id".into(), session_id.clone()]);
            Some(session_id)
        };
        Ok(PreparedCommand { args, stdin: Some(request.prompt.into()), assigned_session_id })
    }
}
