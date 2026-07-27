use crate::*;

#[tauri::command]
pub(crate) fn load_provider_profiles(
    database: State<'_, Database>,
    project_id: String,
) -> Result<Vec<ProviderProfile>, String> {
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    let mut statement = connection
        .prepare(
            "SELECT participant_kind, route, model, effort
             FROM provider_route_profiles
             WHERE project_id = ?1 ORDER BY participant_kind, route",
        )
        .map_err(|error| error.to_string())?;
    let profiles = statement
        .query_map([project_id], |row| {
            Ok(ProviderProfile {
                participant_kind: row.get(0)?,
                route: row.get(1)?,
                model: row.get(2)?,
                effort: row.get(3)?,
            })
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    Ok(profiles)
}

#[tauri::command]
pub(crate) async fn save_provider_profile(
    database: State<'_, Database>,
    profile: ProviderProfileInput,
) -> Result<(), String> {
    if !matches!(
        profile.participant_kind.as_str(),
        "codex" | "claude" | "cursor" | "antigravity"
    ) || !matches!(profile.route.as_str(), "chat" | "build" | "review")
    {
        return Err("Unknown provider profile.".to_owned());
    }
    let model = profile.model.filter(|value| !value.trim().is_empty());
    let effort = profile.effort.filter(|value| !value.trim().is_empty());
    if let Some(value) = effort.as_deref() {
        let allowed = provider_effort_options(&profile.participant_kind);
        if !allowed.iter().any(|option| option == value) {
            return Err(format!(
                "{} does not support the `{value}` effort level.",
                provider_names(&profile.participant_kind).0
            ));
        }
        if profile.participant_kind == "cursor" && model.is_none() {
            return Err("Cursor requires an explicit model before effort can be set.".to_owned());
        }
    }
    {
        let connection = database.0.lock().map_err(|error| error.to_string())?;
        connection
            .execute(
                "INSERT INTO provider_route_profiles
                 (project_id, participant_kind, route, model, effort)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(project_id, participant_kind, route) DO UPDATE SET
                   model = excluded.model,
                   effort = excluded.effort,
                   updated_at = CURRENT_TIMESTAMP",
                params![
                    profile.project_id,
                    profile.participant_kind,
                    profile.route,
                    model,
                    effort
                ],
            )
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub(crate) fn provider_profile(
    database: &Database,
    project_id: &str,
    participant: &str,
    route: &str,
) -> Result<ProviderProfile, String> {
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    connection
        .query_row(
            "SELECT participant_kind, route, model, effort
             FROM provider_route_profiles
             WHERE project_id = ?1 AND participant_kind = ?2 AND route = ?3",
            params![project_id, participant, route],
            |row| {
                Ok(ProviderProfile {
                    participant_kind: row.get(0)?,
                    route: row.get(1)?,
                    model: row.get(2)?,
                    effort: row.get(3)?,
                })
            },
        )
        .optional()
        .map_err(|error| error.to_string())
        .map(|profile| {
            profile.unwrap_or(ProviderProfile {
                participant_kind: participant.to_owned(),
                route: route.to_owned(),
                model: None,
                effort: None,
            })
        })
}

pub(crate) fn recent_chat_handoff(
    database: &Database,
    project_id: &str,
    target_participant: &str,
    last_seen_message_rowid: i64,
    exclude_run_id: Option<&str>,
) -> Result<Option<String>, String> {
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    let mut statement = connection
        .prepare(
            "SELECT sender_kind, message_kind, body
             FROM messages
             WHERE project_id = ?1 AND rowid > ?2 AND message_kind IN ('human', 'agent')
               AND (?3 IS NULL OR run_id IS NULL OR run_id <> ?3)
             ORDER BY rowid ASC",
        )
        .map_err(|error| error.to_string())?;
    let messages = statement
        .query_map(
            params![project_id, last_seen_message_rowid, exclude_run_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    if messages.is_empty() {
        return Ok(None);
    }
    let mut handoff = format!(
        "Room messages since {} last saw the room:\n",
        provider_names(target_participant).0
    );
    for (sender, kind, body) in messages {
        let label = if kind == "human" {
            "User".to_owned()
        } else {
            provider_names(&sender).0.to_owned()
        };
        let entry = format!("{label}: {body}\n");
        let remaining = CHAT_HANDOFF_BUDGET_BYTES.saturating_sub(handoff.len());
        if remaining == 0 {
            break;
        }
        handoff.push_str(&truncate_utf8(&entry, remaining));
    }
    Ok(Some(handoff))
}

pub(crate) fn objective_needs_room_context(objective: &str) -> bool {
    let normalized = format!(" {} ", objective.trim().to_ascii_lowercase());
    objective.len() <= 180
        && [
            " it ",
            " this ",
            " that ",
            " those ",
            " above ",
            " add it ",
            " do it ",
            " fix it ",
            " go ahead ",
            " can you add ",
            " can you do ",
        ]
        .iter()
        .any(|signal| normalized.contains(signal))
}

#[tauri::command]
pub(crate) fn load_room(
    database: State<'_, Database>,
    project_id: String,
    before: Option<MessageCursor>,
) -> Result<RoomSnapshot, String> {
    load_room_snapshot(database.inner(), &project_id, before.as_ref())
}

pub(crate) fn stored_message_from_row(row: &Row<'_>) -> rusqlite::Result<StoredMessage> {
    let changed_json: String = row.get(6)?;
    let verification_json: String = row.get(7)?;
    Ok(StoredMessage {
        id: row.get(0)?,
        kind: row.get(1)?,
        sender: row.get(2)?,
        body: row.get(3)?,
        created_at: row.get(4)?,
        run_id: row.get(5)?,
        changed_files: serde_json::from_str(&changed_json).unwrap_or_default(),
        verification: serde_json::from_str(&verification_json).unwrap_or_default(),
        reason: row.get(8)?,
    })
}

pub(crate) fn load_room_snapshot(
    database: &Database,
    project_id: &str,
    before: Option<&MessageCursor>,
) -> Result<RoomSnapshot, String> {
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    let mut statement = connection
        .prepare(
            "SELECT id, message_kind, sender_kind, body, created_at, run_id,
                    changed_files_json, verification_json, reason
             FROM messages
             WHERE project_id = ?1
               AND (?2 IS NULL OR created_at < ?2 OR (created_at = ?2 AND id < ?3))
             ORDER BY created_at DESC, id DESC
             LIMIT ?4",
        )
        .map_err(|error| error.to_string())?;
    let mut messages = statement
        .query_map(
            params![
                project_id,
                before.map(|cursor| cursor.created_at.as_str()),
                before.map(|cursor| cursor.id.as_str()),
                (ROOM_MESSAGE_PAGE_SIZE + 1) as i64,
            ],
            stored_message_from_row,
        )
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    let has_more = messages.len() > ROOM_MESSAGE_PAGE_SIZE;
    messages.truncate(ROOM_MESSAGE_PAGE_SIZE);
    let next_message_cursor = messages.last().map(|message| MessageCursor {
        created_at: message.created_at.clone(),
        id: message.id.clone(),
    });
    messages.reverse();
    drop(statement);

    let latest_run = connection
        .query_row(
            "SELECT id, objective, state, current_owner, writer, reviewer,
                    review_count, revision_count, started_at, stop_reason,
                    native_session_id, worktree_path, branch, context_bytes, degraded_review,
                    artifact_path, instruction_files_json, skill_files_json, recovery_count
             FROM runs WHERE project_id = ?1 ORDER BY started_at DESC LIMIT 1",
            [&project_id],
            |row| {
                Ok(StoredRun {
                    id: row.get(0)?,
                    objective: row.get(1)?,
                    state: row.get(2)?,
                    current_owner: row.get(3)?,
                    writer: row.get(4)?,
                    reviewer: row.get(5)?,
                    review_count: row.get(6)?,
                    revision_count: row.get(7)?,
                    started_at: row.get(8)?,
                    stop_reason: row.get(9)?,
                    native_session_id: row.get(10)?,
                    worktree_path: row.get(11)?,
                    branch: row.get(12)?,
                    context_bytes: row.get::<_, i64>(13)?.max(0) as usize,
                    degraded_review: row.get::<_, i64>(14)? != 0,
                    artifact_path: row.get(15)?,
                    instruction_files: serde_json::from_str(&row.get::<_, String>(16)?)
                        .unwrap_or_default(),
                    skill_files: serde_json::from_str(&row.get::<_, String>(17)?)
                        .unwrap_or_default(),
                    recovery_count: row.get::<_, i64>(18)?.max(0) as u32,
                    route: stored_run_route(
                        &row.get::<_, String>(2)?,
                        row.get::<_, i64>(6)?.max(0) as u32,
                        row.get::<_, Option<String>>(4)?.as_deref(),
                        row.get::<_, Option<String>>(5)?.as_deref(),
                    ),
                })
            },
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let mut receipts = latest_run
        .as_ref()
        .map(|run| {
            let mut statement = connection
                .prepare(
                    "SELECT id, phase, participant_kind, provider_version, requested_model,
                            requested_effort, actual_model, session_id, context_bytes, usage_json,
                            usage_note, created_at, preflight_ms, process_start_ms, first_output_ms,
                            total_ms, session_resumed, packet_bytes_saved, stdout_log_path, stderr_log_path
                     FROM execution_receipts WHERE run_id = ?1 ORDER BY created_at ASC",
                )
                .map_err(|error| error.to_string())?;
            let receipts = statement
                .query_map([&run.id], |row| {
                    let usage_json: String = row.get(9)?;
                    Ok(ExecutionReceipt {
                        id: row.get(0)?,
                        phase: row.get(1)?,
                        participant: row.get(2)?,
                        provider_version: row.get(3)?,
                        requested_model: row.get(4)?,
                        requested_effort: row.get(5)?,
                        actual_model: row.get(6)?,
                        session_id: row.get(7)?,
                        context_bytes: row.get::<_, i64>(8)?.max(0) as usize,
                        usage: serde_json::from_str(&usage_json).unwrap_or_default(),
                        usage_note: row.get(10)?,
                        created_at: row.get(11)?,
                        preflight_ms: row.get::<_, Option<i64>>(12)?.map(|value| value.max(0) as u64),
                        process_start_ms: row.get::<_, Option<i64>>(13)?.map(|value| value.max(0) as u64),
                        first_output_ms: row.get::<_, Option<i64>>(14)?.map(|value| value.max(0) as u64),
                        total_ms: row.get::<_, Option<i64>>(15)?.map(|value| value.max(0) as u64),
                        session_resumed: row.get::<_, i64>(16)? != 0,
                        packet_bytes_saved: row.get::<_, i64>(17)?.max(0) as usize,
                        stdout_log_path: row.get(18)?,
                        stderr_log_path: row.get(19)?,
                    })
                })
                .map_err(|error| error.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| error.to_string())?;
            Ok::<Vec<ExecutionReceipt>, String>(receipts)
        })
        .transpose()?
        .unwrap_or_default();
    let mut statement = connection
        .prepare(
            "SELECT id, phase, participant_kind, provider_version, requested_model,
                    requested_effort, actual_model, session_id, context_bytes, usage_json,
                    usage_note, created_at, preflight_ms, process_start_ms, first_output_ms, total_ms,
                    session_resumed, packet_bytes_saved, stdout_log_path, stderr_log_path
             FROM chat_receipts WHERE project_id = ?1
             ORDER BY created_at DESC LIMIT 50",
        )
        .map_err(|error| error.to_string())?;
    let chat_receipts = statement
        .query_map([&project_id], |row| {
            let usage_json: String = row.get(9)?;
            Ok(ExecutionReceipt {
                id: row.get(0)?,
                phase: row.get(1)?,
                participant: row.get(2)?,
                provider_version: row.get(3)?,
                requested_model: row.get(4)?,
                requested_effort: row.get(5)?,
                actual_model: row.get(6)?,
                session_id: row.get(7)?,
                context_bytes: row.get::<_, i64>(8)?.max(0) as usize,
                usage: serde_json::from_str(&usage_json).unwrap_or_default(),
                usage_note: row.get(10)?,
                created_at: row.get(11)?,
                preflight_ms: row
                    .get::<_, Option<i64>>(12)?
                    .map(|value| value.max(0) as u64),
                process_start_ms: row
                    .get::<_, Option<i64>>(13)?
                    .map(|value| value.max(0) as u64),
                first_output_ms: row
                    .get::<_, Option<i64>>(14)?
                    .map(|value| value.max(0) as u64),
                total_ms: row
                    .get::<_, Option<i64>>(15)?
                    .map(|value| value.max(0) as u64),
                session_resumed: row.get::<_, i64>(16)? != 0,
                packet_bytes_saved: row.get::<_, i64>(17)?.max(0) as usize,
                stdout_log_path: row.get(18)?,
                stderr_log_path: row.get(19)?,
            })
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    receipts.extend(chat_receipts);
    receipts.sort_by(|left, right| left.created_at.cmp(&right.created_at));
    Ok(RoomSnapshot {
        messages,
        has_more,
        next_message_cursor,
        latest_run,
        receipts,
    })
}

pub(crate) fn stored_run_route(
    state: &str,
    review_count: u32,
    writer: Option<&str>,
    reviewer: Option<&str>,
) -> Vec<StoredRouteStep> {
    let phases = [
        ("Build", writer),
        ("Verify", None),
        ("Review", reviewer),
        ("Revise", writer),
        ("Final review", reviewer),
        ("Promote", None),
    ];
    let current = match state {
        "selecting" | "working" => 0,
        "verifying" => 1,
        "reviewing" if review_count > 1 => 4,
        "reviewing" => 2,
        "revising" => 3,
        "promoting" => 5,
        "complete" => phases.len(),
        _ => 0,
    };
    phases
        .iter()
        .enumerate()
        .map(|(index, (label, agent))| StoredRouteStep {
            agent: agent.map(str::to_owned),
            label: (*label).to_owned(),
            state: if index < current {
                "complete"
            } else if index == current {
                "current"
            } else {
                "next"
            }
            .to_owned(),
        })
        .collect()
}

#[tauri::command]
pub(crate) async fn stop_run(
    runtime: State<'_, RuntimeState>,
    run_id: String,
) -> Result<StopRunResult, String> {
    let sender = runtime.cancellations.lock().await.get(&run_id).cloned();
    match sender {
        Some(sender) if sender.send(true).is_ok() => Ok(StopRunResult {
            cancelled: true,
            reason: "Cancellation requested. Agent Room will preserve recoverable work.".to_owned(),
        }),
        Some(_) => Ok(StopRunResult {
            cancelled: false,
            reason: "This run has already finished and can no longer be cancelled.".to_owned(),
        }),
        None => Ok(StopRunResult {
            cancelled: false,
            reason: "Cancellation is not available yet or the run has already finished.".to_owned(),
        }),
    }
}

#[tauri::command]
pub(crate) fn abandon_run(
    app: AppHandle,
    database: State<'_, Database>,
    run_id: String,
) -> Result<(), String> {
    type AbandonRow = (String, Option<String>, String, String, String);
    let database = database.inner();
    let row = {
        let connection = database.0.lock().map_err(|error| error.to_string())?;
        connection
            .query_row(
                "SELECT runs.state, runs.worktree_path, runs.branch, runs.isolation_kind,
                        projects.repository_path
                 FROM runs JOIN projects ON projects.id = runs.project_id
                 WHERE runs.id = ?1",
                [&run_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "The Ship run could not be found.".to_owned())?
    };
    let (state, worktree_path, branch, isolation_kind, repository_path): AbandonRow = row;
    if run_state_allows_side_chat(&state) || state == "promoting" || state == "selecting" {
        return Err("An active Ship run cannot be abandoned. Stop it first.".to_owned());
    }
    let worktree = PathBuf::from(
        worktree_path
            .ok_or_else(|| "This Ship run has no preserved worktree to abandon.".to_owned())?,
    );
    if !worktree.exists() {
        return Err("The preserved worktree no longer exists.".to_owned());
    }
    let managed_root = app
        .path()
        .app_cache_dir()
        .map_err(|error| error.to_string())?
        .join("worktrees");
    let isolation = IsolationContext {
        worktree,
        branch,
        base_branch: String::new(),
        base_head: String::new(),
        snapshot_head: String::new(),
        isolation_kind,
        workspace_fingerprint: None,
    };
    if let Some(error) = discard_isolation(Path::new(&repository_path), &isolation, &managed_root) {
        return Err(format!("Could not remove the preserved isolation: {error}"));
    }
    update_run(
        database,
        &run_id,
        "abandoned",
        None,
        0,
        0,
        None,
        0,
        None,
        true,
    )?;
    database
        .0
        .lock()
        .map_err(|error| error.to_string())?
        .execute(
            "UPDATE runs SET worktree_path = NULL WHERE id = ?1",
            [&run_id],
        )
        .map_err(|error| error.to_string())?;
    emit_event(
        &app,
        &run_id,
        "complete",
        "ship",
        "abandoned",
        None,
        "Ship run abandoned",
        "The preserved worktree was removed.",
        None,
    );
    Ok(())
}

#[tauri::command]
pub(crate) async fn test_provider_connection(
    app: AppHandle,
    database: State<'_, Database>,
    runtime: State<'_, RuntimeState>,
    request: ConnectionTestRequest,
) -> Result<Participant, String> {
    let database = database.inner();
    let repository = PathBuf::from(&request.repository_path);
    git_static(&repository, &["rev-parse", "--show-toplevel"])
        .map_err(|_| "The attached path is not a Git repository.".to_owned())?;
    let participant = probe_provider(&request.participant_kind);
    runtime
        .provider_cache
        .lock()
        .await
        .insert(participant.kind.clone(), participant.clone());
    if !participant.installed {
        return Err(format!("{} CLI is not installed.", participant.name));
    }
    let profile = provider_profile(database, &request.project_id, &participant.kind, "chat")?;
    let run_id = format!("connection-{}", Uuid::new_v4());
    let output_path = run_artifact_directory(&app, &run_id)?.join("connection-test.final.txt");
    let (cancel_sender, cancellation) = watch::channel(false);
    runtime
        .cancellations
        .lock()
        .await
        .insert(run_id.clone(), cancel_sender);
    let result = invoke_provider(
        &app,
        &run_id,
        &participant,
        Phase::Chat,
        ProviderMode::Ask,
        "This is an Agent Room connection test. Reply with exactly READY. Do not inspect or modify files.",
        &repository,
        None,
        profile.model.as_deref(),
        profile.effort.as_deref(),
        &output_path,
        cancellation,
    )
    .await;
    runtime.cancellations.lock().await.remove(&run_id);
    let result = result?;
    let ready = connection_test_ready(&result);
    let authentication_detail = authentication_attention(&result.summary, &result.stderr);
    let connection = if ready {
        ProviderConnection {
            status: "connected".to_owned(),
            detail: "Native connection test passed.".to_owned(),
            last_verified_at: None,
        }
    } else {
        let status = if authentication_detail.is_some() {
            "sign-in-required"
        } else {
            "failed"
        };
        ProviderConnection {
            status: status.to_owned(),
            detail: if let Some(detail) = authentication_detail {
                detail
            } else if result.idle_timed_out {
                format!("No provider output within {PROCESS_IDLE_TIMEOUT_SECONDS} seconds.")
            } else if !result.summary.trim().is_empty() {
                format!(
                    "Connection test expected READY but received: {}",
                    truncate_utf8(result.summary.trim(), 320)
                )
            } else {
                provider_failure_reason(&participant.name, "the connection test", &result)
            },
            last_verified_at: None,
        }
    };
    save_connection(
        database,
        &request.project_id,
        &participant.kind,
        &connection,
    )?;
    if connection.status != "connected" {
        return Err(connection.detail);
    }
    participant_for_project(database, &request.project_id, participant)
}

#[tauri::command]
pub(crate) async fn discover_provider_models(
    request: ModelDiscoveryRequest,
) -> Result<ModelDiscoveryResult, String> {
    let executable = find_provider_executable(&request.participant_kind)
        .ok_or_else(|| "Install this CLI before refreshing its models.".to_owned())?;
    match request.participant_kind.as_str() {
        "antigravity" | "cursor" => {
            let output = timeout(Duration::from_secs(15), {
                let mut command = Command::new(executable);
                hide_tokio_command_window(&mut command);
                command.arg("models").output()
            })
            .await
            .map_err(|_| "Model discovery timed out after 15 seconds.".to_owned())?
            .map_err(|error| format!("Could not start model discovery: {error}"))?;
            let text = format!(
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            if !output.status.success() {
                return Err(truncate_utf8(&text, 500));
            }
            let models = if request.participant_kind == "cursor" {
                parse_cursor_model_list(&text)
            } else {
                parse_provider_model_list(&text)
            };
            if models.is_empty() {
                return Err("The CLI returned no selectable models for this account.".to_owned());
            }
            Ok(ModelDiscoveryResult {
                models,
                detail: if request.participant_kind == "cursor" {
                    "Fetched from the signed-in CLI. Effort, thinking, context, and speed variants are grouped under each base model.".to_owned()
                } else {
                    "Fetched from the signed-in CLI account just now.".to_owned()
                },
            })
        }
        "codex" | "claude" => {
            let (models, detail, _) =
                model_options(&request.participant_kind, Some(executable.as_path()));
            Ok(ModelDiscoveryResult { models, detail })
        }
        _ => Err("Unknown provider.".to_owned()),
    }
}
