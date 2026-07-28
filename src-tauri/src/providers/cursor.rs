use super::{assert_argv_prompt, is_read_only, PreparedCommand, ProviderAdapter, TurnRequest};

pub(super) struct Adapter;

impl ProviderAdapter for Adapter {
    fn kind(&self) -> &'static str {
        "cursor"
    }

    fn build_command(&self, request: &TurnRequest<'_>) -> Result<PreparedCommand, String> {
        assert_argv_prompt(request.prompt)?;
        let mut args = vec![
            "--print".into(),
            "--output-format".into(),
            "stream-json".into(),
            "--stream-partial-output".into(),
            "--workspace".into(),
            request.repository.display().to_string(),
        ];
        #[cfg(not(windows))]
        args.extend(["--sandbox".into(), "enabled".into()]);
        if is_read_only(request.mode) {
            args.extend(["--mode".into(), "ask".into()]);
        } else {
            args.push("--force".into());
        }
        if let Some(model) = request.model {
            args.extend(["--model".into(), model.to_owned()]);
        }
        if let Some(session_id) = request.session_id {
            args.extend(["--resume".into(), session_id.into()]);
        }
        args.push(request.prompt.into());
        Ok(PreparedCommand {
            args,
            stdin: None,
            assigned_session_id: None,
        })
    }
}
