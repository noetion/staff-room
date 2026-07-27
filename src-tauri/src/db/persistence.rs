use crate::*;

#[allow(clippy::too_many_arguments)]
pub(crate) fn persist_message(
    database: &Database,
    project_id: &str,
    run_id: &str,
    sender: &str,
    kind: &str,
    body: &str,
    changed_files: &[String],
    verification: &[VerificationResult],
    reason: Option<&str>,
) -> Result<(), String> {
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    connection
        .execute(
            "INSERT INTO messages
             (id, project_id, run_id, sender_kind, message_kind, body, changed_files_json, verification_json, reason)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                Uuid::new_v4().to_string(),
                project_id,
                run_id,
                sender,
                kind,
                body,
                serde_json::to_string(changed_files).unwrap_or_else(|_| "[]".to_owned()),
                serde_json::to_string(verification).unwrap_or_else(|_| "[]".to_owned()),
                reason
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn update_run(
    database: &Database,
    run_id: &str,
    state: &str,
    current_owner: Option<&str>,
    review_count: u32,
    revision_count: u32,
    session_id: Option<&str>,
    context_bytes: usize,
    stop_reason: Option<&str>,
    finished: bool,
) -> Result<(), String> {
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    let transaction = connection
        .unchecked_transaction()
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "UPDATE runs SET
               state = ?1,
               current_owner = ?2,
               review_count = ?3,
               revision_count = ?4,
               native_session_id = COALESCE(?5, native_session_id),
               context_bytes = ?6,
               stop_reason = ?7,
               finished_at = CASE WHEN ?8 THEN CURRENT_TIMESTAMP ELSE finished_at END
             WHERE id = ?9",
            params![
                state,
                current_owner,
                review_count,
                revision_count,
                session_id,
                context_bytes as i64,
                stop_reason,
                finished,
                run_id
            ],
        )
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "INSERT INTO run_events
             (id, run_id, event_type, phase, state, current_owner, detail, context_bytes)
             VALUES (?1, ?2, 'transition', ?3, ?4, ?5, ?6, ?7)",
            params![
                Uuid::new_v4().to_string(),
                run_id,
                phase_for_run_state(state),
                state,
                current_owner,
                stop_reason.unwrap_or_default(),
                context_bytes as i64,
            ],
        )
        .map_err(|error| error.to_string())?;
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) fn phase_for_run_state(state: &str) -> &'static str {
    match state {
        "selecting" => "prepare",
        "working" => "build",
        "verifying" => "verify",
        "reviewing" => "review",
        "revising" => "revise",
        "promoting" => "promote",
        "complete" => "complete",
        _ => "ship",
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn persist_activation(
    database: &Database,
    run_id: &str,
    phase: Phase,
    participant: &str,
    state: &str,
    session_id: Option<&str>,
    context_bytes: usize,
    summary: Option<&str>,
) -> Result<(), String> {
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    connection
        .execute(
            "INSERT INTO activations
             (id, run_id, phase, participant_kind, state, session_id, context_bytes, summary, finished_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, CASE WHEN ?5 = 'running' THEN NULL ELSE CURRENT_TIMESTAMP END)",
            params![
                Uuid::new_v4().to_string(),
                run_id,
                phase.as_str(),
                participant,
                state,
                session_id,
                context_bytes as i64,
                summary
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) fn save_provider_session(
    database: &Database,
    project_id: &str,
    participant: &str,
    session_id: Option<&str>,
) -> Result<(), String> {
    let Some(session_id) = session_id else {
        return Ok(());
    };
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    connection
        .execute(
            "INSERT INTO provider_sessions (project_id, participant_kind, provider_session_id)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(project_id, participant_kind) DO UPDATE SET
               provider_session_id = excluded.provider_session_id,
               last_used_at = CURRENT_TIMESTAMP",
            params![project_id, participant, session_id],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) fn save_chat_session(
    database: &Database,
    project_id: &str,
    participant: &str,
    session_id: Option<&str>,
    last_seen_message_rowid: i64,
) -> Result<(), String> {
    let Some(session_id) = session_id else {
        return Ok(());
    };
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    connection
        .execute(
            "INSERT INTO chat_sessions (project_id, participant_kind, provider_session_id, last_seen_message_rowid)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(project_id, participant_kind) DO UPDATE SET
               provider_session_id = excluded.provider_session_id,
               last_seen_message_rowid = excluded.last_seen_message_rowid,
               last_used_at = CURRENT_TIMESTAMP",
            params![project_id, participant, session_id, last_seen_message_rowid],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) fn load_chat_session(
    database: &Database,
    project_id: &str,
    participant: &str,
) -> Result<Option<ChatSession>, String> {
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    connection
        .query_row(
            "SELECT provider_session_id, last_seen_message_rowid FROM chat_sessions
             WHERE project_id = ?1 AND participant_kind = ?2",
            params![project_id, participant],
            |row| {
                Ok(ChatSession {
                    provider_session_id: row.get(0)?,
                    last_seen_message_rowid: row.get(1)?,
                })
            },
        )
        .optional()
        .map_err(|error| error.to_string())
}

pub(crate) fn latest_room_message_marker(
    database: &Database,
    project_id: &str,
) -> Result<i64, String> {
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    connection
        .query_row(
            "SELECT COALESCE(MAX(rowid), 0) FROM messages WHERE project_id = ?1",
            [project_id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())
}

pub(crate) fn persist_handoff(
    database: &Database,
    run_id: &str,
    phase: Phase,
    participant: &str,
    handoff: Option<&AgentHandoff>,
) -> Result<(), String> {
    let Some(handoff) = handoff else {
        return Ok(());
    };
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    connection
        .execute(
            "INSERT INTO handoffs (id, run_id, phase, participant_kind, handoff_json)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                Uuid::new_v4().to_string(),
                run_id,
                phase.as_str(),
                participant,
                serde_json::to_string(handoff).map_err(|error| error.to_string())?
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) fn usage_note(usage: &ProviderUsage) -> String {
    if usage.input_tokens.is_some()
        || usage.cached_input_tokens.is_some()
        || usage.output_tokens.is_some()
        || usage.total_cost_usd.is_some()
    {
        "Provider-reported usage. Account quota and reset windows were not reported by the provider."
            .to_owned()
    } else {
        "Provider did not report token usage, account quota, or reset windows for this phase."
            .to_owned()
    }
}

pub(crate) fn persist_receipt(
    database: &Database,
    run_id: &str,
    phase: Phase,
    participant: &Participant,
    profile: &ProviderProfile,
    metrics: ReceiptMetrics,
    result: &ProviderRun,
) -> Result<(), String> {
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    connection
        .execute(
            "INSERT INTO execution_receipts
             (id, run_id, phase, participant_kind, provider_version, requested_model,
              requested_effort, actual_model, session_id, context_bytes, usage_json, usage_note,
              preflight_ms, process_start_ms, first_output_ms, total_ms, session_resumed,
              packet_bytes_saved, stdout_log_path, stderr_log_path)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
                     ?16, ?17, ?18, ?19, ?20)",
            params![
                Uuid::new_v4().to_string(),
                run_id,
                phase.as_str(),
                participant.kind,
                participant.version.as_deref(),
                profile.model.as_deref(),
                profile.effort.as_deref(),
                result.actual_model.as_deref(),
                result.session_id.as_deref(),
                metrics.context_bytes as i64,
                serde_json::to_string(&result.usage).map_err(|error| error.to_string())?,
                usage_note(&result.usage),
                metrics.preflight_ms as i64,
                result.process_start_ms as i64,
                result
                    .first_output_ms
                    .map(|milliseconds| { (metrics.preflight_ms + milliseconds) as i64 }),
                metrics.total_ms as i64,
                result.session_resumed as i64,
                metrics.packet_bytes_saved as i64,
                result.stdout_log_path,
                result.stderr_log_path,
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) fn persist_chat_receipt(
    database: &Database,
    project_id: &str,
    chat_id: &str,
    participant: &Participant,
    profile: &ProviderProfile,
    metrics: ChatReceiptMetrics,
    result: &ProviderRun,
) -> Result<(), String> {
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    connection
        .execute(
            "INSERT INTO chat_receipts
             (id, project_id, chat_id, phase, participant_kind, provider_version,
              requested_model, requested_effort, actual_model, session_id, context_bytes,
              usage_json, usage_note, preflight_ms, process_start_ms, first_output_ms, total_ms,
              session_resumed, packet_bytes_saved, stdout_log_path, stderr_log_path)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                     ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
            params![
                Uuid::new_v4().to_string(),
                project_id,
                chat_id,
                Phase::Chat.as_str(),
                participant.kind,
                participant.version.as_deref(),
                profile.model.as_deref(),
                profile.effort.as_deref(),
                result.actual_model.as_deref(),
                result.session_id.as_deref(),
                metrics.context_bytes as i64,
                serde_json::to_string(&result.usage).map_err(|error| error.to_string())?,
                usage_note(&result.usage),
                metrics.preflight_ms as i64,
                result.process_start_ms as i64,
                result
                    .first_output_ms
                    .map(|milliseconds| (metrics.preflight_ms + milliseconds) as i64),
                metrics.total_ms as i64,
                result.session_resumed as i64,
                0_i64,
                result.stdout_log_path,
                result.stderr_log_path,
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) fn run_artifact_directory(app: &AppHandle, run_id: &str) -> Result<PathBuf, String> {
    let path = app
        .path()
        .app_local_data_dir()
        .map_err(|error| error.to_string())?
        .join("runs")
        .join(run_id);
    std::fs::create_dir_all(&path).map_err(|error| error.to_string())?;
    Ok(path)
}

pub(crate) fn git_paths(repository: &Path, args: &[&str]) -> Result<Vec<String>, String> {
    let arguments = args
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    Ok(git_bytes(repository, &arguments)?
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| String::from_utf8_lossy(path).into_owned())
        .collect())
}
