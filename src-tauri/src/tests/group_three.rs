use super::*;

#[test]
fn quick_edit_apply_requires_the_exact_reviewed_diff() {
    assert!(
        require_reviewed_quick_edit(Some("diff --git a/a b/a\n"), "diff --git a/a b/a\n").is_ok()
    );
    let changed = require_reviewed_quick_edit(
        Some("diff --git a/a b/a\n-old\n+reviewed\n"),
        "diff --git a/a b/a\n-old\n+changed\n",
    )
    .expect_err("a post-review mutation must fail closed");
    assert!(changed.contains("changed after you reviewed"));
    assert!(require_reviewed_quick_edit(None, "diff").is_err());
}

#[test]
fn quick_edit_patch_serialization_terminates_the_last_hunk() {
    assert_eq!(serialize_git_patch("diff --git a/a b/a"), "diff --git a/a b/a\n");
    assert_eq!(
        serialize_git_patch("diff --git a/a b/a\n"),
        "diff --git a/a b/a\n"
    );
}

#[test]
fn ship_route_only_shows_revision_stages_when_they_ran() {
    let direct = stored_run_route(
        "awaiting-promotion",
        1,
        0,
        Some("codex"),
        Some("claude"),
        Some("review"),
    );
    assert_eq!(
        direct
            .iter()
            .map(|step| step.label.as_str())
            .collect::<Vec<_>>(),
        vec!["Build", "Verify", "Review", "Promote"]
    );
    assert_eq!(
        direct
            .iter()
            .map(|step| step.state.as_str())
            .collect::<Vec<_>>(),
        vec!["complete", "complete", "complete", "current"]
    );

    let revised = stored_run_route(
        "awaiting-promotion",
        2,
        1,
        Some("codex"),
        Some("claude"),
        Some("final-review"),
    );
    assert_eq!(
        revised
            .iter()
            .map(|step| step.label.as_str())
            .collect::<Vec<_>>(),
        vec![
            "Build",
            "Verify",
            "Review",
            "Revise",
            "Final review",
            "Promote"
        ]
    );
    assert_eq!(revised[4].state, "complete");
    assert_eq!(revised[5].state, "current");

    let failed_verification = stored_run_route(
        "failed",
        0,
        0,
        Some("codex"),
        Some("claude"),
        Some("verify"),
    );
    assert_eq!(failed_verification[0].state, "complete");
    assert_eq!(failed_verification[1].state, "current");

    let stopped_final_review = stored_run_route(
        "stopped",
        2,
        1,
        Some("codex"),
        Some("claude"),
        Some("final-review"),
    );
    assert_eq!(stopped_final_review[3].state, "complete");
    assert_eq!(stopped_final_review[4].state, "current");
}

#[test]
fn abandonment_claim_excludes_promotion_and_preserves_evidence() {
    let connection = Connection::open_in_memory().expect("open test database");
    migrate(&connection).expect("migrate test database");
    connection
        .execute(
            "INSERT INTO projects (id, name, goal, repository_path)
             VALUES ('project-1', 'The Staff Room', 'Test', 'C:\\repo')",
            [],
        )
        .expect("insert project");
    connection
        .execute(
            "INSERT INTO runs
             (id, project_id, objective, state, worktree_path, branch, isolation_kind,
              review_count, revision_count, context_bytes)
             VALUES ('run-1', 'project-1', 'Test', 'awaiting-promotion', 'C:\\worktree',
                     'staff-room/test', 'worktree', 2, 1, 4096)",
            [],
        )
        .expect("insert awaiting run");
    let database = Database(Mutex::new(connection));
    let request = ProjectRunRequest {
        project_id: "project-1".to_owned(),
        run_id: "run-1".to_owned(),
    };

    let claim = claim_run_for_abandonment(&database, &request).expect("claim abandonment");
    assert_eq!(claim.previous_state, "awaiting-promotion");
    let promotion_claimed = database
        .0
        .lock()
        .expect("lock database")
        .execute(
            "UPDATE runs SET state = 'promoting'
             WHERE id = 'run-1' AND project_id = 'project-1'
               AND state = 'awaiting-promotion'",
            [],
        )
        .expect("attempt promotion claim");
    assert_eq!(promotion_claimed, 0);

    finish_abandonment(&database, &request).expect("finish abandonment");
    let persisted: (String, i64, i64, i64, Option<String>) = database
        .0
        .lock()
        .expect("lock database")
        .query_row(
            "SELECT state, review_count, revision_count, context_bytes, worktree_path
             FROM runs WHERE id = 'run-1'",
            [],
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
        .expect("read abandoned run");
    assert_eq!(persisted, ("abandoned".to_owned(), 2, 1, 4096, None));
}

#[test]
fn completed_run_cannot_be_relabelled_as_abandoned() {
    let connection = Connection::open_in_memory().expect("open test database");
    migrate(&connection).expect("migrate test database");
    connection
        .execute(
            "INSERT INTO projects (id, name, goal, repository_path)
             VALUES ('project-1', 'The Staff Room', 'Test', 'C:\\repo')",
            [],
        )
        .expect("insert project");
    connection
        .execute(
            "INSERT INTO runs
             (id, project_id, objective, state, worktree_path, branch, isolation_kind)
             VALUES ('run-1', 'project-1', 'Test', 'complete', 'C:\\worktree',
                     'staff-room/test', 'worktree')",
            [],
        )
        .expect("insert completed run");
    let database = Database(Mutex::new(connection));
    let request = ProjectRunRequest {
        project_id: "project-1".to_owned(),
        run_id: "run-1".to_owned(),
    };

    let error = claim_run_for_abandonment(&database, &request)
        .expect_err("completed runs must preserve promotion history");
    assert!(error.contains("inactive Ship result"));
    let state: String = database
        .0
        .lock()
        .expect("lock database")
        .query_row("SELECT state FROM runs WHERE id = 'run-1'", [], |row| {
            row.get(0)
        })
        .expect("read completed run");
    assert_eq!(state, "complete");
}

#[test]
fn recovery_and_abandonment_claims_are_mutually_exclusive() {
    let connection = Connection::open_in_memory().expect("open test database");
    migrate(&connection).expect("migrate test database");
    connection
        .execute(
            "INSERT INTO projects (id, name, goal, repository_path)
             VALUES ('project-1', 'The Staff Room', 'Test', 'C:\\repo')",
            [],
        )
        .expect("insert project");
    connection
        .execute(
            "INSERT INTO runs
             (id, project_id, objective, state, worktree_path, branch, isolation_kind)
             VALUES ('recover-first', 'project-1', 'Recover', 'waiting', 'C:\\recover',
                     'staff-room/recover', 'worktree'),
                    ('abandon-first', 'project-1', 'Abandon', 'waiting', 'C:\\abandon',
                     'staff-room/abandon', 'worktree')",
            [],
        )
        .expect("insert preserved runs");
    let database = Database(Mutex::new(connection));

    let recovery = StartRunRequest {
        run_id: "recover-first".to_owned(),
        project_id: "project-1".to_owned(),
        objective: "Recover".to_owned(),
        requested_agent: None,
    };
    claim_run_for_recovery(&database, &recovery, "waiting", 0)
        .expect("recovery claims the preserved run");
    let abandon_after_recovery = ProjectRunRequest {
        run_id: "recover-first".to_owned(),
        project_id: "project-1".to_owned(),
    };
    assert!(claim_run_for_abandonment(&database, &abandon_after_recovery).is_err());

    let abandon_before_recovery = ProjectRunRequest {
        run_id: "abandon-first".to_owned(),
        project_id: "project-1".to_owned(),
    };
    claim_run_for_abandonment(&database, &abandon_before_recovery)
        .expect("abandonment claims the preserved run");
    let blocked_recovery = StartRunRequest {
        run_id: "abandon-first".to_owned(),
        project_id: "project-1".to_owned(),
        objective: "Abandon".to_owned(),
        requested_agent: None,
    };
    assert!(claim_run_for_recovery(&database, &blocked_recovery, "waiting", 0).is_err());
}

#[test]
fn autonomous_ship_setting_is_fail_closed_until_acceptance() {
    let connection = Connection::open_in_memory().expect("open test database");
    migrate(&connection).expect("migrate test database");
    connection
        .execute(
            "INSERT INTO projects (id, name, goal, repository_path)
             VALUES ('armed', 'Armed', 'Test', 'C:\\armed'),
                    ('quiet', 'Quiet', 'Test', 'C:\\quiet')",
            [],
        )
        .expect("insert projects");
    connection
        .execute(
            "INSERT INTO project_settings (project_id, autonomous_ship_enabled)
             VALUES ('armed', 1)",
            [],
        )
        .expect("arm project");
    let database = Database(Mutex::new(connection));

    assert!(
        !project_settings(&database, "armed")
            .expect("armed settings")
            .autonomous_ship_enabled
    );
    assert!(
        !project_settings(&database, "quiet")
            .expect("default settings")
            .autonomous_ship_enabled
    );
}

#[test]
fn legacy_provider_profile_migrates_to_every_route_without_overwriting() {
    let connection = Connection::open_in_memory().expect("open test database");
    migrate(&connection).expect("migrate test database");
    connection
        .execute(
            "INSERT INTO projects (id, name, goal, repository_path)
             VALUES ('project-1', 'The Staff Room', 'Test', 'C:\\repo')",
            [],
        )
        .expect("insert project");
    connection
        .execute(
            "INSERT INTO provider_profiles
             (project_id, participant_kind, model, effort)
             VALUES ('project-1', 'codex', 'legacy-model', 'high')",
            [],
        )
        .expect("insert legacy profile");
    migrate(&connection).expect("rerun migration");
    connection
        .execute(
            "UPDATE provider_route_profiles
             SET model = 'chat-model'
             WHERE project_id = 'project-1'
               AND participant_kind = 'codex'
               AND route = 'chat'",
            [],
        )
        .expect("customize chat route");
    migrate(&connection).expect("rerun idempotent migration");

    let mut statement = connection
        .prepare(
            "SELECT route, model
             FROM provider_route_profiles
             WHERE project_id = 'project-1' AND participant_kind = 'codex'
             ORDER BY route",
        )
        .expect("prepare profiles");
    let profiles = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .expect("query profiles")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect profiles");

    assert_eq!(
        profiles,
        vec![
            ("build".to_owned(), "legacy-model".to_owned()),
            ("chat".to_owned(), "chat-model".to_owned()),
            ("review".to_owned(), "legacy-model".to_owned()),
        ]
    );
}

#[test]
fn cursor_command_passes_the_selected_compound_model_through_unchanged() {
    let request = TurnRequest {
        mode: ProviderMode::Ask,
        phase: "chat",
        prompt: "Read the packet.",
        repository: Path::new("C:/worktree"),
        session_id: Some("chat-1"),
        model: Some("cursor-grok-4.5-high"),
        effort: Some("high"),
        final_output_path: Path::new("C:/output.txt"),
        structured_output: true,
        handoff_contract: None,
    };
    let command = providers::build_command("cursor", &request).expect("build cursor command");
    let mut expected = vec![
        "--print".to_owned(),
        "--output-format".to_owned(),
        "stream-json".to_owned(),
        "--stream-partial-output".to_owned(),
        "--workspace".to_owned(),
        "C:/worktree".to_owned(),
    ];
    #[cfg(not(windows))]
    expected.extend(["--sandbox".to_owned(), "enabled".to_owned()]);
    expected.extend([
        "--mode".to_owned(),
        "ask".to_owned(),
        "--model".to_owned(),
        "cursor-grok-4.5-high".to_owned(),
        "--resume".to_owned(),
        "chat-1".to_owned(),
        "Read the packet.".to_owned(),
    ]);
    assert_eq!(command.args, expected);
}

#[test]
fn cursor_fatal_stderr_is_detected_before_the_idle_watchdog() {
    assert!(provider_fatal_stderr(
        "cursor",
        "Cannot use this model: cursor-grok-4.5"
    ));
    assert!(provider_fatal_stderr(
        "cursor",
        "Failed to load models: [internal]"
    ));
    assert!(provider_fatal_stderr("cursor", "Authentication required"));
    assert!(!provider_fatal_stderr(
        "cursor",
        "Working on the repository"
    ));
    assert!(!provider_fatal_stderr(
        "codex",
        "Cannot use this model: stale"
    ));
}

#[test]
fn cursor_unavailable_model_extracts_the_actionable_identifier() {
    assert_eq!(
        cursor_unavailable_model(
            "Cannot use this model: cursor-grok-4.5. Available models: cursor-grok-4.5-high"
        )
        .as_deref(),
        Some("cursor-grok-4.5")
    );
    assert_eq!(
        cursor_unavailable_model("  cannot use this model: cursor-grok-4.5  ").as_deref(),
        Some("cursor-grok-4.5")
    );
    assert_eq!(cursor_unavailable_model("Cursor is ready"), None);
}

#[test]
fn antigravity_review_uses_plan_without_permission_bypass() {
    let request = TurnRequest {
        mode: ProviderMode::Review,
        phase: "review",
        prompt: "Review the diff.",
        repository: Path::new("C:/worktree"),
        session_id: None,
        model: None,
        effort: None,
        final_output_path: Path::new("C:/output.txt"),
        structured_output: false,
        handoff_contract: None,
    };
    let command = providers::build_command("antigravity", &request)
        .expect("build Antigravity review command");
    assert!(command
        .args
        .windows(2)
        .any(|pair| pair == ["--mode", "plan"]));
    assert!(!command
        .args
        .iter()
        .any(|arg| arg == "--dangerously-skip-permissions"));
}

#[test]
fn antigravity_probe_allows_required_headless_reads_without_leaving_plan_mode() {
    let request = TurnRequest {
        mode: ProviderMode::Probe,
        phase: "chat",
        prompt: "Reply with exactly READY.",
        repository: Path::new("C:/worktree"),
        session_id: None,
        model: None,
        effort: None,
        final_output_path: Path::new("C:/output.txt"),
        structured_output: false,
        handoff_contract: None,
    };
    let command =
        providers::build_command("antigravity", &request).expect("build Antigravity probe command");
    assert!(command
        .args
        .windows(2)
        .any(|pair| pair == ["--mode", "plan"]));
    assert!(command
        .args
        .iter()
        .any(|arg| arg == "--dangerously-skip-permissions"));
}

#[test]
fn every_review_adapter_is_prepared_read_only() {
    for kind in ["codex", "claude", "cursor", "antigravity"] {
        let request = TurnRequest {
            mode: ProviderMode::Review,
            phase: "review",
            prompt: "Review without editing.",
            repository: Path::new("C:/worktree"),
            session_id: None,
            model: None,
            effort: None,
            final_output_path: Path::new("C:/output.txt"),
            structured_output: true,
            handoff_contract: None,
        };
        let command = providers::build_command(kind, &request).expect("build review command");
        let joined = command.args.join(" ");
        match kind {
            "codex" => assert!(joined.contains("-s read-only")),
            "claude" => {
                assert!(joined.contains("--permission-mode plan"));
                assert!(joined.contains("--tools Read,Grep,Glob"));
            }
            "cursor" => {
                assert!(joined.contains("--mode ask"));
                assert!(!command
                    .args
                    .iter()
                    .any(|value| value == "--force" || value == "--trust"));
            }
            "antigravity" => {
                assert!(joined.contains("--mode plan"));
                assert!(!joined.contains("--dangerously-skip-permissions"));
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn argv_provider_rejects_an_oversized_prompt_before_spawn() {
    let prompt = "x".repeat(providers::MAX_ARGV_PROMPT_CHARS);
    let request = TurnRequest {
        mode: ProviderMode::Ship,
        phase: "build",
        prompt: &prompt,
        repository: Path::new("C:/worktree"),
        session_id: None,
        model: None,
        effort: None,
        final_output_path: Path::new("C:/output.txt"),
        structured_output: true,
        handoff_contract: None,
    };
    assert!(providers::build_command("cursor", &request).is_err());
    assert!(providers::build_command("antigravity", &request).is_err());
}

#[test]
fn every_provider_exposes_exact_models_and_truthful_effort_options() {
    let (codex_models, _, codex_effort) = model_options("codex", Some(Path::new("codex")));
    let (claude_models, _, claude_effort) = model_options("claude", Some(Path::new("claude")));
    let (cursor_models, _, cursor_effort) =
        model_options("cursor", Some(Path::new("cursor-agent")));
    let (antigravity_models, _, antigravity_effort) =
        model_options("antigravity", Some(Path::new("agy")));

    assert!(codex_models.contains(&"gpt-5.6-sol".to_owned()));
    assert!(claude_models.contains(&"claude-opus-4-8".to_owned()));
    assert!(claude_models.contains(&"claude-opus-5".to_owned()));
    assert!(cursor_models.is_empty());
    assert!(antigravity_models.contains(&"Gemini 3.1 Pro (high)".to_owned()));
    assert!(codex_effort.contains(&"xhigh".to_owned()));
    assert!(codex_effort.contains(&"max".to_owned()));
    assert!(codex_effort.contains(&"ultra".to_owned()));
    assert!(claude_effort.contains(&"max".to_owned()));
    assert!(cursor_effort.is_empty());
    assert_eq!(antigravity_effort, vec!["low", "medium", "high"]);
}

#[test]
fn connection_test_rejects_onboarding_output_even_when_the_process_succeeds() {
    let result = ProviderRun {
        summary: "Welcome to the Antigravity CLI. You are currently not signed in.".to_owned(),
        session_id: None,
        success: true,
        stopped: false,
        timed_out: false,
        idle_timed_out: false,
        stderr: String::new(),
        handoff: None,
        actual_model: None,
        usage: ProviderUsage::default(),
        stdout_log_path: String::new(),
        stderr_log_path: String::new(),
        process_start_ms: 0,
        first_output_ms: Some(10),
        session_resumed: false,
    };
    assert!(!connection_test_ready(&result));
    assert!(authentication_attention(&result.summary, &result.stderr).is_some());

    let ready = ProviderRun {
        summary: " READY ".to_owned(),
        ..result
    };
    assert!(connection_test_ready(&ready));
    let punctuated = ProviderRun {
        summary: "```READY.```".to_owned(),
        ..ready
    };
    assert!(!connection_test_ready(&punctuated));
    assert!(authentication_attention("This repository uses OAuth authentication.", "").is_none());
    assert!(authentication_attention("", "OAuth authentication required").is_some());
}

#[tokio::test]
async fn warm_provider_cache_avoids_reprobing_the_cli() {
    let runtime = RuntimeState::default();
    let participant = Participant {
        kind: "codex".to_owned(),
        name: "Codex".to_owned(),
        installed: true,
        version: Some("cached-version".to_owned()),
        executable_path: Some("intentionally-missing.exe".to_owned()),
        models: Vec::new(),
        model_discovery_note: String::new(),
        supports_effort: false,
        effort_options: Vec::new(),
        state: "ready".to_owned(),
        connection_status: "connected".to_owned(),
        connection_detail: String::new(),
        last_verified_at: None,
        capabilities: capabilities_for("codex", true, None, "", ""),
    };
    runtime
        .provider_cache
        .lock()
        .await
        .insert("codex".to_owned(), participant);

    let cached = cached_provider(&runtime, "codex", false)
        .await
        .expect("read warm cache");
    assert_eq!(cached.version.as_deref(), Some("cached-version"));
}

#[test]
fn codex_chat_adapter_extracts_only_agent_messages() {
    let message = serde_json::json!({
        "type": "item.completed",
        "item": {"type": "agent_message", "text": "Codex answer"}
    });
    let command = serde_json::json!({
        "type": "item.completed",
        "item": {"type": "command_execution", "text": "not chat"}
    });
    assert_eq!(
        provider_chat_text("codex", &message).as_deref(),
        Some("Codex answer")
    );
    assert!(provider_chat_text("codex", &command).is_none());
}

#[test]
fn claude_chat_adapter_supports_snapshots_and_partial_text() {
    let assistant = serde_json::json!({
        "type": "assistant",
        "message": {
            "content": [
                {"type": "thinking", "thinking": "private"},
                {"type": "text", "text": "Claude answer"}
            ]
        }
    });
    let partial = serde_json::json!({
        "type": "stream_event",
        "event": {
            "type": "content_block_delta",
            "delta": {"type": "text_delta", "text": "Cl"}
        }
    });
    assert_eq!(
        provider_chat_text("claude", &assistant).as_deref(),
        Some("Claude answer")
    );
    assert_eq!(
        provider_chat_fragment("claude", &partial).as_deref(),
        Some("Cl")
    );
}

#[test]
fn codex_app_server_handshake_fixture_is_valid_jsonl() {
    let handshake = include_str!("../../tests/fixtures/codex/app-server-handshake.jsonl")
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("live handshake JSON"))
        .collect::<Vec<_>>();

    assert_eq!(handshake.len(), 1);
    assert_eq!(handshake[0]["id"], 1);
    assert_eq!(handshake[0]["result"]["platformFamily"], "windows");
}

#[test]
fn chat_stream_coalesces_fixture_deltas_with_linear_event_bytes() {
    let answer = "Claude stream output. ".repeat(1_100);
    let mut stream = ChatStreamBuffer::default();
    for fragment in answer.as_bytes().chunks(19) {
        let event = serde_json::json!({
            "type": "stream_event",
            "event": {
                "type": "content_block_delta",
                "delta": {"type": "text_delta", "text": std::str::from_utf8(fragment).expect("utf-8 fragment")}
            }
        });
        stream
            .push_fragment(&provider_chat_fragment("claude", &event).expect("fixture text delta"));
    }
    assert_eq!(stream.result_text, answer);

    let delta = stream.take_text_delta().expect("coalesced delta");
    assert_eq!(delta, answer);
    let emitted_bytes = serde_json::to_vec(&RunEvent {
        run_id: "fixture".to_owned(),
        event_type: "text-delta".to_owned(),
        phase: "chat".to_owned(),
        state: "running".to_owned(),
        agent: Some("claude".to_owned()),
        title: "Chat response".to_owned(),
        detail: String::new(),
        text_delta: Some(delta),
        context_bytes: None,
    })
    .expect("serialize event")
    .len();
    assert!(emitted_bytes < answer.len() * 2);
}

#[test]
fn cursor_chat_adapter_supports_stream_fragments_and_terminal_result() {
    let fragment = serde_json::json!({
        "type": "assistant",
        "message": {
            "role": "assistant",
            "content": [{"type": "text", "text": "Cursor "}]
        }
    });
    let result = serde_json::json!({
        "type": "result",
        "result": "Cursor answer"
    });
    let forwarded_prompt = serde_json::json!({
        "type": "user",
        "message": {
            "role": "user",
            "content": [{"type": "text", "text": "Read the prompt file."}]
        }
    });
    let assistant_message = serde_json::json!({
        "message": {
            "role": "assistant",
            "content": [{"type": "text", "text": "Assistant answer"}]
        }
    });
    assert_eq!(
        provider_chat_fragment("cursor", &fragment).as_deref(),
        Some("Cursor ")
    );
    assert_eq!(
        provider_chat_text("cursor", &result).as_deref(),
        Some("Cursor answer")
    );
    assert!(parse_result_text(&forwarded_prompt).is_none());
    assert_eq!(
        parse_result_text(&assistant_message).as_deref(),
        Some("Assistant answer")
    );
}

#[test]
fn unborn_git_repository_is_detected_before_cursor_launch() {
    let root = std::env::temp_dir().join(format!("staff-room-unborn-{}", Uuid::new_v4()));
    let repository = root.join("repository");
    std::fs::create_dir_all(&repository).expect("create unborn repository");
    git(&repository, &["init".to_owned()]).expect("initialize unborn repository");

    assert!(!repository_has_head(&repository).expect("inspect unborn repository"));
    let error = ensure_cursor_repository_has_head(&repository)
        .expect_err("Cursor launch should require an initial commit");
    assert!(error.contains("requires the attached Git repository to have an initial commit"));

    remove_test_repository(&root);

    let (committed_root, committed_repository) = test_repository();
    assert!(repository_has_head(&committed_repository).expect("inspect committed repository"));
    remove_test_repository(&committed_root);
}

#[test]
fn provider_usage_accepts_claude_and_codex_field_shapes() {
    let claude = serde_json::json!({
        "message": {
            "usage": {
                "input_tokens": 20,
                "cache_read_input_tokens": 8,
                "output_tokens": 4
            }
        },
        "total_cost_usd": 0.01
    });
    let codex = serde_json::json!({
        "usage": {
            "inputTokens": 30,
            "cachedInputTokens": 10,
            "outputTokens": 5
        }
    });
    let mut claude_usage = ProviderUsage::default();
    merge_usage(&mut claude_usage, &claude);
    assert_eq!(claude_usage.input_tokens, Some(20));
    assert_eq!(claude_usage.cached_input_tokens, Some(8));
    assert_eq!(claude_usage.output_tokens, Some(4));
    assert_eq!(claude_usage.total_cost_usd, Some(0.01));
    let mut codex_usage = ProviderUsage::default();
    merge_usage(&mut codex_usage, &codex);
    assert_eq!(codex_usage.input_tokens, Some(30));
    assert_eq!(codex_usage.cached_input_tokens, Some(10));
    assert_eq!(codex_usage.output_tokens, Some(5));
}

#[test]
fn chat_accepts_plain_provider_text_without_a_handoff() {
    assert!(chat_response_is_complete("The repository is ready."));
    assert!(!chat_response_is_complete(" \n\t "));
    assert!(extract_phase_handoff("plain provider text", Phase::Chat).is_err());
}
