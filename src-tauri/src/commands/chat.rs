use crate::*;

pub(crate) fn cursor_unavailable_model(stderr: &str) -> Option<String> {
    stderr.lines().find_map(|line| {
        let line = line.trim();
        let prefix = "cannot use this model:";
        if !line.to_ascii_lowercase().starts_with(prefix) {
            return None;
        }
        let model = line.get(prefix.len()..)?.trim();
        let model = model
            .split_once(". Available models:")
            .map(|(value, _)| value)
            .unwrap_or(model)
            .trim();
        (!model.is_empty()).then(|| model.to_owned())
    })
}

fn clear_cursor_chat_model(database: &Database, project_id: &str) -> Result<(), String> {
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    connection
        .execute(
            "UPDATE provider_route_profiles
             SET model = NULL, effort = NULL, updated_at = CURRENT_TIMESTAMP
             WHERE project_id = ?1 AND participant_kind = 'cursor' AND route = 'chat'",
            params![project_id],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub(crate) async fn start_room_chat(
    app: AppHandle,
    database: State<'_, Database>,
    runtime: State<'_, RuntimeState>,
    request: ChatRequest,
) -> Result<ChatResult, String> {
    let chat_started = Instant::now();
    let database = database.inner();
    consume_operation_id(
        runtime.inner(),
        &request.run_id,
        &request.project_id,
        "chat",
    )
    .await?;
    let active_run = if let Some(active_run_id) = request.active_run_id.as_deref() {
        let connection = database.0.lock().map_err(|error| error.to_string())?;
        connection
            .query_row(
                "SELECT worktree_path, current_owner, writer, reviewer, state
                 FROM runs WHERE id = ?1 AND project_id = ?2",
                params![active_run_id, request.project_id],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "The active Ship run could not be found.".to_owned())
            .and_then(|(worktree, current_owner, writer, reviewer, state)| {
                if !run_state_allows_side_chat(&state) {
                    return Err("This Ship run is no longer active.".to_owned());
                }
                let worktree = worktree
                    .ok_or_else(|| "The active Ship worktree is unavailable.".to_owned())?;
                let preferred_owner = if state == "reviewing" {
                    reviewer.clone()
                } else {
                    writer.clone()
                };
                let owner = current_owner
                    .or(preferred_owner)
                    .or(writer)
                    .or(reviewer)
                    .ok_or_else(|| {
                        "The active Ship run has no available participant.".to_owned()
                    })?;
                Ok((PathBuf::from(worktree), owner))
            })?
            .into()
    } else {
        None
    };
    let repository = active_run
        .as_ref()
        .map(|(repository, _)| repository.clone())
        .map_or_else(|| project_repository(database, &request.project_id), Ok)?;
    let message = request.message.trim();
    if message.is_empty() {
        return Err("Enter a message before sending.".to_owned());
    }
    let requested_agent = active_run
        .as_ref()
        .map(|(_, owner)| owner.as_str())
        .or(request.requested_agent.as_deref());
    let participant = if let Some(requested) = requested_agent {
        let participant = participant_for_project(
            database,
            &request.project_id,
            cached_provider(runtime.inner(), requested, false).await?,
        )?;
        if !participant.installed {
            return Err(format!("The requested {requested} CLI is not installed."));
        }
        participant
    } else {
        participants_with_connections(
            database,
            &request.project_id,
            cached_participants(runtime.inner(), false).await?,
        )?
        .into_iter()
        .find(|participant| participant.installed)
        .ok_or_else(|| "No installed coding-agent CLI is available for chat.".to_owned())?
    };
    if participant.kind == "antigravity" {
        return Err(
            "Antigravity is available for Ship only because it has no true read-only mode."
                .to_owned(),
        );
    }
    let profile = provider_profile(database, &request.project_id, &participant.kind, "chat")?;
    let settings = project_settings(database, &request.project_id)?;
    let session_id = load_chat_session(database, &request.project_id, &participant.kind)?;
    let room_handoff = recent_chat_handoff(
        database,
        &request.project_id,
        &participant.kind,
        session_id
            .as_ref()
            .map(|session| session.last_seen_message_rowid)
            .unwrap_or_default(),
        None,
    )?;
    persist_message(
        database,
        &request.project_id,
        &request.run_id,
        "human",
        "human",
        message,
        &[],
        &[],
        None,
    )?;
    let chat_mode_detail = if active_run.is_some() {
        "Read-only side chat attached to the active managed worktree. The autonomous Ship route continues separately."
    } else {
        "Instant project chat. No worktree, verification, review, or promotion."
    };
    emit_event(
        &app,
        &request.run_id,
        "phase",
        "chat",
        "working",
        Some(&participant.kind),
        &format!("{} is answering", participant.name),
        chat_mode_detail,
        None,
    );
    let mut prompt = if active_run.is_some() {
        format!(
            "You are in a read-only side chat attached to an active Agent Room Ship worktree. \
             Answer the user's question directly and concisely using the worktree's current state. \
             Do not modify files, interrupt or steer the active builder, start another Ship run, \
             run full project verification, or emit an Agent Room Ship intent or handoff.\n\nUser message:\n{message}"
        )
    } else {
        format!(
            "You are in Ask mode for the current repository. Answer questions directly and concisely. \
             Do not modify files, create a worktree, run broad project verification, or use an Agent Room handoff. \
             For substantial or unattended implementation work, do not edit first; use the autonomous Ship intent when its trigger matches.\n\nUser message:\n{message}"
        )
    };
    if settings.autonomous_ship_enabled && active_run.is_none() {
        if session_id.is_some() {
            prompt.push_str("\n\nAutonomous Ship remains armed. Follow the previously supplied skill when its trigger matches.");
        } else {
            prompt.push_str(&format!(
                "\n\nAutonomous Ship is armed for this project. Apply the following skill when its trigger matches. The skill is `{AUTONOMOUS_SHIP_SKILL_PATH}`:\n\n{AUTONOMOUS_SHIP_SKILL}"
            ));
        }
    }
    if let Some(handoff) = room_handoff {
        prompt.push_str("\n\n");
        prompt.push_str(&handoff);
    }
    let prompt = truncate_utf8(&prompt, Phase::Chat.context_budget_bytes());
    let artifact_dir = run_artifact_directory(&app, &request.run_id)?;
    let output_path = artifact_dir.join("chat.final.txt");
    let (cancel_sender, cancel_receiver) = watch::channel(false);
    if runtime
        .cancellations
        .lock()
        .await
        .insert(
            request.run_id.clone(),
            CancellationEntry {
                project_id: request.project_id.clone(),
                sender: cancel_sender,
            },
        )
        .is_some()
    {
        return Err("This operation ID is already active.".to_owned());
    }
    let preflight_ms = chat_started.elapsed().as_millis() as u64;
    let first_result = invoke_provider(
        &app,
        &request.run_id,
        &participant,
        Phase::Chat,
        ProviderMode::Ask,
        &prompt,
        &repository,
        session_id
            .as_ref()
            .map(|session| session.provider_session_id.as_str()),
        profile.model.as_deref(),
        profile.effort.as_deref(),
        &output_path,
        cancel_receiver.clone(),
    )
    .await;
    let result = first_result;
    runtime.cancellations.lock().await.remove(&request.run_id);
    let result = match result {
        Ok(result) => result,
        Err(error) => {
            runtime
                .provider_cache
                .lock()
                .await
                .remove(&participant.kind);
            return Err(error);
        }
    };
    let unavailable_cursor_model = if participant.kind == "cursor" {
        cursor_unavailable_model(&result.stderr)
    } else {
        None
    };
    if unavailable_cursor_model.is_some() {
        clear_cursor_chat_model(database, &request.project_id)?;
    }
    let total_ms = chat_started.elapsed().as_millis() as u64;
    persist_chat_receipt(
        database,
        &request.project_id,
        &request.run_id,
        &participant,
        &profile,
        ChatReceiptMetrics {
            context_bytes: prompt.len(),
            preflight_ms,
            total_ms,
        },
        &result,
    )?;
    let authentication_detail = authentication_attention(&result.summary, &result.stderr);
    if !result.success || authentication_detail.is_some() {
        let reason = if let Some(model) = unavailable_cursor_model {
            format!(
                "Cursor rejected the saved model `{model}`. It was cleared; refresh Cursor models and choose an exact identifier before retrying."
            )
        } else if let Some(detail) = authentication_detail {
            detail
        } else if result.stopped {
            "Chat stopped by you.".to_owned()
        } else if result.timed_out {
            "Chat exceeded the 20 minute limit.".to_owned()
        } else if result.idle_timed_out {
            format!(
                "Chat produced no output for {PROCESS_IDLE_TIMEOUT_SECONDS} seconds and was stopped."
            )
        } else {
            provider_failure_reason(&participant.name, "the chat response", &result)
        };
        let status = if authentication_attention(&result.summary, &result.stderr).is_some() {
            "sign-in-required"
        } else {
            "failed"
        };
        save_connection(
            database,
            &request.project_id,
            &participant.kind,
            &ProviderConnection {
                status: status.to_owned(),
                detail: reason.clone(),
                last_verified_at: None,
            },
        )?;
        persist_message(
            database,
            &request.project_id,
            &request.run_id,
            "system",
            "error",
            "Chat needs attention.",
            &[],
            &[],
            Some(&reason),
        )?;
        emit_event(
            &app,
            &request.run_id,
            "attention",
            "chat",
            if result.stopped { "stopped" } else { "failed" },
            Some(&participant.kind),
            "Chat needs attention",
            &reason,
            None,
        );
        return Err(reason);
    }

    save_connection(
        database,
        &request.project_id,
        &participant.kind,
        &ProviderConnection {
            status: "connected".to_owned(),
            detail: "Most recent native project chat completed.".to_owned(),
            last_verified_at: None,
        },
    )?;

    let (ship_intent, intent_diagnostic) = if settings.autonomous_ship_enabled {
        match extract_ship_intent(&result.summary) {
            Ok(intent) => (intent, None),
            Err(error) => (None, Some(error)),
        }
    } else {
        (None, None)
    };
    let visible_summary = truncate_utf8(
        &visible_chat_response(&result.summary),
        CHAT_TIMELINE_BUDGET_BYTES,
    );
    persist_message(
        database,
        &request.project_id,
        &request.run_id,
        &participant.kind,
        "agent",
        &visible_summary,
        &[],
        &[],
        None,
    )?;
    save_chat_session(
        database,
        &request.project_id,
        &participant.kind,
        result.session_id.as_deref(),
        latest_room_message_marker(database, &request.project_id)?,
    )?;
    if let Some(intent) = ship_intent.as_ref() {
        persist_message(
            database,
            &request.project_id,
            &request.run_id,
            "system",
            "status",
            "Autonomous Ship intent accepted.",
            &[],
            &[],
            Some(&format!("{}\n{}", intent.objective, intent.reason)),
        )?;
    } else if let Some(diagnostic) = intent_diagnostic {
        persist_message(
            database,
            &request.project_id,
            &request.run_id,
            "system",
            "status",
            "Autonomous Ship intent was ignored.",
            &[],
            &[],
            Some(&diagnostic),
        )?;
    }
    emit_event(
        &app,
        &request.run_id,
        "complete",
        "chat",
        "complete",
        Some(&participant.kind),
        "Chat complete",
        "Project chat response saved to this room.",
        None,
    );
    Ok(ChatResult {
        run_id: request.run_id,
        participant: participant.kind.clone(),
        summary: visible_summary,
        session_id: result.session_id,
        actual_model: result.actual_model,
        stopped: false,
        ship_intent,
    })
}
