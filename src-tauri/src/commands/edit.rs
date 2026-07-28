use crate::*;

#[tauri::command]
pub(crate) async fn quick_edit_start(
    app: AppHandle,
    database: State<'_, Database>,
    runtime: State<'_, RuntimeState>,
    request: QuickEditRequest,
) -> Result<QuickEditResult, String> {
    let database = database.inner();
    consume_operation_id(
        runtime.inner(),
        &request.edit_id,
        &request.project_id,
        "quick-edit",
    )
    .await?;
    let repository = project_repository(database, &request.project_id)?;
    let message = request.message.trim();
    if message.is_empty() {
        return Err("Enter a Quick Edit request before sending.".to_owned());
    }
    let participants = participants_with_connections(
        database,
        &request.project_id,
        cached_participants(runtime.inner(), false).await?,
    )?;
    let participant = if let Some(requested) = request.requested_agent.as_deref() {
        participants
            .into_iter()
            .find(|participant| participant.kind == requested)
            .ok_or_else(|| format!("The requested {requested} CLI is unavailable."))?
    } else {
        participants
            .into_iter()
            .find(|participant| participant.installed && participant.kind != "antigravity")
            .ok_or_else(|| {
                "No Full-tier coding-agent CLI is available for Quick Edit.".to_owned()
            })?
    };
    if !participant.installed || participant.kind == "antigravity" {
        return Err(
            "Quick Edit is available only to Codex, Claude Code, and Cursor Agent.".to_owned(),
        );
    }
    let worktree = create_quick_edit_worktree(&app, &repository, &request.edit_id)?;
    let state = QuickEditState {
        project_id: request.project_id.clone(),
        repository,
        worktree: worktree.clone(),
    };
    runtime
        .quick_edits
        .lock()
        .await
        .insert(request.edit_id.clone(), state.clone());
    let profile = provider_profile(database, &request.project_id, &participant.kind, "chat")?;
    let artifact_dir = run_artifact_directory(&app, &request.edit_id)?;
    let output_path = artifact_dir.join("quick-edit.final.txt");
    let (cancel_sender, cancel_receiver) = watch::channel(false);
    if runtime
        .cancellations
        .lock()
        .await
        .insert(
            request.edit_id.clone(),
            CancellationEntry {
                project_id: request.project_id.clone(),
                sender: cancel_sender,
            },
        )
        .is_some()
    {
        runtime.quick_edits.lock().await.remove(&request.edit_id);
        let _ = remove_quick_edit_worktree(&state.repository, &state.worktree);
        return Err("This operation ID is already active.".to_owned());
    }
    let prompt = truncate_utf8(&format!(
        "You are in Quick Edit mode inside an isolated worktree. Make only the requested bounded edit. \
         Do not modify the user's attached checkout, create another worktree, run broad verification, or start Ship. \
         Explain the completed edit briefly.\n\nUser request:\n{message}"
    ), 16 * 1024);
    let result = invoke_provider(
        &app,
        &request.edit_id,
        &participant,
        Phase::Chat,
        ProviderMode::QuickEdit,
        &prompt,
        &worktree,
        None,
        profile.model.as_deref(),
        profile.effort.as_deref(),
        &output_path,
        cancel_receiver,
    )
    .await;
    runtime.cancellations.lock().await.remove(&request.edit_id);
    let result = match result {
        Ok(result) => result,
        Err(error) => {
            runtime.quick_edits.lock().await.remove(&request.edit_id);
            let _ = remove_quick_edit_worktree(&state.repository, &state.worktree);
            return Err(error);
        }
    };
    if !result.success {
        runtime.quick_edits.lock().await.remove(&request.edit_id);
        let _ = remove_quick_edit_worktree(&state.repository, &state.worktree);
        return Err(provider_failure_reason(
            &participant.name,
            "the Quick Edit",
            &result,
        ));
    }
    Ok(QuickEditResult {
        edit_id: request.edit_id,
        participant: participant.kind,
        summary: result.summary,
        diff: quick_edit_diff(&worktree)?,
        stopped: result.stopped,
    })
}

#[tauri::command]
pub(crate) async fn quick_edit_apply(
    app: AppHandle,
    runtime: State<'_, RuntimeState>,
    request: QuickEditActionRequest,
) -> Result<QuickEditActionResult, String> {
    let state = runtime
        .quick_edits
        .lock()
        .await
        .get(&request.edit_id)
        .cloned()
        .ok_or_else(|| "This Quick Edit is no longer available.".to_owned())?;
    if state.project_id != request.project_id {
        return Err("This Quick Edit belongs to another project.".to_owned());
    }
    let diff = quick_edit_diff(&state.worktree)?;
    if diff.trim().is_empty() {
        return Err("The Quick Edit produced no changes to apply.".to_owned());
    }
    let patch = app
        .path()
        .app_cache_dir()
        .map_err(|error| error.to_string())?
        .join("quick-edits")
        .join(format!(".apply-{}.patch", request.edit_id));
    std::fs::write(&patch, diff).map_err(|error| error.to_string())?;
    let apply_result = (|| {
        git(
            &state.repository,
            &[
                "apply".to_owned(),
                "--check".to_owned(),
                "--binary".to_owned(),
                patch.to_string_lossy().into_owned(),
            ],
        )?;
        git(
            &state.repository,
            &[
                "apply".to_owned(),
                "--binary".to_owned(),
                patch.to_string_lossy().into_owned(),
            ],
        )
    })();
    let _ = std::fs::remove_file(&patch);
    apply_result?;
    runtime.quick_edits.lock().await.remove(&request.edit_id);
    Ok(QuickEditActionResult {
        cleanup_warning: remove_quick_edit_worktree(&state.repository, &state.worktree).err(),
    })
}

#[tauri::command]
pub(crate) async fn quick_edit_discard(
    runtime: State<'_, RuntimeState>,
    request: QuickEditActionRequest,
) -> Result<QuickEditActionResult, String> {
    let state = runtime
        .quick_edits
        .lock()
        .await
        .get(&request.edit_id)
        .cloned()
        .ok_or_else(|| "This Quick Edit is no longer available.".to_owned())?;
    if state.project_id != request.project_id {
        return Err("This Quick Edit belongs to another project.".to_owned());
    }
    remove_quick_edit_worktree(&state.repository, &state.worktree)?;
    runtime.quick_edits.lock().await.remove(&request.edit_id);
    Ok(QuickEditActionResult {
        cleanup_warning: None,
    })
}
