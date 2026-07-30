use super::{assert_argv_prompt, is_read_only, PreparedCommand, ProviderAdapter, TurnRequest};

pub(super) struct Adapter;

impl ProviderAdapter for Adapter {
    fn kind(&self) -> &'static str {
        "antigravity"
    }

    fn build_command(&self, request: &TurnRequest<'_>) -> Result<PreparedCommand, String> {
        if request.phase == "chat" && request.mode != super::Mode::Probe {
            return Err(
                "Antigravity is available for Ship only because it has no true read-only mode."
                    .into(),
            );
        }
        assert_argv_prompt(request.prompt)?;
        let review = matches!(request.mode, super::Mode::Probe | super::Mode::Review);
        let mut args = vec![
            "--sandbox".into(),
            "--add-dir".into(),
            request.repository.display().to_string(),
            "--mode".into(),
            if is_read_only(request.mode) || review {
                "plan"
            } else {
                "accept-edits"
            }
            .into(),
        ];
        // The fixed probe remains in plan mode, but agy can perform a read before
        // answering READY. Headless mode cannot prompt for that read permission.
        if (!is_read_only(request.mode) && !review) || request.mode == super::Mode::Probe {
            args.push("--dangerously-skip-permissions".into());
        }
        if let Some(model) = request.model {
            args.extend(["--model".into(), model.into()]);
        }
        if let Some(effort) = request.effort {
            args.extend(["--effort".into(), effort.into()]);
        }
        if let Some(session_id) = request.session_id {
            args.extend(["--conversation".into(), session_id.into()]);
        }
        args.extend(["--print".into(), request.prompt.into()]);
        Ok(PreparedCommand {
            args,
            stdin: None,
            assigned_session_id: None,
        })
    }
}
