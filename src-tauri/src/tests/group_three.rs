use super::*;

#[test]
fn autonomous_ship_setting_is_project_scoped() {
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
        project_settings(&database, "armed")
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
             VALUES ('project-1', 'Agent Room', 'Test', 'C:\\repo')",
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
fn cursor_command_uses_declared_headless_flags() {
    let request = TurnRequest {
        mode: ProviderMode::Ask,
        phase: "chat",
        prompt: "Read the packet.",
        repository: Path::new("C:/worktree"),
        session_id: Some("chat-1"),
        model: Some("claude-opus-4-8"),
        effort: Some("high"),
        final_output_path: Path::new("C:/output.txt"),
        structured_output: true,
        handoff_contract: None,
    };
    let command = providers::build_command("cursor", &request).expect("build cursor command");
    assert_eq!(
        command.args,
        [
            "--print",
            "--output-format",
            "stream-json",
            "--stream-partial-output",
            "--trust",
            "--workspace",
            "C:/worktree",
            "--sandbox",
            "enabled",
            "--mode",
            "ask",
            "--model",
            "claude-opus-4-8[effort=high]",
            "--resume",
            "chat-1",
            "Read the packet."
        ]
    );
}

#[test]
fn antigravity_review_uses_plan_without_permission_bypass() {
    let request = TurnRequest {
        mode: ProviderMode::Ship,
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
    assert!(cursor_models.contains(&"claude-opus-4-8".to_owned()));
    assert!(antigravity_models.contains(&"Gemini 3.1 Pro (high)".to_owned()));
    assert!(codex_effort.contains(&"xhigh".to_owned()));
    assert!(codex_effort.contains(&"max".to_owned()));
    assert!(codex_effort.contains(&"ultra".to_owned()));
    assert!(claude_effort.contains(&"max".to_owned()));
    assert!(cursor_effort.contains(&"xhigh".to_owned()));
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
    assert!(connection_test_ready(&punctuated));
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
    assert_eq!(
        provider_chat_fragment("cursor", &fragment).as_deref(),
        Some("Cursor ")
    );
    assert_eq!(
        provider_chat_text("cursor", &result).as_deref(),
        Some("Cursor answer")
    );
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
