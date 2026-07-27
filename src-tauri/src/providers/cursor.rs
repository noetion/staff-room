use super::{assert_argv_prompt, is_read_only, PreparedCommand, ProviderAdapter, TurnRequest};

pub(super) struct Adapter;

impl ProviderAdapter for Adapter {
    fn kind(&self) -> &'static str { "cursor" }

    fn build_command(&self, request: &TurnRequest<'_>) -> Result<PreparedCommand, String> {
        assert_argv_prompt(request.prompt)?;
        let mut args = vec![
            "--print".into(), "--output-format".into(), "stream-json".into(),
            "--stream-partial-output".into(), "--trust".into(),
            "--workspace".into(), request.repository.display().to_string(),
        ];
        #[cfg(not(windows))]
        args.extend(["--sandbox".into(), "enabled".into()]);
        if is_read_only(request.mode) { args.extend(["--mode".into(), "ask".into()]); } else { args.push("--force".into()); }
        if let Some(model) = request.model {
            let model = match request.effort {
                Some(effort) => cursor_model_with_effort(model, effort),
                None => model.to_owned(),
            };
            args.extend(["--model".into(), model]);
        }
        if let Some(session_id) = request.session_id { args.extend(["--resume".into(), session_id.into()]); }
        args.push(request.prompt.into());
        Ok(PreparedCommand { args, stdin: None, assigned_session_id: None })
    }
}

pub(super) fn cursor_model_with_effort(model: &str, effort: &str) -> String {
    if let Some(open) = model.find('[') {
        if model.ends_with(']') {
            let base = &model[..open];
            let mut parameters = model[open + 1..model.len() - 1].split(',').map(str::trim).filter(|parameter| !parameter.is_empty() && !parameter.starts_with("effort=")).map(str::to_owned).collect::<Vec<_>>();
            parameters.push(format!("effort={effort}"));
            return format!("{base}[{}]", parameters.join(","));
        }
    }
    format!("{model}[effort={effort}]")
}
