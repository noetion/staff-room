use super::{is_read_only, PreparedCommand, ProviderAdapter, TurnRequest};

pub(super) struct Adapter;

impl ProviderAdapter for Adapter {
    fn kind(&self) -> &'static str { "codex" }

    fn build_command(&self, request: &TurnRequest<'_>) -> Result<PreparedCommand, String> {
        let mut args = vec![
            "-a".into(), "never".into(),
            "-c".into(), "approval_policy=\"never\"".into(),
            "-c".into(), "shell_environment_policy.inherit=all".into(),
            "-s".into(), if is_read_only(request.mode) { "read-only" } else { "workspace-write" }.into(),
            "-C".into(), request.repository.display().to_string(),
        ];
        if let Some(model) = request.model { args.extend(["--model".into(), model.into()]); }
        if let Some(effort) = request.effort {
            args.extend(["-c".into(), format!("model_reasoning_effort=\"{effort}\"")]);
        }
        args.push("exec".into());
        if let Some(session_id) = request.session_id {
            args.extend(["resume".into(), "--json".into(), "-o".into(), request.final_output_path.display().to_string(), session_id.into(), "-".into()]);
        } else {
            args.extend(["--json".into(), "-o".into(), request.final_output_path.display().to_string(), "-".into()]);
        }
        Ok(PreparedCommand { args, stdin: Some(request.prompt.into()), assigned_session_id: None })
    }
}
