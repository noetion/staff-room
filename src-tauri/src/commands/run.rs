use crate::*;

#[tauri::command]
pub(crate) async fn start_room_run(
    app: AppHandle,
    database: State<'_, Database>,
    runtime: State<'_, RuntimeState>,
    request: StartRunRequest,
) -> Result<StartRunResult, String> {
    let run_id = request.run_id.clone();
    let project_id = request.project_id.clone();
    let is_recovery = {
        let connection = database.0.lock().map_err(|error| error.to_string())?;
        connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM runs WHERE id = ?1 AND project_id = ?2)",
                params![run_id, project_id],
                |row| row.get::<_, bool>(0),
            )
            .map_err(|error| error.to_string())?
    };
    if !is_recovery {
        let unresolved = {
            let connection = database.0.lock().map_err(|error| error.to_string())?;
            connection
                .query_row(
                    // 'promoting' must be included: promotion flips the row out of
                    // 'awaiting-promotion' before it starts seconds of git work, and
                    // a new Ship run starting inside that window snapshots a
                    // half-applied checkout.
                    "SELECT EXISTS(
                        SELECT 1 FROM runs
                        WHERE project_id = ?1
                          AND worktree_path IS NOT NULL
                          AND state IN ('awaiting-promotion', 'promoting')
                    )",
                    [&project_id],
                    |row| row.get::<_, bool>(0),
                )
                .map_err(|error| error.to_string())?
        };
        if unresolved {
            return Err(
                "Promote or abandon the verified Ship result before starting another Ship run."
                    .to_owned(),
            );
        }
    }
    if !is_recovery {
        consume_operation_id(runtime.inner(), &run_id, &project_id, "ship").await?;
    }
    {
        let mut active_runs = runtime.active_ship_runs.lock().await;
        if !active_runs.is_empty() {
            return Err("A Ship run is already active. Stop, wait for, or abandon it before starting another.".to_owned());
        }
        active_runs.insert(run_id.clone());
    }
    let result = execute_room_run(app.clone(), database.inner(), runtime.inner(), request).await;
    runtime.active_ship_runs.lock().await.remove(&run_id);
    if let Err(error) = result.as_ref() {
        let _ = finalize_unhandled_run_error(&app, database.inner(), &project_id, &run_id, error);
        runtime.cancellations.lock().await.remove(&run_id);
    }
    result
}

#[tauri::command]
pub(crate) async fn approve_run_promotion(
    app: AppHandle,
    database: State<'_, Database>,
    runtime: State<'_, RuntimeState>,
    request: ProjectRunRequest,
) -> Result<PromotionActionResult, String> {
    project_repository(database.inner(), &request.project_id)?;
    {
        let mut promotions = runtime.active_promotions.lock().await;
        if !promotions.insert(request.project_id.clone()) {
            return Err("Promotion is already in progress for this project.".to_owned());
        }
    }
    let result = approve_run_promotion_inner(&app, database.inner(), &request);
    runtime
        .active_promotions
        .lock()
        .await
        .remove(&request.project_id);
    result
}

fn approve_run_promotion_inner(
    app: &AppHandle,
    database: &Database,
    request: &ProjectRunRequest,
) -> Result<PromotionActionResult, String> {
    type PromotionRow = (
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        String,
        u32,
        u32,
        usize,
        Option<String>,
        Option<String>,
    );
    let row: PromotionRow = {
        let connection = database.0.lock().map_err(|error| error.to_string())?;
        connection
            .query_row(
                "SELECT state, worktree_path, branch, base_head, base_branch, snapshot_head,
                        isolation_kind, review_count, revision_count, context_bytes,
                        workspace_fingerprint, reviewed_fingerprint
                 FROM runs WHERE id = ?1 AND project_id = ?2",
                params![request.run_id, request.project_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get::<_, i64>(7)?.max(0) as u32,
                        row.get::<_, i64>(8)?.max(0) as u32,
                        row.get::<_, i64>(9)?.max(0) as usize,
                        row.get(10)?,
                        row.get(11)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "The verified Ship run could not be found.".to_owned())?
    };
    let (
        state,
        worktree_path,
        branch,
        base_head,
        base_branch,
        snapshot_head,
        isolation_kind,
        review_count,
        revision_count,
        context_bytes,
        workspace_fingerprint,
        reviewed_fingerprint,
    ) = row;
    if state != "awaiting-promotion" {
        return Err("This run is not awaiting human promotion.".to_owned());
    }
    let base_branch =
        base_branch.ok_or_else(|| "The verified run has no recorded target branch.".to_owned())?;
    let snapshot_head = snapshot_head
        .ok_or_else(|| "The verified run has no recorded repository snapshot.".to_owned())?;
    let base_repository = project_repository(database, &request.project_id)?;
    let worktree = PathBuf::from(worktree_path);
    if !worktree.is_dir() {
        return Err("The verified worktree is no longer available.".to_owned());
    }
    let reviewed_fingerprint = reviewed_fingerprint
        .ok_or_else(|| "The run has no recorded reviewed-worktree fingerprint.".to_owned())?;
    if review_mutation_guard(&worktree)? != reviewed_fingerprint {
        return Err(
            "The isolated worktree changed after review. Run review and verification again before promotion."
                .to_owned(),
        );
    }
    let isolation = IsolationContext {
        worktree: worktree.clone(),
        branch,
        base_branch,
        base_head,
        snapshot_head: snapshot_head.clone(),
        isolation_kind,
        workspace_fingerprint,
    };
    let managed_root = app
        .path()
        .app_cache_dir()
        .map_err(|error| error.to_string())?
        .join("worktrees");
    // Claim the run atomically before any git work begins. The `state` read above
    // is only a fast-fail; promotion then spends seconds in git apply/merge, and
    // reading-then-writing across that window let a second promotion or an
    // abandon act on the same worktree. Every validation above this point leaves
    // the row untouched, so a rejected promotion stays promotable.
    {
        let connection = database.0.lock().map_err(|error| error.to_string())?;
        let claimed = connection
            .execute(
                "UPDATE runs SET state = 'promoting'
                 WHERE id = ?1 AND project_id = ?2 AND state = 'awaiting-promotion'",
                params![request.run_id, request.project_id],
            )
            .map_err(|error| error.to_string())?;
        if claimed == 0 {
            return Err(
                "This run is no longer awaiting promotion. Reload the room and try again."
                    .to_owned(),
            );
        }
    }
    update_run(
        database,
        &request.run_id,
        "promoting",
        None,
        review_count,
        revision_count,
        None,
        context_bytes,
        None,
        false,
    )?;
    emit_event(
        app,
        &request.run_id,
        "phase",
        "promote",
        "promoting",
        None,
        "Promoting verified work",
        "The approved delta is being checked against the attached checkout.",
        Some(context_bytes),
    );
    let files = changed_files(&worktree, &snapshot_head);
    let promotion = match promote_worktree(&base_repository, &isolation, &managed_root) {
        Ok(result) => result,
        Err(error) => {
            let reason = format!("Safe promotion was blocked: {error}");
            final_failure(
                app,
                database,
                &request.project_id,
                &request.run_id,
                "waiting",
                &reason,
                Some(&worktree),
                review_count,
                revision_count,
                context_bytes,
            )?;
            return Err(reason);
        }
    };
    let body = match promotion.mode {
        PromotionMode::FastForward => {
            "Human-approved verified work was promoted to the attached branch."
        }
        PromotionMode::WorkingTree => {
            "Human-approved verified work was applied to the existing uncommitted checkout."
        }
    };
    persist_message(
        database,
        &request.project_id,
        &request.run_id,
        "system",
        "status",
        body,
        &files,
        &[],
        promotion.cleanup_warning.as_deref(),
    )?;
    update_run(
        database,
        &request.run_id,
        "complete",
        None,
        review_count,
        revision_count,
        None,
        context_bytes,
        None,
        true,
    )?;
    if promotion.cleanup_warning.is_none() {
        database
            .0
            .lock()
            .map_err(|error| error.to_string())?
            .execute(
                "UPDATE runs SET worktree_path = NULL WHERE id = ?1 AND project_id = ?2",
                params![request.run_id, request.project_id],
            )
            .map_err(|error| error.to_string())?;
    }
    emit_event(
        app,
        &request.run_id,
        "complete",
        "complete",
        "complete",
        None,
        "Run complete",
        body,
        Some(context_bytes),
    );
    notify(app, "Agent Room complete", body);
    Ok(PromotionActionResult {
        promoted: true,
        cleanup_warning: promotion.cleanup_warning,
    })
}
