use super::*;

#[test]
fn receipt_usage_uses_only_provider_reported_fields() {
    let value = serde_json::json!({
        "model": "example-model",
        "usage": {
            "input_tokens": 120,
            "cached_input_tokens": "30",
            "output_tokens": 45,
            "total_cost_usd": 0.0125,
            "num_turns": 2
        }
    });
    let mut usage = ProviderUsage::default();
    merge_usage(&mut usage, &value);
    assert_eq!(reported_model(&value).as_deref(), Some("example-model"));
    assert_eq!(usage.input_tokens, Some(120));
    assert_eq!(usage.cached_input_tokens, Some(30));
    assert_eq!(usage.output_tokens, Some(45));
    assert_eq!(usage.total_cost_usd, Some(0.0125));
    assert_eq!(usage.num_turns, Some(2));
    assert!(usage_note(&ProviderUsage::default()).contains("did not report"));
}

#[test]
fn provider_activity_surfaces_only_explicit_reasoning_and_tools() {
    let reasoning = serde_json::json!({
        "type": "item.completed",
        "item": {
            "type": "reasoning",
            "text": "Inspecting the repository instructions."
        }
    });
    assert_eq!(
        provider_activity("codex", &reasoning),
        Some((
            "Thinking".to_owned(),
            "Inspecting the repository instructions.".to_owned()
        ))
    );

    let command = serde_json::json!({
        "type": "item.started",
        "item": {
            "type": "command_execution",
            "command": "npm test",
            "status": "in_progress"
        }
    });
    assert_eq!(
        provider_activity("codex", &command),
        Some(("Running command".to_owned(), "npm test".to_owned()))
    );
    assert!(provider_activity("codex", &serde_json::json!({"type": "thread.started"})).is_none());
}

#[test]
fn side_chat_closes_before_promotion_removes_the_worktree() {
    for state in ["selecting", "working", "verifying", "reviewing", "revising"] {
        assert!(run_state_allows_side_chat(state), "{state}");
    }
    for state in ["promoting", "complete", "failed", "stopped", "waiting"] {
        assert!(!run_state_allows_side_chat(state), "{state}");
    }
}

#[test]
fn unhandled_run_failure_finishes_the_active_activation_transactionally() {
    let connection = Connection::open_in_memory().expect("open test database");
    migrate(&connection).expect("migrate test database");
    connection
        .execute(
            "INSERT INTO projects (id, name, goal, repository_path)
             VALUES ('project-1', 'The Staff Room', 'Test failure', 'C:\\repo')",
            [],
        )
        .expect("insert project");
    connection
        .execute(
            "INSERT INTO runs
             (id, project_id, objective, state, current_owner)
             VALUES ('run-1', 'project-1', 'Test', 'working', 'codex')",
            [],
        )
        .expect("insert active run");
    connection
        .execute(
            "INSERT INTO activations
             (id, run_id, phase, participant_kind, state, context_bytes)
             VALUES ('activation-1', 'run-1', 'build', 'codex', 'running', 0)",
            [],
        )
        .expect("insert active activation");
    let database = Database(Mutex::new(connection));

    assert!(
        mark_run_and_activation_failed(&database, "run-1", "coordinator error")
            .expect("mark failure")
    );
    let connection = database.0.lock().expect("lock test database");
    let run: (String, Option<String>, Option<String>) = connection
        .query_row(
            "SELECT state, current_owner, finished_at FROM runs WHERE id = 'run-1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("read failed run");
    let activation: (String, Option<String>) = connection
        .query_row(
            "SELECT state, finished_at FROM activations WHERE id = 'activation-1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read failed activation");

    assert_eq!(run.0, "failed");
    assert!(run.1.is_none());
    assert!(run.2.is_some());
    assert_eq!(activation.0, "failed");
    assert!(activation.1.is_some());
}

#[test]
fn dirty_checkout_ship_applies_only_the_verified_delta_without_committing_user_work() {
    let (root, repository) = test_repository();
    let managed_root = root.join("managed");
    std::fs::write(repository.join("plan.md"), "user draft\n").expect("modify tracked file");
    std::fs::write(repository.join("notes.md"), "user notes\n").expect("write untracked file");
    let original_head = git_static(&repository, &["rev-parse", "HEAD"]).expect("read head");

    let isolation = create_isolation_at_root(&repository, "dirty-ship-test", &managed_root)
        .expect("create dirty isolation");
    assert_eq!(isolation.isolation_kind, "snapshot-clone");
    assert_eq!(
        std::fs::read_to_string(isolation.worktree.join("plan.md"))
            .expect("read snapshot tracked file")
            .replace("\r\n", "\n"),
        "user draft\n"
    );
    assert_eq!(
        std::fs::read_to_string(isolation.worktree.join("notes.md"))
            .expect("read snapshot untracked file"),
        "user notes\n"
    );

    std::fs::write(
        isolation.worktree.join("plan.md"),
        "user draft\nagent addition\n",
    )
    .expect("write agent change");
    std::fs::write(
        isolation.worktree.join("notes.md"),
        "user notes\nagent addition\n",
    )
    .expect("update captured untracked file");
    commit_managed_changes(&isolation.worktree, "Add implementation detail")
        .expect("commit agent change");
    let promotion =
        promote_worktree(&repository, &isolation, &managed_root).expect("apply verified delta");

    assert_eq!(promotion.mode, PromotionMode::WorkingTree);
    assert!(promotion.cleanup_warning.is_none());
    assert_eq!(
        git_static(&repository, &["rev-parse", "HEAD"]).expect("read unchanged head"),
        original_head
    );
    assert_eq!(
        std::fs::read_to_string(repository.join("plan.md"))
            .expect("read promoted file")
            .replace("\r\n", "\n"),
        "user draft\nagent addition\n"
    );
    assert_eq!(
        std::fs::read_to_string(repository.join("notes.md"))
            .expect("read user notes")
            .replace("\r\n", "\n"),
        "user notes\nagent addition\n"
    );
    assert!(git_static(&repository, &["status", "--porcelain"])
        .expect("read dirty status")
        .contains("?? notes.md"));
    assert!(!isolation.worktree.exists());
    remove_test_repository(&root);
}

#[test]
fn dirty_checkout_ship_preserves_isolation_when_the_user_edits_during_the_run() {
    let (root, repository) = test_repository();
    let managed_root = root.join("managed");
    std::fs::write(repository.join("plan.md"), "user draft\n").expect("modify tracked file");
    let isolation = create_isolation_at_root(&repository, "dirty-conflict-test", &managed_root)
        .expect("create dirty isolation");
    std::fs::write(
        isolation.worktree.join("plan.md"),
        "user draft\nagent addition\n",
    )
    .expect("write agent change");
    commit_managed_changes(&isolation.worktree, "Add implementation detail")
        .expect("commit agent change");

    std::fs::write(repository.join("plan.md"), "newer user edit\n")
        .expect("change attached checkout");
    let error =
        promote_worktree(&repository, &isolation, &managed_root).expect_err("promotion must stop");

    assert!(error.contains("changed while Ship was active"));
    assert_eq!(
        std::fs::read_to_string(repository.join("plan.md")).expect("read preserved edit"),
        "newer user edit\n"
    );
    assert!(isolation.worktree.exists());
    remove_managed_clone(&managed_root, &isolation.worktree).expect("remove isolation");
    remove_test_repository(&root);
}

#[test]
fn abandonment_removes_untracked_verification_artifacts() {
    let (root, repository) = test_repository();
    let managed_root = root.join("managed");
    let isolation = create_isolation_at_root(&repository, "abandon-dirty-test", &managed_root)
        .expect("create isolation");
    std::fs::write(
        isolation.worktree.join("generated.lock"),
        "verification output\n",
    )
    .expect("write untracked verification artifact");

    assert!(discard_isolation(&repository, &isolation, &managed_root).is_none());
    assert!(!isolation.worktree.exists());
    remove_test_repository(&root);
}

#[test]
fn dirty_checkout_ship_preserves_index_rename_deletion_and_binary_state() {
    let (root, repository) = test_repository();
    let managed_root = root.join("managed");
    std::fs::write(repository.join("delete.md"), "delete me\n").expect("write deleted file");
    std::fs::write(repository.join("old-name.md"), "rename me\n").expect("write renamed file");
    std::fs::write(repository.join("binary.bin"), [0_u8, 1, 2, 3]).expect("write binary file");
    git_static(&repository, &["add", "-A"]).expect("stage fixtures");
    git_static(&repository, &["commit", "-m", "Add fixtures"]).expect("commit fixtures");

    std::fs::write(repository.join("plan.md"), "staged user draft\n")
        .expect("write staged user change");
    git_static(&repository, &["add", "plan.md"]).expect("stage user change");
    std::fs::remove_file(repository.join("delete.md")).expect("delete tracked file");
    git_static(&repository, &["mv", "old-name.md", "new-name.md"]).expect("stage rename");
    std::fs::write(repository.join("binary.bin"), [0_u8, 255, 2, 3]).expect("modify binary file");
    std::fs::write(repository.join("notes.md"), "untracked user note\n")
        .expect("write untracked file");
    let staged_before =
        git_static(&repository, &["diff", "--cached", "--binary"]).expect("capture index");

    let isolation = create_isolation_at_root(&repository, "dirty-state-test", &managed_root)
        .expect("capture dirty state");
    assert!(!isolation.worktree.join("delete.md").exists());
    assert!(!isolation.worktree.join("old-name.md").exists());
    assert!(isolation.worktree.join("new-name.md").is_file());
    assert_eq!(
        std::fs::read(isolation.worktree.join("binary.bin")).expect("read snapshot binary"),
        [0_u8, 255, 2, 3]
    );

    std::fs::write(
        isolation.worktree.join("plan.md"),
        "staged user draft\nagent addition\n",
    )
    .expect("change staged-origin file");
    std::fs::write(
        isolation.worktree.join("new-name.md"),
        "rename me\nagent addition\n",
    )
    .expect("change renamed file");
    std::fs::write(isolation.worktree.join("binary.bin"), [0_u8, 255, 9, 3])
        .expect("change binary file");
    std::fs::write(
        isolation.worktree.join("notes.md"),
        "untracked user note\nagent addition\n",
    )
    .expect("change untracked file");
    commit_managed_changes(&isolation.worktree, "Update captured files")
        .expect("commit agent delta");
    promote_worktree(&repository, &isolation, &managed_root).expect("apply complex verified delta");

    assert_eq!(
        git_static(&repository, &["diff", "--cached", "--binary"]).expect("read preserved index"),
        staged_before
    );
    assert!(!repository.join("delete.md").exists());
    assert!(!repository.join("old-name.md").exists());
    assert_eq!(
        std::fs::read_to_string(repository.join("new-name.md"))
            .expect("read renamed file")
            .replace("\r\n", "\n"),
        "rename me\nagent addition\n"
    );
    assert_eq!(
        std::fs::read(repository.join("binary.bin")).expect("read promoted binary"),
        [0_u8, 255, 9, 3]
    );
    assert_eq!(
        std::fs::read_to_string(repository.join("notes.md"))
            .expect("read promoted untracked file")
            .replace("\r\n", "\n"),
        "untracked user note\nagent addition\n"
    );
    remove_test_repository(&root);
}

#[test]
fn clean_checkout_ship_still_fast_forwards_the_verified_branch() {
    let (root, repository) = test_repository();
    let managed_root = root.join("managed");
    let original_head = git_static(&repository, &["rev-parse", "HEAD"]).expect("read head");
    let isolation = create_isolation_at_root(&repository, "clean-ship-test", &managed_root)
        .expect("create clean isolation");
    assert_eq!(isolation.isolation_kind, "worktree");

    std::fs::write(isolation.worktree.join("plan.md"), "agent change\n")
        .expect("write agent change");
    commit_managed_changes(&isolation.worktree, "Update plan").expect("commit agent change");
    std::fs::write(
        isolation.worktree.join("generated.lock"),
        "verification output\n",
    )
    .expect("write untracked verification artifact");
    let promotion = promote_worktree(&repository, &isolation, &managed_root)
        .expect("fast-forward verified branch");

    assert_eq!(promotion.mode, PromotionMode::FastForward);
    assert!(promotion.cleanup_warning.is_none());
    assert_ne!(
        git_static(&repository, &["rev-parse", "HEAD"]).expect("read promoted head"),
        original_head
    );
    assert_eq!(
        std::fs::read_to_string(repository.join("plan.md"))
            .expect("read promoted file")
            .replace("\r\n", "\n"),
        "agent change\n"
    );
    assert!(!repository.join("generated.lock").exists());
    assert!(!isolation.worktree.exists());
    remove_test_repository(&root);
}

#[test]
fn ship_rejects_a_same_head_branch_switch_before_promotion() {
    let (root, repository) = test_repository();
    let managed_root = root.join("managed");
    let isolation = create_isolation_at_root(&repository, "branch-switch-test", &managed_root)
        .expect("create clean isolation");
    std::fs::write(isolation.worktree.join("plan.md"), "agent change\n")
        .expect("write agent change");
    commit_managed_changes(&isolation.worktree, "Update plan").expect("commit agent change");

    git_static(&repository, &["switch", "-c", "other-branch"]).expect("switch base branch");
    let error = promote_worktree(&repository, &isolation, &managed_root)
        .expect_err("branch switch must block promotion");
    assert!(error.contains("moved from branch"));
    assert!(error.contains("other-branch"));
    assert_eq!(
        std::fs::read_to_string(repository.join("plan.md"))
            .expect("read unchanged base")
            .replace("\r\n", "\n"),
        "committed\n"
    );

    if let Some(warning) = discard_isolation(&repository, &isolation, &managed_root) {
        panic!("{warning}");
    }
    remove_test_repository(&root);
}

#[test]
fn ship_rejects_a_detached_head_before_promotion() {
    let (root, repository) = test_repository();
    let managed_root = root.join("managed");
    let isolation = create_isolation_at_root(&repository, "detached-head-test", &managed_root)
        .expect("create clean isolation");
    std::fs::write(isolation.worktree.join("plan.md"), "agent change\n")
        .expect("write agent change");
    commit_managed_changes(&isolation.worktree, "Update plan").expect("commit agent change");

    git_static(&repository, &["checkout", "--detach", &isolation.base_head])
        .expect("detach base checkout");
    let error = promote_worktree(&repository, &isolation, &managed_root)
        .expect_err("detached head must block promotion");
    assert!(error.contains("detached HEAD"));

    if let Some(warning) = discard_isolation(&repository, &isolation, &managed_root) {
        panic!("{warning}");
    }
    remove_test_repository(&root);
}

#[test]
fn interrupted_runs_become_recoverable_on_restart() {
    let connection = Connection::open_in_memory().expect("open test database");
    migrate(&connection).expect("migrate test database");
    connection
        .execute(
            "INSERT INTO projects (id, name, goal, repository_path)
             VALUES ('project-1', 'The Staff Room', 'Test restart', 'C:\\repo')",
            [],
        )
        .expect("insert project");
    connection
        .execute(
            "INSERT INTO runs
             (id, project_id, objective, state, current_owner)
             VALUES ('run-1', 'project-1', 'Test', 'working', 'codex')",
            [],
        )
        .expect("insert active run");
    connection
        .execute(
            "INSERT INTO runs (id, project_id, objective, state)
             VALUES ('run-2', 'project-1', 'Promotion', 'promoting')",
            [],
        )
        .expect("insert promoting run");
    connection
        .execute(
            "INSERT INTO runs (id, project_id, objective, state, worktree_path)
             VALUES ('run-3', 'project-1', 'Abandonment', 'abandoning', 'C:\\worktree')",
            [],
        )
        .expect("insert abandoning run");
    connection
        .execute(
            "INSERT INTO activations
             (id, run_id, phase, participant_kind, state, context_bytes)
             VALUES ('activation-1', 'run-1', 'build', 'codex', 'running', 0)",
            [],
        )
        .expect("insert active activation");

    assert_eq!(
        reconcile_interrupted_runs(&connection).expect("reconcile runs"),
        3
    );
    let run: (String, Option<String>, Option<String>) = connection
        .query_row(
            "SELECT state, current_owner, stop_reason FROM runs WHERE id = 'run-1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("read reconciled run");
    let activation: String = connection
        .query_row(
            "SELECT state FROM activations WHERE id = 'activation-1'",
            [],
            |row| row.get(0),
        )
        .expect("read reconciled activation");
    assert_eq!(run.0, "stopped");
    assert!(run.1.is_none());
    assert!(run.2.expect("stop reason").contains("preserved"));
    assert_eq!(activation, "failed");
    let promoting: (String, String) = connection
        .query_row(
            "SELECT state, stop_reason FROM runs WHERE id = 'run-2'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read promoting run");
    assert_eq!(promoting.0, "waiting");
    assert_eq!(
        promoting.1,
        "Promotion state unknown — inspect the repository before continuing."
    );
    let abandoning: (String, String) = connection
        .query_row(
            "SELECT state, stop_reason FROM runs WHERE id = 'run-3'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read abandoning run");
    assert_eq!(abandoning.0, "waiting");
    assert_eq!(
        abandoning.1,
        "Abandonment state unknown — confirm Abandon again to finish cleanup."
    );
}

#[test]
fn ambiguous_repository_transitions_cannot_be_resumed() {
    assert!(recovery_has_ambiguous_repository_state(
        "waiting",
        Some("Promotion state unknown — inspect the repository before continuing.")
    ));
    assert!(recovery_has_ambiguous_repository_state(
        "waiting",
        Some("Abandonment state unknown — confirm Abandon again to finish cleanup.")
    ));
    assert!(!recovery_has_ambiguous_repository_state(
        "stopped",
        Some("The managed worktree was preserved for recovery.")
    ));
}

#[test]
fn review_mutation_guard_detects_a_review_write() {
    let (root, repository) = test_repository();
    let before = review_mutation_guard(&repository).expect("record review guard");
    std::fs::write(repository.join("reviewer-note.txt"), "unexpected write\n")
        .expect("simulate reviewer write");
    let after = review_mutation_guard(&repository).expect("record changed review guard");
    assert_ne!(before, after);
    remove_test_repository(&root);
}

#[test]
fn chat_receipt_does_not_require_an_autonomous_run() {
    let connection = Connection::open_in_memory().expect("open test database");
    migrate(&connection).expect("migrate test database");
    connection
        .execute(
            "INSERT INTO projects (id, name, goal, repository_path)
             VALUES (?1, ?2, ?3, ?4)",
            params!["project-1", "The Staff Room", "Test chat", "C:\\repo"],
        )
        .expect("insert project");
    let database = Database(Mutex::new(connection));
    let participant = Participant {
        kind: "codex".to_owned(),
        name: "Codex".to_owned(),
        installed: true,
        version: Some("test-version".to_owned()),
        executable_path: None,
        models: Vec::new(),
        model_discovery_note: String::new(),
        supports_effort: true,
        effort_options: provider_effort_options("codex"),
        state: "ready".to_owned(),
        connection_status: "ready".to_owned(),
        connection_detail: String::new(),
        last_verified_at: None,
        capabilities: capabilities_for("codex", true, None, "", ""),
    };
    let profile = ProviderProfile {
        participant_kind: "codex".to_owned(),
        route: "chat".to_owned(),
        model: Some("test-model".to_owned()),
        effort: None,
    };
    let result = ProviderRun {
        summary: "Chat response".to_owned(),
        session_id: Some("session-1".to_owned()),
        success: true,
        stopped: false,
        timed_out: false,
        idle_timed_out: false,
        stderr: String::new(),
        handoff: None,
        actual_model: Some("test-model".to_owned()),
        usage: ProviderUsage {
            input_tokens: Some(12),
            output_tokens: Some(4),
            ..ProviderUsage::default()
        },
        stdout_log_path: String::new(),
        stderr_log_path: String::new(),
        process_start_ms: 5,
        first_output_ms: Some(25),
        session_resumed: true,
    };

    persist_chat_receipt(
        &database,
        "project-1",
        "chat-1",
        &participant,
        &profile,
        ChatReceiptMetrics {
            context_bytes: 128,
            preflight_ms: 10,
            total_ms: 100,
        },
        &result,
    )
    .expect("persist chat receipt");

    let snapshot = load_room_snapshot(&database, "project-1", None).expect("load room");
    let receipt = snapshot.receipts.first().expect("chat receipt");
    assert_eq!(receipt.context_bytes, 128);
    assert_eq!(receipt.preflight_ms, Some(10));
    assert_eq!(receipt.process_start_ms, Some(5));
    assert_eq!(receipt.first_output_ms, Some(35));
    assert_eq!(receipt.total_ms, Some(100));
    assert!(receipt.session_resumed);
    assert_eq!(receipt.packet_bytes_saved, 0);
}

#[test]
fn cross_provider_handoff_uses_each_session_watermark() {
    let connection = Connection::open_in_memory().expect("open test database");
    migrate(&connection).expect("migrate test database");
    connection
        .execute(
            "INSERT INTO projects (id, name, goal, repository_path)
             VALUES (?1, ?2, ?3, ?4)",
            params!["project-1", "The Staff Room", "Test chat", "C:\\repo"],
        )
        .expect("insert project");
    let database = Database(Mutex::new(connection));
    persist_message(
        &database,
        "project-1",
        "chat-1",
        "human",
        "human",
        "Explain the architecture.",
        &[],
        &[],
        None,
    )
    .expect("persist human message");
    persist_message(
        &database,
        "project-1",
        "chat-1",
        "codex",
        "agent",
        &"A".repeat(CHAT_HANDOFF_BUDGET_BYTES * 2),
        &[],
        &[],
        None,
    )
    .expect("persist agent message");

    assert!(
        recent_chat_handoff(&database, "project-1", "codex", 2, None)
            .expect("same-provider handoff")
            .is_none()
    );
    let switched = recent_chat_handoff(&database, "project-1", "claude", 0, None)
        .expect("cross-provider handoff")
        .expect("handoff exists");
    assert!(switched.contains("Codex"));
    assert!(switched.contains("Explain the architecture."));
    assert!(switched.len() <= CHAT_HANDOFF_BUDGET_BYTES);
    assert!(
        recent_chat_handoff(&database, "project-1", "codex", 0, None)
            .expect("fresh same-provider handoff")
            .is_some()
    );

    persist_message(
        &database,
        "project-1",
        "ship-1",
        "human",
        "human",
        "How about now?",
        &[],
        &[],
        None,
    )
    .expect("persist referential objective");
    persist_message(
        &database,
        "project-1",
        "ship-1",
        "codex",
        "agent",
        "The objective is missing context.",
        &[],
        &[],
        None,
    )
    .expect("persist failed build response");
    let recovery_context = recent_chat_handoff(&database, "project-1", "codex", 0, Some("ship-1"))
        .expect("recovery context")
        .expect("prior chat remains available");
    assert!(recovery_context.contains("Explain the architecture."));
    assert!(!recovery_context.contains("missing context"));
}

#[test]
fn autonomous_ship_skill_requires_a_valid_explicit_intent() {
    let response = format!(
        "Ready to proceed.\n{SHIP_INTENT_START}\n{{\"schemaVersion\":1,\"objective\":\"Fix the failing chat persistence test\",\"reason\":\"The user explicitly requested a repository fix\"}}\n{SHIP_INTENT_END}"
    );
    let intent = extract_ship_intent(&response)
        .expect("valid intent")
        .expect("intent exists");
    assert_eq!(intent.objective, "Fix the failing chat persistence test");
    assert_eq!(visible_chat_response(&response), "Ready to proceed.");
    assert!(extract_ship_intent("Information-only answer.")
        .expect("no intent")
        .is_none());
    assert!(extract_ship_intent(&format!(
        "{SHIP_INTENT_START}\n{{\"schemaVersion\":2}}\n{SHIP_INTENT_END}"
    ))
    .is_err());
    let malformed =
        format!("I can help.\n{SHIP_INTENT_START}\n{{\"schemaVersion\":1,\"objective\":");
    assert_eq!(visible_chat_response(&malformed), "I can help.");
}
