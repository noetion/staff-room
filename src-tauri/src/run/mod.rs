mod context;
mod failures;
mod process;
mod verification;

pub(crate) use context::*;
pub(crate) use failures::*;
pub(crate) use process::*;
pub(crate) use verification::*;

use crate::*;

pub(crate) async fn execute_room_run(
    app: AppHandle,
    database: &Database,
    runtime: &RuntimeState,
    request: StartRunRequest,
) -> Result<StartRunResult, String> {
    let (cancel_sender, cancel_receiver) = watch::channel(false);
    runtime.cancellations.lock().await.insert(
        request.run_id.clone(),
        CancellationEntry {
            project_id: request.project_id.clone(),
            sender: cancel_sender,
        },
    );
    let base_repository = project_repository(database, &request.project_id)?;

    let participants = participants_with_connections(
        database,
        &request.project_id,
        cached_participants(runtime, false).await?,
    )?;
    let ready = participants
        .iter()
        .filter(|participant| {
            participant.connection_status == "connected"
                && participant.installed
                && !matches!(
                    participant.capabilities.autonomy_mode.as_str(),
                    "manual" | "unavailable"
                )
        })
        .collect::<Vec<_>>();
    if ready.is_empty() {
        return Err("No installed coding-agent CLI proves a safe unattended mode.".to_owned());
    }

    type RecoveryRow = (
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        u32,
    );
    let existing = {
        let connection = database.0.lock().map_err(|error| error.to_string())?;
        connection
            .query_row(
                "SELECT objective, state, worktree_path, branch, base_head, base_branch,
                        snapshot_head, isolation_kind, workspace_fingerprint, writer,
                        native_session_id, recovery_count
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
                        row.get(7)?,
                        row.get(8)?,
                        row.get(9)?,
                        row.get(10)?,
                        row.get::<_, i64>(11)?.max(0) as u32,
                    ))
                },
            )
            .optional()
            .map_err(|error| error.to_string())?
    };
    let is_recovery = existing.is_some();
    let requested_builder = existing
        .as_ref()
        .and_then(|row: &RecoveryRow| row.9.as_deref())
        .or(request.requested_agent.as_deref());
    let builder = if let Some(requested) = requested_builder {
        ready
            .iter()
            .find(|participant| participant.kind == requested)
            .copied()
            .ok_or_else(|| {
                format!("The requested {requested} participant cannot run unattended.")
            })?
    } else {
        ready[0]
    };
    let reviewer = ready
        .iter()
        .find(|participant| participant.kind != builder.kind)
        .copied()
        .unwrap_or(builder);
    let degraded_review = reviewer.kind == builder.kind;
    let builder_profile = provider_profile(database, &request.project_id, &builder.kind, "build")?;
    let reviewer_profile =
        provider_profile(database, &request.project_id, &reviewer.kind, "review")?;
    let (isolation, recovery_session, recovery_count) = if let Some((
        stored_objective,
        state,
        worktree_path,
        stored_branch,
        stored_base_head,
        stored_base_branch,
        stored_snapshot_head,
        stored_isolation_kind,
        stored_fingerprint,
        _,
        session,
        prior_recovery_count,
    )) = existing
    {
        if stored_objective != request.objective {
            runtime.cancellations.lock().await.remove(&request.run_id);
            return Err("A preserved run can only resume its original objective.".to_owned());
        }
        if !matches!(state.as_str(), "waiting" | "failed" | "stopped") {
            runtime.cancellations.lock().await.remove(&request.run_id);
            return Err(format!("Run state `{state}` cannot be resumed."));
        }
        if prior_recovery_count >= MAX_RECOVERY_ATTEMPTS {
            runtime.cancellations.lock().await.remove(&request.run_id);
            return Err(format!(
                "This run has exhausted its {MAX_RECOVERY_ATTEMPTS} recovery attempts."
            ));
        }
        let path = PathBuf::from(worktree_path);
        if !path.is_dir() {
            runtime.cancellations.lock().await.remove(&request.run_id);
            return Err("The preserved worktree no longer exists.".to_owned());
        }
        let stored_base_branch = match stored_base_branch {
            Some(value) => value,
            None => {
                runtime.cancellations.lock().await.remove(&request.run_id);
                return Err(
                    "This preserved run predates branch-identity safety. Start a new Ship run so Agent Room can prove the target branch."
                        .to_owned(),
                );
            }
        };
        (
            IsolationContext {
                worktree: path,
                branch: stored_branch,
                base_branch: stored_base_branch,
                snapshot_head: stored_snapshot_head.unwrap_or_else(|| stored_base_head.clone()),
                base_head: stored_base_head,
                isolation_kind: stored_isolation_kind,
                workspace_fingerprint: stored_fingerprint,
            },
            session,
            prior_recovery_count + 1,
        )
    } else {
        let worktree_result = create_worktree(&app, &base_repository, &request.run_id);
        let isolation = match worktree_result {
            Ok(result) => result,
            Err(error) => {
                runtime.cancellations.lock().await.remove(&request.run_id);
                return Err(error);
            }
        };
        (isolation, None, 0)
    };
    let worktree = isolation.worktree.clone();
    let branch = isolation.branch.clone();
    let base_branch = isolation.base_branch.clone();
    let base_head = isolation.base_head.clone();
    let snapshot_head = isolation.snapshot_head.clone();
    let isolation_kind = isolation.isolation_kind.clone();
    let workspace_fingerprint = isolation.workspace_fingerprint.clone();
    let managed_root = app
        .path()
        .app_cache_dir()
        .map_err(|error| error.to_string())?
        .join("worktrees");
    let artifact_dir = run_artifact_directory(&app, &request.run_id)?;
    let (instructions, instruction_files) = repository_instructions(&worktree, &[]);
    let selected_skills = select_project_skills(&worktree, &request.objective);
    let skill_files = selected_skills
        .iter()
        .map(|skill| skill.relative_path.clone())
        .collect::<Vec<_>>();
    let selected_skill_context = skill_context(&selected_skills);

    if is_recovery {
        let connection = database.0.lock().map_err(|error| error.to_string())?;
        connection
            .execute(
                "UPDATE runs SET state = 'working', current_owner = ?1, writer = ?1,
                   reviewer = ?2, review_count = 0, revision_count = 0,
                   degraded_review = ?3, stop_reason = NULL, finished_at = NULL,
                   artifact_path = ?4, instruction_files_json = ?5,
                   skill_files_json = ?6, recovery_count = ?7
                 WHERE id = ?8",
                params![
                    builder.kind,
                    reviewer.kind,
                    degraded_review,
                    artifact_dir.to_string_lossy(),
                    serde_json::to_string(&instruction_files).unwrap_or_else(|_| "[]".to_owned()),
                    serde_json::to_string(&skill_files).unwrap_or_else(|_| "[]".to_owned()),
                    recovery_count,
                    request.run_id
                ],
            )
            .map_err(|error| error.to_string())?;
    } else {
        let connection = database.0.lock().map_err(|error| error.to_string())?;
        let transaction = connection
            .unchecked_transaction()
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "INSERT INTO runs
                 (id, project_id, objective, state, current_owner, writer, reviewer,
                  worktree_path, branch, base_head, base_branch, snapshot_head,
                  isolation_kind, workspace_fingerprint, degraded_review, artifact_path,
                  instruction_files_json, skill_files_json, recovery_count)
                 VALUES (?1, ?2, ?3, 'working', ?4, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                         ?11, ?12, ?13, ?14, ?15, ?16, 0)",
                params![
                    request.run_id,
                    request.project_id,
                    request.objective,
                    builder.kind,
                    reviewer.kind,
                    worktree.to_string_lossy(),
                    branch,
                    base_head,
                    base_branch,
                    snapshot_head,
                    isolation_kind,
                    workspace_fingerprint,
                    degraded_review,
                    artifact_dir.to_string_lossy(),
                    serde_json::to_string(&instruction_files).unwrap_or_else(|_| "[]".to_owned()),
                    serde_json::to_string(&skill_files).unwrap_or_else(|_| "[]".to_owned())
                ],
            )
            .map_err(|error| error.to_string())?;
        transaction.commit().map_err(|error| error.to_string())?;
    };
    if !is_recovery {
        persist_message(
            database,
            &request.project_id,
            &request.run_id,
            "human",
            "human",
            &request.objective,
            &[],
            &[],
            None,
        )?;
    }
    persist_message(
        database,
        &request.project_id,
        &request.run_id,
        "system",
        "status",
        &format!(
            "{} isolated run on `{branch}` from `{base_branch}`.{}",
            if is_recovery {
                "Resumed the"
            } else {
                "Created an"
            },
            if isolation_kind == "snapshot-clone" {
                " Current uncommitted files were captured without committing or stashing the attached checkout."
            } else {
                ""
            }
        ),
        &[],
        &[],
        Some(&format!(
            "Managed workspace: {}\nIsolation: {}\nArtifacts: {}\nInstructions: {}\nSelected skills: {}",
            worktree.to_string_lossy(),
            if isolation_kind == "snapshot-clone" {
                "dirty-checkout snapshot"
            } else {
                "Git worktree"
            },
            artifact_dir.to_string_lossy(),
            instruction_files.len(),
            skill_files.len()
        )),
    )?;

    emit_event(
        &app,
        &request.run_id,
        "phase",
        "prepare",
        "working",
        Some(&builder.kind),
        "Isolation ready",
        &format!(
            "{} will build on {branch}.{}",
            builder.name,
            if isolation_kind == "snapshot-clone" {
                " Existing uncommitted work is included and remains untouched in the attached checkout."
            } else {
                ""
            }
        ),
        None,
    );

    let build_started = Instant::now();
    let memory = project_memory(&worktree);
    let assignment = format!(
        "{} the objective completely in this managed worktree.\n\
         Work autonomously within the repository. Read every selected skill before acting and follow the hierarchical instruction files that govern each file you touch.\n\
         Run focused checks when useful. Do not ask the user to relay information to another agent.\n\
         {}\n\n{}",
        if is_recovery {
            "Recover the preserved work and finish"
        } else {
            "Implement"
        },
        if is_recovery {
            "Inspect the existing branch, prior changes, and failure evidence before continuing."
        } else {
            "Preserve unrelated repository work."
        },
        handoff_contract(Phase::Build)
    );
    let room_context = if objective_needs_room_context(&request.objective) {
        recent_chat_handoff(
            database,
            &request.project_id,
            &builder.kind,
            0,
            Some(&request.run_id),
        )?
    } else {
        None
    };
    let mut build_sections = vec![
        ("Objective", request.objective.clone()),
        ("Assignment", assignment),
        ("Repository instructions", instructions.clone()),
        ("Selected project skills", selected_skill_context.clone()),
        ("Project memory", memory.clone()),
        (
            "Run limits",
            format!(
                "One build, one review, at most one revision, and one final review. Provider timeout: {} minutes. Idle-output timeout: {} minutes. Recovery attempt: {} of {}.",
                PROCESS_TIMEOUT_SECONDS / 60,
                PROCESS_IDLE_TIMEOUT_SECONDS / 60,
                recovery_count,
                MAX_RECOVERY_ATTEMPTS
            ),
        ),
    ];
    if let Some(context) = room_context {
        build_sections.push((
            "Recent room context",
            format!(
                "The objective refers to the recent conversation. Resolve pronouns from this bounded handoff and implement the previously described change:\n{context}"
            ),
        ));
    }
    let (build_packet, build_context_bytes) =
        assemble_packet(Phase::Build.context_budget_bytes(), &build_sections);
    let mut max_context_bytes = build_context_bytes;
    emit_event(
        &app,
        &request.run_id,
        "phase",
        "build",
        "working",
        Some(&builder.kind),
        &format!("{} is building", builder.name),
        &format!(
            "Compact context: {} KiB, approximately {} tokens.",
            build_context_bytes.div_ceil(1024),
            build_context_bytes / 4
        ),
        Some(build_context_bytes),
    );
    persist_activation(
        database,
        &request.run_id,
        Phase::Build,
        &builder.kind,
        "running",
        None,
        build_context_bytes,
        None,
    )?;
    let build_output_path = artifact_dir.join("build.final.txt");
    let build_preflight_ms = build_started.elapsed().as_millis() as u64;
    let build_result = invoke_provider(
        &app,
        &request.run_id,
        builder,
        Phase::Build,
        ProviderMode::Ship,
        &build_packet,
        &worktree,
        recovery_session.as_deref(),
        builder_profile.model.as_deref(),
        builder_profile.effort.as_deref(),
        &build_output_path,
        cancel_receiver.clone(),
    )
    .await?;
    let build_total_ms = build_started.elapsed().as_millis() as u64;
    persist_handoff(
        database,
        &request.run_id,
        Phase::Build,
        &builder.kind,
        build_result.handoff.as_ref(),
    )?;
    persist_activation(
        database,
        &request.run_id,
        Phase::Build,
        &builder.kind,
        match build_result
            .handoff
            .as_ref()
            .map(|handoff| handoff.status.as_str())
        {
            Some("completed") => "complete",
            Some("blocked") => "blocked",
            _ => "failed",
        },
        build_result.session_id.as_deref(),
        build_context_bytes,
        Some(&truncate_utf8(&build_result.summary, 8 * 1024)),
    )?;
    persist_receipt(
        database,
        &request.run_id,
        Phase::Build,
        builder,
        &builder_profile,
        ReceiptMetrics {
            context_bytes: build_context_bytes,
            packet_bytes_saved: 0,
            preflight_ms: build_preflight_ms,
            total_ms: build_total_ms,
        },
        &build_result,
    )?;
    save_provider_session(
        database,
        &request.project_id,
        &builder.kind,
        build_result.session_id.as_deref(),
    )?;

    if build_result.stopped || build_result.timed_out || !build_result.success {
        let state = if build_result.stopped {
            "stopped"
        } else if build_result
            .handoff
            .as_ref()
            .is_some_and(|handoff| handoff.status == "blocked")
        {
            "waiting"
        } else {
            "failed"
        };
        let reason = if build_result.stopped {
            "Stopped by you.".to_owned()
        } else if build_result.timed_out {
            "The builder exceeded the 20 minute phase limit.".to_owned()
        } else if build_result.idle_timed_out {
            "The builder produced no output for 5 minutes and was stopped.".to_owned()
        } else if let Some(handoff) = build_result.handoff.as_ref() {
            handoff_attention_reason(handoff)
        } else {
            provider_failure_reason(&builder.name, "the build", &build_result)
        };
        final_failure(
            &app,
            database,
            &request.project_id,
            &request.run_id,
            state,
            &reason,
            Some(&worktree),
            0,
            0,
            max_context_bytes,
        )?;
        runtime.cancellations.lock().await.remove(&request.run_id);
        return Ok(StartRunResult {
            run_id: request.run_id,
            state: state.to_owned(),
            summary: build_result.summary,
            builder: builder.kind.clone(),
            reviewer: reviewer.kind.clone(),
            degraded_review,
            session_id: build_result.session_id,
            changed_files: changed_files(&worktree, &snapshot_head),
            git_status: git_status(&worktree),
            verification: vec![],
            stopped: state == "stopped",
            promoted: false,
            worktree_path: Some(worktree.to_string_lossy().into_owned()),
            branch,
            context_bytes: max_context_bytes,
            artifact_path: Some(artifact_dir.to_string_lossy().into_owned()),
            instruction_files: instruction_files.clone(),
            skill_files: skill_files.clone(),
            recovery_count,
            attention_reason: Some(reason),
        });
    }

    commit_managed_changes(&worktree, &request.objective)?;
    let mut files = changed_files(&worktree, &snapshot_head);
    persist_message(
        database,
        &request.project_id,
        &request.run_id,
        &builder.kind,
        "agent",
        if build_result.summary.trim().is_empty() {
            "Builder completed without a textual summary."
        } else {
            &build_result.summary
        },
        &files,
        &[],
        Some(&provider_log_note(&build_result)),
    )?;

    update_run(
        database,
        &request.run_id,
        "verifying",
        None,
        0,
        0,
        build_result.session_id.as_deref(),
        max_context_bytes,
        None,
        false,
    )?;
    let mut verification = run_verification(
        &app,
        database,
        &request.project_id,
        &request.run_id,
        &worktree,
        cancel_receiver.clone(),
    )
    .await;
    if *cancel_receiver.borrow() {
        final_failure(
            &app,
            database,
            &request.project_id,
            &request.run_id,
            "stopped",
            "Stopped by you during verification.",
            Some(&worktree),
            0,
            0,
            max_context_bytes,
        )?;
        runtime.cancellations.lock().await.remove(&request.run_id);
        return Ok(StartRunResult {
            run_id: request.run_id,
            state: "stopped".to_owned(),
            summary: build_result.summary,
            builder: builder.kind.clone(),
            reviewer: reviewer.kind.clone(),
            degraded_review,
            session_id: build_result.session_id,
            changed_files: files,
            git_status: git_status(&worktree),
            verification,
            stopped: true,
            promoted: false,
            worktree_path: Some(worktree.to_string_lossy().into_owned()),
            branch,
            context_bytes: max_context_bytes,
            artifact_path: Some(artifact_dir.to_string_lossy().into_owned()),
            instruction_files: instruction_files.clone(),
            skill_files: skill_files.clone(),
            recovery_count,
            attention_reason: Some("Stopped by you during verification.".to_owned()),
        });
    }
    persist_message(
        database,
        &request.project_id,
        &request.run_id,
        "system",
        "evidence",
        if verification_passed(&verification) {
            "Detected verification completed without failures."
        } else {
            "One or more verification commands failed."
        },
        &files,
        &verification,
        Some(&git_status(&worktree)),
    )?;

    if verification_is_unconfigured(&verification) {
        let reason = verification[0].detail.clone();
        final_failure(
            &app,
            database,
            &request.project_id,
            &request.run_id,
            "waiting",
            &reason,
            Some(&worktree),
            0,
            0,
            max_context_bytes,
        )?;
        runtime.cancellations.lock().await.remove(&request.run_id);
        return Ok(StartRunResult {
            run_id: request.run_id,
            state: "waiting".to_owned(),
            summary: build_result.summary,
            builder: builder.kind.clone(),
            reviewer: reviewer.kind.clone(),
            degraded_review,
            session_id: build_result.session_id,
            changed_files: files,
            git_status: git_status(&worktree),
            verification,
            stopped: false,
            promoted: false,
            worktree_path: Some(worktree.to_string_lossy().into_owned()),
            branch,
            context_bytes: max_context_bytes,
            artifact_path: Some(artifact_dir.to_string_lossy().into_owned()),
            instruction_files: instruction_files.clone(),
            skill_files: skill_files.clone(),
            recovery_count,
            attention_reason: Some(reason),
        });
    }

    let review_assignment = format!(
        "Review the implementation against the objective and repository evidence.\n\
         You are read-only. Do not edit files.\n\
         Classify only correctness, regression, security, or missing-test findings that materially affect completion.\n\n{}",
        handoff_contract(Phase::Review)
    );
    let review_handoff = format!(
        "Builder: {}\n{}\n\nChanged files:\n{}\n\nVerification:\n{}",
        builder.name,
        truncate_utf8(&build_result.summary, SOURCE_BUDGET_BYTES / 2),
        files.join("\n"),
        verification_summary(&verification)
    );
    let review_started = Instant::now();
    let (review_instructions, _) = repository_instructions(&worktree, &files);
    let (review_packet, review_context_bytes) = assemble_packet(
        Phase::Review.context_budget_bytes(),
        &[
            ("Objective", request.objective.clone()),
            ("Review assignment", review_assignment),
            ("Applicable repository instructions", review_instructions),
            ("Selected project skills", selected_skill_context.clone()),
            ("Project memory", memory.clone()),
            ("Builder handoff", review_handoff),
            (
                "Focused repository delta",
                diff_evidence(&worktree, &snapshot_head),
            ),
        ],
    );
    max_context_bytes = max_context_bytes.max(review_context_bytes);
    update_run(
        database,
        &request.run_id,
        "reviewing",
        Some(&reviewer.kind),
        1,
        0,
        build_result.session_id.as_deref(),
        max_context_bytes,
        None,
        false,
    )?;
    emit_event(
        &app,
        &request.run_id,
        "phase",
        "review",
        "reviewing",
        Some(&reviewer.kind),
        if degraded_review {
            "Same-provider review"
        } else {
            "Independent review"
        },
        &format!(
            "{} is reviewing {} KiB of selected evidence.",
            reviewer.name,
            review_context_bytes.div_ceil(1024)
        ),
        Some(review_context_bytes),
    );
    persist_activation(
        database,
        &request.run_id,
        Phase::Review,
        &reviewer.kind,
        "running",
        None,
        review_context_bytes,
        None,
    )?;
    let review_output_path = artifact_dir.join("review.final.txt");
    let review_preflight_ms = review_started.elapsed().as_millis() as u64;
    let review_guard = review_mutation_guard(&worktree)?;
    let review_result = invoke_provider(
        &app,
        &request.run_id,
        reviewer,
        Phase::Review,
        ProviderMode::Review,
        &review_packet,
        &worktree,
        None,
        reviewer_profile.model.as_deref(),
        reviewer_profile.effort.as_deref(),
        &review_output_path,
        cancel_receiver.clone(),
    )
    .await?;
    let review_total_ms = review_started.elapsed().as_millis() as u64;
    if review_mutation_guard(&worktree)? != review_guard {
        let reason = "The reviewer modified the worktree.".to_owned();
        final_failure(
            &app,
            database,
            &request.project_id,
            &request.run_id,
            "failed",
            &reason,
            Some(&worktree),
            1,
            0,
            max_context_bytes,
        )?;
        runtime.cancellations.lock().await.remove(&request.run_id);
        return Ok(StartRunResult {
            run_id: request.run_id,
            state: "failed".to_owned(),
            summary: review_result.summary,
            builder: builder.kind.clone(),
            reviewer: reviewer.kind.clone(),
            degraded_review,
            session_id: build_result.session_id,
            changed_files: files,
            git_status: git_status(&worktree),
            verification,
            stopped: false,
            promoted: false,
            worktree_path: Some(worktree.to_string_lossy().into_owned()),
            branch,
            context_bytes: max_context_bytes,
            artifact_path: Some(artifact_dir.to_string_lossy().into_owned()),
            instruction_files: instruction_files.clone(),
            skill_files: skill_files.clone(),
            recovery_count,
            attention_reason: Some(reason),
        });
    }
    persist_handoff(
        database,
        &request.run_id,
        Phase::Review,
        &reviewer.kind,
        review_result.handoff.as_ref(),
    )?;
    persist_activation(
        database,
        &request.run_id,
        Phase::Review,
        &reviewer.kind,
        if review_result.success {
            "complete"
        } else {
            "failed"
        },
        review_result.session_id.as_deref(),
        review_context_bytes,
        Some(&truncate_utf8(&review_result.summary, 8 * 1024)),
    )?;
    persist_receipt(
        database,
        &request.run_id,
        Phase::Review,
        reviewer,
        &reviewer_profile,
        ReceiptMetrics {
            context_bytes: review_context_bytes,
            packet_bytes_saved: 0,
            preflight_ms: review_preflight_ms,
            total_ms: review_total_ms,
        },
        &review_result,
    )?;
    save_provider_session(
        database,
        &request.project_id,
        &reviewer.kind,
        review_result.session_id.as_deref(),
    )?;
    persist_message(
        database,
        &request.project_id,
        &request.run_id,
        &reviewer.kind,
        "review",
        &review_result.summary,
        &files,
        &verification,
        Some(&format!(
            "{}\n{}",
            if degraded_review {
                "A different CLI was unavailable, so this used a fresh session of the builder provider."
            } else {
                "Independent provider review."
            },
            provider_log_note(&review_result)
        )),
    )?;

    if review_result.stopped || !review_result.success {
        let state = if review_result.stopped {
            "stopped"
        } else {
            "failed"
        };
        let reason = if review_result.stopped {
            "Stopped by you during review.".to_owned()
        } else if review_result.timed_out {
            "The reviewer exceeded the 20 minute phase limit.".to_owned()
        } else if review_result.idle_timed_out {
            "The reviewer produced no output for 5 minutes and was stopped.".to_owned()
        } else {
            provider_failure_reason(&reviewer.name, "the review", &review_result)
        };
        final_failure(
            &app,
            database,
            &request.project_id,
            &request.run_id,
            state,
            &reason,
            Some(&worktree),
            1,
            0,
            max_context_bytes,
        )?;
        runtime.cancellations.lock().await.remove(&request.run_id);
        return Ok(StartRunResult {
            run_id: request.run_id,
            state: state.to_owned(),
            summary: review_result.summary,
            builder: builder.kind.clone(),
            reviewer: reviewer.kind.clone(),
            degraded_review,
            session_id: build_result.session_id,
            changed_files: files,
            git_status: git_status(&worktree),
            verification,
            stopped: state == "stopped",
            promoted: false,
            worktree_path: Some(worktree.to_string_lossy().into_owned()),
            branch,
            context_bytes: max_context_bytes,
            artifact_path: Some(artifact_dir.to_string_lossy().into_owned()),
            instruction_files: instruction_files.clone(),
            skill_files: skill_files.clone(),
            recovery_count,
            attention_reason: Some(reason),
        });
    }

    let initial_approved = review_handoff_decision(&review_result);
    let needs_revision = !verification_passed(&verification) || initial_approved != Some(true);
    let mut review_count = 1;
    let mut revision_count = 0;
    let mut final_review_summary = review_result.summary.clone();
    let mut final_approved = initial_approved;

    if needs_revision {
        revision_count = 1;
        let revision_started = Instant::now();
        let revision_assignment = format!(
            "Revise the implementation once to address the review and verification evidence.\n\
             Preserve correct existing work. Do not broaden scope.\n\
             Read the selected skills and applicable instruction files before editing.\n\n\
             Review findings:\n{}\n\nVerification:\n{}\n\n{}",
            truncate_utf8(&review_result.summary, SOURCE_BUDGET_BYTES),
            verification_summary(&verification),
            handoff_contract(Phase::Revise)
        );
        let revision_evidence = format!(
            "Review findings:\n{}\n\nVerification:\n{}",
            truncate_utf8(&review_result.summary, SOURCE_BUDGET_BYTES),
            verification_summary(&verification),
        );
        let (revision_instructions, _) = repository_instructions(&worktree, &files);
        let (full_revision_packet, full_revision_context_bytes) = assemble_packet(Phase::Build.context_budget_bytes(), &[
            ("Objective", request.objective.clone()),
            ("Revision assignment", revision_assignment),
            ("Applicable repository instructions", revision_instructions),
            ("Selected project skills", selected_skill_context.clone()),
            ("Current changed files", files.join("\n")),
            ("Focused delta", diff_evidence(&worktree, &snapshot_head)),
            (
                "Run limit",
                "This is the only automatic revision. Resolve all material findings before stopping."
                    .to_owned(),
            ),
        ]);
        let revision_resume_requested =
            builder.capabilities.exact_resume && build_result.session_id.is_some();
        let (revision_packet, revision_context_bytes) = if revision_resume_requested {
            assemble_packet(
                Phase::Revise.context_budget_bytes(),
                &[("Revision evidence", revision_evidence)],
            )
        } else {
            (full_revision_packet.clone(), full_revision_context_bytes)
        };
        max_context_bytes = max_context_bytes.max(revision_context_bytes);
        update_run(
            database,
            &request.run_id,
            "revising",
            Some(&builder.kind),
            review_count,
            revision_count,
            build_result.session_id.as_deref(),
            max_context_bytes,
            None,
            false,
        )?;
        emit_event(
            &app,
            &request.run_id,
            "phase",
            "revise",
            "revising",
            Some(&builder.kind),
            &format!("{} is revising", builder.name),
            "The original builder session is resumed when the provider supports it.",
            Some(revision_context_bytes),
        );
        let revision_output_path = artifact_dir.join("revision.final.txt");
        let revision_preflight_ms = revision_started.elapsed().as_millis() as u64;
        let mut revision_result = invoke_provider(
            &app,
            &request.run_id,
            builder,
            Phase::Revise,
            ProviderMode::Ship,
            &revision_packet,
            &worktree,
            build_result.session_id.as_deref(),
            builder_profile.model.as_deref(),
            builder_profile.effort.as_deref(),
            &revision_output_path,
            cancel_receiver.clone(),
        )
        .await?;
        let mut recorded_revision_context_bytes = revision_context_bytes;
        if revision_resume_requested && !revision_result.session_resumed {
            emit_event(
                &app,
                &request.run_id,
                "resume",
                "revise",
                "working",
                Some(&builder.kind),
                "Resume unavailable",
                "The provider reported a new or unknown session, so Agent Room resent the full revision packet.",
                Some(full_revision_context_bytes),
            );
            revision_result = invoke_provider(
                &app,
                &request.run_id,
                builder,
                Phase::Revise,
                ProviderMode::Ship,
                &full_revision_packet,
                &worktree,
                None,
                builder_profile.model.as_deref(),
                builder_profile.effort.as_deref(),
                &revision_output_path,
                cancel_receiver.clone(),
            )
            .await?;
            recorded_revision_context_bytes = full_revision_context_bytes;
        }
        let revision_total_ms = revision_started.elapsed().as_millis() as u64;
        persist_handoff(
            database,
            &request.run_id,
            Phase::Revise,
            &builder.kind,
            revision_result.handoff.as_ref(),
        )?;
        persist_activation(
            database,
            &request.run_id,
            Phase::Revise,
            &builder.kind,
            if revision_result.success {
                "complete"
            } else {
                "failed"
            },
            revision_result.session_id.as_deref(),
            recorded_revision_context_bytes,
            Some(&truncate_utf8(&revision_result.summary, 8 * 1024)),
        )?;
        persist_receipt(
            database,
            &request.run_id,
            Phase::Revise,
            builder,
            &builder_profile,
            ReceiptMetrics {
                context_bytes: recorded_revision_context_bytes,
                packet_bytes_saved: if revision_result.session_resumed {
                    full_revision_context_bytes.saturating_sub(recorded_revision_context_bytes)
                } else {
                    0
                },
                preflight_ms: revision_preflight_ms,
                total_ms: revision_total_ms,
            },
            &revision_result,
        )?;
        if !revision_result.success {
            let state = if revision_result.stopped {
                "stopped"
            } else {
                "failed"
            };
            let reason = if revision_result.stopped {
                "Stopped by you during revision.".to_owned()
            } else if revision_result.timed_out {
                "The revision exceeded the 20 minute phase limit.".to_owned()
            } else if revision_result.idle_timed_out {
                "The revision produced no output for 5 minutes and was stopped.".to_owned()
            } else {
                provider_failure_reason(&builder.name, "the bounded revision", &revision_result)
            };
            final_failure(
                &app,
                database,
                &request.project_id,
                &request.run_id,
                state,
                &reason,
                Some(&worktree),
                review_count,
                revision_count,
                max_context_bytes,
            )?;
            runtime.cancellations.lock().await.remove(&request.run_id);
            return Ok(StartRunResult {
                run_id: request.run_id,
                state: state.to_owned(),
                summary: revision_result.summary,
                builder: builder.kind.clone(),
                reviewer: reviewer.kind.clone(),
                degraded_review,
                session_id: build_result.session_id,
                changed_files: changed_files(&worktree, &snapshot_head),
                git_status: git_status(&worktree),
                verification,
                stopped: state == "stopped",
                promoted: false,
                worktree_path: Some(worktree.to_string_lossy().into_owned()),
                branch,
                context_bytes: max_context_bytes,
                artifact_path: Some(artifact_dir.to_string_lossy().into_owned()),
                instruction_files: instruction_files.clone(),
                skill_files: skill_files.clone(),
                recovery_count,
                attention_reason: Some(reason),
            });
        }
        commit_managed_changes(&worktree, &format!("Revise {}", request.objective))?;
        files = changed_files(&worktree, &snapshot_head);
        persist_message(
            database,
            &request.project_id,
            &request.run_id,
            &builder.kind,
            "agent",
            &revision_result.summary,
            &files,
            &[],
            Some(&format!(
                "Bounded revision 1 of 1.\n{}",
                provider_log_note(&revision_result)
            )),
        )?;
        verification = run_verification(
            &app,
            database,
            &request.project_id,
            &request.run_id,
            &worktree,
            cancel_receiver.clone(),
        )
        .await;
        persist_message(
            database,
            &request.project_id,
            &request.run_id,
            "system",
            "evidence",
            if verification_passed(&verification) {
                "Post-revision verification passed."
            } else {
                "Post-revision verification still has failures."
            },
            &files,
            &verification,
            Some(&git_status(&worktree)),
        )?;

        review_count = 2;
        let final_review_started = Instant::now();
        let final_assignment = format!(
            "Perform the final read-only review after one bounded revision.\n\
             Confirm whether the original material findings and verification failures are resolved.\n\
             Do not request optional improvements.\n\n{}",
            handoff_contract(Phase::FinalReview)
        );
        let final_evidence = format!(
            "Post-revision verification:\n{}\n\nCurrent focused delta:\n{}",
            verification_summary(&verification),
            diff_evidence(&worktree, &snapshot_head),
        );
        let (full_final_packet, full_final_context_bytes) = assemble_packet(
            Phase::Review.context_budget_bytes(),
            &[
                ("Objective", request.objective.clone()),
                ("Final review assignment", final_assignment),
                (
                    "Prior findings",
                    truncate_utf8(&review_result.summary, SOURCE_BUDGET_BYTES),
                ),
                (
                    "Post-revision verification",
                    verification_summary(&verification),
                ),
                (
                    "Current focused delta",
                    diff_evidence(&worktree, &snapshot_head),
                ),
            ],
        );
        let final_resume_requested =
            reviewer.capabilities.exact_resume && review_result.session_id.is_some();
        let (final_packet, final_context_bytes) = if final_resume_requested {
            assemble_packet(
                Phase::FinalReview.context_budget_bytes(),
                &[("Final review evidence", final_evidence)],
            )
        } else {
            (full_final_packet.clone(), full_final_context_bytes)
        };
        max_context_bytes = max_context_bytes.max(final_context_bytes);
        update_run(
            database,
            &request.run_id,
            "reviewing",
            Some(&reviewer.kind),
            review_count,
            revision_count,
            build_result.session_id.as_deref(),
            max_context_bytes,
            None,
            false,
        )?;
        emit_event(
            &app,
            &request.run_id,
            "phase",
            "final-review",
            "reviewing",
            Some(&reviewer.kind),
            "Final review",
            "The reviewer receives the revised delta, prior findings, and current checks.",
            Some(final_context_bytes),
        );
        let final_output_path = artifact_dir.join("final-review.final.txt");
        let final_review_preflight_ms = final_review_started.elapsed().as_millis() as u64;
        let final_review_guard = review_mutation_guard(&worktree)?;
        let mut final_result = invoke_provider(
            &app,
            &request.run_id,
            reviewer,
            Phase::FinalReview,
            ProviderMode::Review,
            &final_packet,
            &worktree,
            review_result.session_id.as_deref(),
            reviewer_profile.model.as_deref(),
            reviewer_profile.effort.as_deref(),
            &final_output_path,
            cancel_receiver.clone(),
        )
        .await?;
        let mut recorded_final_context_bytes = final_context_bytes;
        if final_resume_requested && !final_result.session_resumed {
            emit_event(
                &app,
                &request.run_id,
                "resume",
                "final-review",
                "working",
                Some(&reviewer.kind),
                "Resume unavailable",
                "The provider reported a new or unknown session, so Agent Room resent the full final-review packet.",
                Some(full_final_context_bytes),
            );
            final_result = invoke_provider(
                &app,
                &request.run_id,
                reviewer,
                Phase::FinalReview,
                ProviderMode::Review,
                &full_final_packet,
                &worktree,
                None,
                reviewer_profile.model.as_deref(),
                reviewer_profile.effort.as_deref(),
                &final_output_path,
                cancel_receiver.clone(),
            )
            .await?;
            recorded_final_context_bytes = full_final_context_bytes;
        }
        let final_review_total_ms = final_review_started.elapsed().as_millis() as u64;
        if review_mutation_guard(&worktree)? != final_review_guard {
            let reason = "The reviewer modified the worktree.".to_owned();
            final_failure(
                &app,
                database,
                &request.project_id,
                &request.run_id,
                "failed",
                &reason,
                Some(&worktree),
                review_count,
                revision_count,
                max_context_bytes,
            )?;
            runtime.cancellations.lock().await.remove(&request.run_id);
            return Ok(StartRunResult {
                run_id: request.run_id,
                state: "failed".to_owned(),
                summary: final_result.summary,
                builder: builder.kind.clone(),
                reviewer: reviewer.kind.clone(),
                degraded_review,
                session_id: build_result.session_id,
                changed_files: files,
                git_status: git_status(&worktree),
                verification,
                stopped: false,
                promoted: false,
                worktree_path: Some(worktree.to_string_lossy().into_owned()),
                branch,
                context_bytes: max_context_bytes,
                artifact_path: Some(artifact_dir.to_string_lossy().into_owned()),
                instruction_files: instruction_files.clone(),
                skill_files: skill_files.clone(),
                recovery_count,
                attention_reason: Some(reason),
            });
        }
        final_review_summary = final_result.summary.clone();
        final_approved = review_handoff_decision(&final_result);
        persist_handoff(
            database,
            &request.run_id,
            Phase::FinalReview,
            &reviewer.kind,
            final_result.handoff.as_ref(),
        )?;
        persist_activation(
            database,
            &request.run_id,
            Phase::FinalReview,
            &reviewer.kind,
            if final_result.success {
                "complete"
            } else {
                "failed"
            },
            final_result.session_id.as_deref(),
            recorded_final_context_bytes,
            Some(&truncate_utf8(&final_result.summary, 8 * 1024)),
        )?;
        persist_receipt(
            database,
            &request.run_id,
            Phase::FinalReview,
            reviewer,
            &reviewer_profile,
            ReceiptMetrics {
                context_bytes: recorded_final_context_bytes,
                packet_bytes_saved: if final_result.session_resumed {
                    full_final_context_bytes.saturating_sub(recorded_final_context_bytes)
                } else {
                    0
                },
                preflight_ms: final_review_preflight_ms,
                total_ms: final_review_total_ms,
            },
            &final_result,
        )?;
        persist_message(
            database,
            &request.project_id,
            &request.run_id,
            &reviewer.kind,
            "review",
            &final_result.summary,
            &files,
            &verification,
            Some(&format!(
                "Final review 2 of 2.\n{}",
                provider_log_note(&final_result)
            )),
        )?;
        if !final_result.success {
            let reason = if final_result.stopped {
                "Stopped by you during final review.".to_owned()
            } else if final_result.timed_out {
                "The final review exceeded the 20 minute phase limit.".to_owned()
            } else if final_result.idle_timed_out {
                "The final review produced no output for 5 minutes and was stopped.".to_owned()
            } else {
                provider_failure_reason(&reviewer.name, "the final review", &final_result)
            };
            let state = if final_result.stopped {
                "stopped"
            } else {
                "failed"
            };
            final_failure(
                &app,
                database,
                &request.project_id,
                &request.run_id,
                state,
                &reason,
                Some(&worktree),
                review_count,
                revision_count,
                max_context_bytes,
            )?;
            runtime.cancellations.lock().await.remove(&request.run_id);
            return Ok(StartRunResult {
                run_id: request.run_id,
                state: state.to_owned(),
                summary: final_result.summary,
                builder: builder.kind.clone(),
                reviewer: reviewer.kind.clone(),
                degraded_review,
                session_id: build_result.session_id,
                changed_files: files,
                git_status: git_status(&worktree),
                verification,
                stopped: state == "stopped",
                promoted: false,
                worktree_path: Some(worktree.to_string_lossy().into_owned()),
                branch,
                context_bytes: max_context_bytes,
                artifact_path: Some(artifact_dir.to_string_lossy().into_owned()),
                instruction_files: instruction_files.clone(),
                skill_files: skill_files.clone(),
                recovery_count,
                attention_reason: Some(reason),
            });
        }
    }

    let approved = final_approved == Some(true);
    if !approved || !verification_passed(&verification) {
        let reason = if !verification_passed(&verification) {
            "Verification still fails after the single allowed revision."
        } else {
            "Final review did not provide an approval decision."
        };
        final_failure(
            &app,
            database,
            &request.project_id,
            &request.run_id,
            "waiting",
            reason,
            Some(&worktree),
            review_count,
            revision_count,
            max_context_bytes,
        )?;
        runtime.cancellations.lock().await.remove(&request.run_id);
        return Ok(StartRunResult {
            run_id: request.run_id,
            state: "waiting".to_owned(),
            summary: final_review_summary,
            builder: builder.kind.clone(),
            reviewer: reviewer.kind.clone(),
            degraded_review,
            session_id: build_result.session_id,
            changed_files: files,
            git_status: git_status(&worktree),
            verification,
            stopped: false,
            promoted: false,
            worktree_path: Some(worktree.to_string_lossy().into_owned()),
            branch,
            context_bytes: max_context_bytes,
            artifact_path: Some(artifact_dir.to_string_lossy().into_owned()),
            instruction_files: instruction_files.clone(),
            skill_files: skill_files.clone(),
            recovery_count,
            attention_reason: Some(reason.to_owned()),
        });
    }

    let has_changes = git_static(&worktree, &["rev-parse", "HEAD"])? != snapshot_head;
    if has_changes && !project_settings(database, &request.project_id)?.autonomous_ship_enabled {
        let reviewed_fingerprint = review_mutation_guard(&worktree)?;
        database
            .0
            .lock()
            .map_err(|error| error.to_string())?
            .execute(
                "UPDATE runs SET reviewed_fingerprint = ?1 WHERE id = ?2 AND project_id = ?3",
                params![reviewed_fingerprint, request.run_id, request.project_id],
            )
            .map_err(|error| error.to_string())?;
        let reason =
            "Verification and review passed. Inspect the evidence, then choose Promote or Abandon."
                .to_owned();
        update_run(
            database,
            &request.run_id,
            "awaiting-promotion",
            None,
            review_count,
            revision_count,
            build_result.session_id.as_deref(),
            max_context_bytes,
            Some(&reason),
            false,
        )?;
        persist_message(
            database,
            &request.project_id,
            &request.run_id,
            "system",
            "decision",
            "Verified work is ready for human promotion.",
            &files,
            &verification,
            Some(&reason),
        )?;
        emit_event(
            &app,
            &request.run_id,
            "attention",
            "promote",
            "awaiting-promotion",
            None,
            "Promotion approval required",
            &reason,
            Some(max_context_bytes),
        );
        runtime.cancellations.lock().await.remove(&request.run_id);
        return Ok(StartRunResult {
            run_id: request.run_id,
            state: "awaiting-promotion".to_owned(),
            summary: final_review_summary,
            builder: builder.kind.clone(),
            reviewer: reviewer.kind.clone(),
            degraded_review,
            session_id: build_result.session_id,
            changed_files: files,
            git_status: git_status(&worktree),
            verification,
            stopped: false,
            promoted: false,
            worktree_path: Some(worktree.to_string_lossy().into_owned()),
            branch,
            context_bytes: max_context_bytes,
            artifact_path: Some(artifact_dir.to_string_lossy().into_owned()),
            instruction_files,
            skill_files,
            recovery_count,
            attention_reason: Some(reason),
        });
    }

    if *cancel_receiver.borrow() {
        let reason =
            "Cancellation was requested before promotion. Verified work was preserved.".to_owned();
        final_failure(
            &app,
            database,
            &request.project_id,
            &request.run_id,
            "stopped",
            &reason,
            Some(&worktree),
            review_count,
            revision_count,
            max_context_bytes,
        )?;
        runtime.cancellations.lock().await.remove(&request.run_id);
        return Ok(StartRunResult {
            run_id: request.run_id,
            state: "stopped".to_owned(),
            summary: final_review_summary,
            builder: builder.kind.clone(),
            reviewer: reviewer.kind.clone(),
            degraded_review,
            session_id: build_result.session_id,
            changed_files: files,
            git_status: git_status(&worktree),
            verification,
            stopped: true,
            promoted: false,
            worktree_path: Some(worktree.to_string_lossy().into_owned()),
            branch,
            context_bytes: max_context_bytes,
            artifact_path: Some(artifact_dir.to_string_lossy().into_owned()),
            instruction_files,
            skill_files,
            recovery_count,
            attention_reason: Some(reason),
        });
    }

    update_run(
        database,
        &request.run_id,
        "promoting",
        None,
        review_count,
        revision_count,
        build_result.session_id.as_deref(),
        max_context_bytes,
        None,
        false,
    )?;
    emit_event(
        &app,
        &request.run_id,
        "phase",
        "promote",
        "promoting",
        None,
        "Promoting verified work",
        "Agent Room is checking that the base checkout has not changed.",
        Some(max_context_bytes),
    );
    let (promoted, promotion_mode, cleanup_warning) = if has_changes {
        match promote_worktree(&base_repository, &isolation, &managed_root) {
            Ok(result) => (true, Some(result.mode), result.cleanup_warning),
            Err(error) => {
                final_failure(
                    &app,
                    database,
                    &request.project_id,
                    &request.run_id,
                    "waiting",
                    &format!("Safe promotion was blocked: {error}"),
                    Some(&worktree),
                    review_count,
                    revision_count,
                    max_context_bytes,
                )?;
                runtime.cancellations.lock().await.remove(&request.run_id);
                return Ok(StartRunResult {
                    run_id: request.run_id,
                    state: "waiting".to_owned(),
                    summary: final_review_summary,
                    builder: builder.kind.clone(),
                    reviewer: reviewer.kind.clone(),
                    degraded_review,
                    session_id: build_result.session_id,
                    changed_files: files,
                    git_status: git_status(&worktree),
                    verification,
                    stopped: false,
                    promoted: false,
                    worktree_path: Some(worktree.to_string_lossy().into_owned()),
                    branch,
                    context_bytes: max_context_bytes,
                    artifact_path: Some(artifact_dir.to_string_lossy().into_owned()),
                    instruction_files: instruction_files.clone(),
                    skill_files: skill_files.clone(),
                    recovery_count,
                    attention_reason: Some(format!("Safe promotion was blocked: {error}")),
                });
            }
        }
    } else {
        (
            false,
            None,
            discard_isolation(&base_repository, &isolation, &managed_root),
        )
    };
    let completion_body = match promotion_mode {
        Some(PromotionMode::FastForward) => {
            "Verification and review passed. The managed branch was fast-forwarded into the base checkout."
        }
        Some(PromotionMode::WorkingTree) => {
            "Verification and review passed. Agent Room safely applied the verified delta to your existing uncommitted checkout without committing or stashing your work."
        }
        None => {
            "Verification and review passed. The objective intentionally produced no repository change."
        }
    };
    let mut completion_reasons = vec![if degraded_review {
        "Completed with a same-provider review downgrade.".to_owned()
    } else {
        "Completed with independent provider review.".to_owned()
    }];
    if let Some(warning) = cleanup_warning {
        completion_reasons.push(format!(
            "The verified result was applied, but temporary isolation cleanup needs attention: {warning}"
        ));
    }
    let completion_reason = completion_reasons.join("\n");

    persist_message(
        database,
        &request.project_id,
        &request.run_id,
        "system",
        "status",
        completion_body,
        &files,
        &verification,
        Some(&completion_reason),
    )?;
    update_run(
        database,
        &request.run_id,
        "complete",
        None,
        review_count,
        revision_count,
        build_result.session_id.as_deref(),
        max_context_bytes,
        None,
        true,
    )?;
    emit_event(
        &app,
        &request.run_id,
        "complete",
        "complete",
        "complete",
        None,
        "Run complete",
        match promotion_mode {
            Some(PromotionMode::WorkingTree) => {
                "Verified work was applied to your current uncommitted checkout."
            }
            Some(PromotionMode::FastForward) => "Verified work was promoted safely.",
            None => "The reviewed objective required no repository change.",
        },
        Some(max_context_bytes),
    );
    notify(
        &app,
        "Agent Room complete",
        match promotion_mode {
            Some(PromotionMode::WorkingTree) => {
                "Verified work was applied without committing or stashing your existing changes."
            }
            Some(PromotionMode::FastForward) => "Verified work was promoted to your base branch.",
            None => "The reviewed objective completed without repository changes.",
        },
    );
    runtime.cancellations.lock().await.remove(&request.run_id);

    Ok(StartRunResult {
        run_id: request.run_id,
        state: "complete".to_owned(),
        summary: final_review_summary,
        builder: builder.kind.clone(),
        reviewer: reviewer.kind.clone(),
        degraded_review,
        session_id: build_result.session_id,
        changed_files: files,
        git_status: git_status(&base_repository),
        verification,
        stopped: false,
        promoted,
        worktree_path: None,
        branch,
        context_bytes: max_context_bytes,
        artifact_path: Some(artifact_dir.to_string_lossy().into_owned()),
        instruction_files,
        skill_files,
        recovery_count,
        attention_reason: None,
    })
}

pub(crate) fn run_state_allows_side_chat(state: &str) -> bool {
    matches!(
        state,
        "selecting" | "working" | "verifying" | "reviewing" | "revising"
    )
}

pub(crate) fn mark_run_and_activation_failed(
    database: &Database,
    run_id: &str,
    error: &str,
) -> Result<bool, String> {
    let connection = database.0.lock().map_err(|lock| lock.to_string())?;
    let transaction = connection
        .unchecked_transaction()
        .map_err(|database_error| database_error.to_string())?;
    let changed = transaction
        .execute(
            "UPDATE runs SET
                   state = 'failed',
                   current_owner = NULL,
                   stop_reason = ?1,
                   finished_at = CURRENT_TIMESTAMP
                 WHERE id = ?2
                   AND state IN ('selecting', 'working', 'verifying', 'reviewing', 'revising', 'promoting')",
            params![error, run_id],
        )
        .map_err(|database_error| database_error.to_string())?;
    if changed == 0 {
        transaction
            .rollback()
            .map_err(|database_error| database_error.to_string())?;
        return Ok(false);
    }
    transaction
        .execute(
            "UPDATE activations
             SET state = 'failed', finished_at = CURRENT_TIMESTAMP
             WHERE run_id = ?1 AND state = 'running'",
            params![run_id],
        )
        .map_err(|database_error| database_error.to_string())?;
    transaction
        .commit()
        .map_err(|database_error| database_error.to_string())?;
    Ok(true)
}

pub(crate) fn finalize_unhandled_run_error(
    app: &AppHandle,
    database: &Database,
    project_id: &str,
    run_id: &str,
    error: &str,
) -> Result<bool, String> {
    if !mark_run_and_activation_failed(database, run_id, error)? {
        return Ok(false);
    }
    persist_message(
        database,
        project_id,
        run_id,
        "system",
        "error",
        "Ship stopped safely.",
        &[],
        &[],
        Some(error),
    )?;
    emit_event(
        app,
        run_id,
        "attention",
        "ship",
        "failed",
        None,
        "Ship stopped safely",
        error,
        None,
    );
    notify(
        app,
        "Agent Room needs attention",
        "Ship stopped after an unexpected coordinator error. Its worktree was preserved.",
    );
    Ok(true)
}
