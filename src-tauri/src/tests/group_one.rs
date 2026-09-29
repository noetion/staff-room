use super::*;

#[tokio::test]
async fn lockfile_prepare_runs_before_detected_npm_check() {
    if find_executable(&["npm", "npm.cmd"]).is_none() {
        return;
    }
    let (root, repository) = test_repository();
    std::fs::write(
        repository.join("package.json"),
        r#"{"name":"verification-fixture","version":"1.0.0","scripts":{"test":"node -e \"process.exit(0)\""}}"#,
    )
    .expect("write package manifest");
    std::fs::write(
        repository.join("package-lock.json"),
        r#"{"name":"verification-fixture","version":"1.0.0","lockfileVersion":3,"packages":{"":{"name":"verification-fixture","version":"1.0.0"}}}"#,
    )
    .expect("write package lock");
    let config = detected_verification_config(&repository);
    assert_eq!(config.prepare.as_deref(), Some("npm ci --prefer-offline"));
    let (_cancellation_sender, mut cancellation) = watch::channel(false);
    let prepare = run_verification_command(
        "Prepare dependencies",
        config.prepare.as_deref().expect("prepare command"),
        &repository,
        &mut cancellation,
        "unavailable",
    )
    .await;
    assert_eq!(prepare.status, "passed", "{}", prepare.detail);
    let check = run_verification_command(
        &config.commands[0].label,
        &config.commands[0].command,
        &repository,
        &mut cancellation,
        "failed",
    )
    .await;
    assert_eq!(check.status, "passed", "{}", check.detail);
    std::fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn empty_verification_config_cannot_pass() {
    let results = vec![VerificationResult {
        label: "Project checks".to_owned(),
        status: "not-run".to_owned(),
        detail: "No verification is configured for this project.".to_owned(),
    }];
    assert!(verification_is_unconfigured(&results));
    assert!(!verification_passed(&results));
}

#[test]
fn attached_projects_keep_room_messages_scoped() {
    let (first_root, first_repository) = test_repository();
    let (second_root, second_repository) = test_repository();
    let connection = Connection::open_in_memory().expect("open test database");
    migrate(&connection).expect("migrate test database");
    let database = Database(Mutex::new(connection));
    let first = attach_project(&database, &first_repository.to_string_lossy())
        .expect("attach first repository");
    let second = attach_project(&database, &second_repository.to_string_lossy())
        .expect("attach second repository");

    {
        let connection = database.0.lock().expect("lock database");
        connection
            .execute(
                "INSERT INTO messages (id, project_id, sender_kind, message_kind, body)
                 VALUES ('first-message', ?1, 'human', 'human', 'first room')",
                [&first.id],
            )
            .expect("write first message");
        connection
            .execute(
                "INSERT INTO messages (id, project_id, sender_kind, message_kind, body)
                 VALUES ('second-message', ?1, 'human', 'human', 'second room')",
                [&second.id],
            )
            .expect("write second message");
    }

    let first_room = load_room_snapshot(&database, &first.id, None).expect("load first room");
    let second_room = load_room_snapshot(&database, &second.id, None).expect("load second room");
    assert_eq!(first_room.messages.len(), 1);
    assert_eq!(first_room.messages[0].body, "first room");
    assert_eq!(second_room.messages.len(), 1);
    assert_eq!(second_room.messages[0].body, "second room");

    std::fs::remove_dir_all(first_root).expect("remove first test repository");
    std::fs::remove_dir_all(second_root).expect("remove second test repository");
}

#[test]
fn room_messages_page_backwards_from_the_newest_hundred() {
    let connection = Connection::open_in_memory().expect("open test database");
    migrate(&connection).expect("migrate test database");
    connection
        .execute(
            "INSERT INTO projects (id, name, goal, repository_path)
             VALUES ('project-1', 'Project', '', 'C:/project')",
            [],
        )
        .expect("insert project");
    for index in 0..500 {
        connection
            .execute(
                "INSERT INTO messages (id, project_id, sender_kind, message_kind, body)
                 VALUES (?1, 'project-1', 'human', 'human', ?1)",
                [format!("message-{index:03}")],
            )
            .expect("insert message");
    }
    let database = Database(Mutex::new(connection));
    let mut page = load_room_snapshot(&database, "project-1", None).expect("load newest page");
    let mut message_ids = page
        .messages
        .iter()
        .map(|message| message.id.clone())
        .collect::<Vec<_>>();
    assert_eq!(message_ids.first().map(String::as_str), Some("message-400"));
    assert_eq!(message_ids.last().map(String::as_str), Some("message-499"));
    while page.has_more {
        let cursor = page
            .next_message_cursor
            .clone()
            .expect("cursor for older page");
        page = load_room_snapshot(&database, "project-1", Some(&cursor)).expect("load older page");
        message_ids.extend(page.messages.iter().map(|message| message.id.clone()));
    }
    assert_eq!(message_ids.len(), 500);
    assert_eq!(message_ids.first().map(String::as_str), Some("message-400"));
    assert_eq!(message_ids.last().map(String::as_str), Some("message-099"));
}

#[test]
fn v1_project_migration_preserves_project_scoped_row_counts() {
    let (root, repository) = test_repository();
    let connection = Connection::open_in_memory().expect("open fixture database");
    migrate(&connection).expect("create v1 fixture schema");
    connection
        .execute(
            "INSERT INTO projects (id, name, goal, repository_path)
             VALUES ('agent-room', 'The Staff Room', 'legacy', ?1)",
            [repository.to_string_lossy().into_owned()],
        )
        .expect("insert legacy project");
    connection
        .execute(
            "INSERT INTO messages (id, project_id, sender_kind, message_kind, body)
             VALUES ('legacy-message', 'agent-room', 'human', 'human', 'preserve me')",
            [],
        )
        .expect("insert legacy message");
    let before = project_row_counts(&connection).expect("count legacy rows");

    migrate(&connection).expect("migrate v1 fixture");

    assert_eq!(
        project_row_counts(&connection).expect("count migrated rows"),
        before
    );
    let project_id =
        project_id_for_root(&std::fs::canonicalize(&repository).expect("canonical repository"));
    let message_project_id: String = connection
        .query_row(
            "SELECT project_id FROM messages WHERE id = 'legacy-message'",
            [],
            |row| row.get(0),
        )
        .expect("load migrated message");
    assert_eq!(message_project_id, project_id);
    std::fs::remove_dir_all(root).expect("remove fixture repository");
}

#[test]
fn v1_project_migration_merges_when_canonical_project_already_exists() {
    let (root, repository) = test_repository();
    let connection = Connection::open_in_memory().expect("open fixture database");
    migrate(&connection).expect("create v1 fixture schema");
    let repository = std::fs::canonicalize(repository).expect("canonical repository");
    let project_id = project_id_for_root(&repository);
    connection
        .execute(
            "INSERT INTO projects (id, name, goal, repository_path)
             VALUES ('agent-room', 'Legacy', 'legacy', ?1),
                    (?2, 'Current', 'current', ?1)",
            params![repository.to_string_lossy(), project_id],
        )
        .expect("insert duplicate projects");
    connection
        .execute(
            "INSERT INTO messages (id, project_id, sender_kind, message_kind, body)
             VALUES ('legacy-message', 'agent-room', 'human', 'human', 'preserve me')",
            [],
        )
        .expect("insert legacy message");
    connection
        .execute(
            "INSERT INTO provider_connections
             (project_id, participant_kind, status, detail)
             VALUES ('agent-room', 'codex', 'legacy', 'legacy'),
                    (?1, 'codex', 'current', 'current')",
            [&project_id],
        )
        .expect("insert duplicate provider connections");
    connection
        .execute(
            "INSERT INTO app_state (key, value) VALUES ('active_project_id', 'agent-room')",
            [],
        )
        .expect("set legacy project active");

    migrate(&connection).expect("merge duplicate projects");

    let project_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0))
        .expect("count projects");
    assert_eq!(project_count, 1);
    let message_project_id: String = connection
        .query_row(
            "SELECT project_id FROM messages WHERE id = 'legacy-message'",
            [],
            |row| row.get(0),
        )
        .expect("load migrated message");
    assert_eq!(message_project_id, project_id);
    let connection_detail: String = connection
        .query_row(
            "SELECT detail FROM provider_connections
             WHERE project_id = ?1 AND participant_kind = 'codex'",
            [&project_id],
            |row| row.get(0),
        )
        .expect("load canonical provider connection");
    assert_eq!(connection_detail, "current");
    let active_project_id: String = connection
        .query_row(
            "SELECT value FROM app_state WHERE key = 'active_project_id'",
            [],
            |row| row.get(0),
        )
        .expect("load active project");
    assert_eq!(active_project_id, project_id);
    std::fs::remove_dir_all(root).expect("remove fixture repository");
}

#[test]
fn context_packet_is_bounded_and_explicitly_truncated() {
    let oversized = "x".repeat(BUILD_CONTEXT_BUDGET_BYTES * 2);
    let (packet, bytes) =
        assemble_packet(Phase::Build.context_budget_bytes(), &[("Large", oversized)]);
    assert!(bytes <= BUILD_CONTEXT_BUDGET_BYTES);
    assert!(packet.contains("truncated"));
}

#[test]
fn handoff_requires_markers_and_a_valid_schema() {
    let value = format!(
        "Done.\n{HANDOFF_START}\n{{\"schemaVersion\":1,\"status\":\"approved\",\"summary\":\"Verified\",\"changedFiles\":[],\"checks\":[\"npm test: passed\"],\"findings\":[],\"nextAction\":\"none\"}}\n{HANDOFF_END}"
    );
    let handoff = extract_handoff(&value).expect("valid handoff");
    assert_eq!(handoff.status, "approved");
    assert!(extract_handoff("{\"status\":\"approved\"}").is_err());
}

#[test]
fn phase_handoff_accepts_a_completed_builder_before_process_exit() {
    let value = format!(
        "Working output.\n{HANDOFF_START}\n{{\"schemaVersion\":1,\"status\":\"completed\",\"summary\":\"Made the requested change\",\"changedFiles\":[],\"checks\":[],\"findings\":[],\"nextAction\":\"none\"}}\n{HANDOFF_END}"
    );
    assert!(extract_phase_handoff(&value, Phase::Build).is_ok());
    assert!(extract_phase_handoff(&value, Phase::Review).is_err());
}

#[test]
fn referential_ship_objectives_receive_recent_room_context() {
    assert!(objective_needs_room_context("@codex can you add it now"));
    assert!(objective_needs_room_context("Go ahead and fix that"));
    assert!(!objective_needs_room_context(
        "Implement a retry button in the room header and preserve existing state."
    ));

    let handoff = AgentHandoff {
        schema_version: 1,
        status: "blocked".to_owned(),
        summary: "The requested change was not specified.".to_owned(),
        changed_files: vec![],
        checks: vec![],
        findings: vec!["Recent context was unavailable.".to_owned()],
        next_action: "Carry the prior chat into Ship.".to_owned(),
    };
    let reason = handoff_attention_reason(&handoff);
    assert!(reason.contains("requested change was not specified"));
    assert!(reason.contains("Recent context was unavailable"));
    assert!(reason.contains("Next action: Carry the prior chat into Ship"));
}

#[test]
fn unavailable_providers_do_not_claim_execution_capabilities() {
    let participant = capabilities_for("cursor", false, None, "", "");
    assert!(!participant.non_interactive_turn);
    assert_eq!(participant.autonomy_mode, "unavailable");
}

#[test]
fn installed_providers_without_command_proof_fail_closed() {
    for (kind, version) in [
        ("codex", "codex-cli 0.144.4"),
        ("claude", "2.1.221 (Claude Code)"),
        ("cursor", "2026.08.04-aaa8809"),
        ("antigravity", "1.1.11"),
    ] {
        for help in ["", "renamed help", "--print-renamed --sandbox-renamed"] {
            let capabilities = capabilities_for(kind, true, Some(version), help, "");
            assert!(!capabilities.non_interactive_turn, "{kind}: {help}");
            assert!(!capabilities.write_mode, "{kind}: {help}");
            assert_eq!(capabilities.autonomy_mode, "manual");
        }
    }
}

#[test]
fn supported_command_contract_requires_exact_version_and_each_flag() {
    let help = "exec --ask-for-approval never --sandbox read-only workspace-write --cd --config --model";
    let exec_help = "resume --json --output-last-message";
    let supported = capabilities_for("codex", true, Some("codex-cli 0.144.4"), help, exec_help);
    assert!(supported.non_interactive_turn);
    assert!(supported.write_mode);
    for token in help.split_whitespace() {
        let incomplete = help.split_whitespace().filter(|value| *value != token).collect::<Vec<_>>().join(" ");
        assert!(!capabilities_for("codex", true, Some("codex-cli 0.144.4"), &incomplete, exec_help).write_mode, "{token}");
    }
    for version in [None, Some("codex-cli 999.0.0"), Some("garbage")] {
        assert!(!capabilities_for("codex", true, version, help, exec_help).non_interactive_turn);
    }
    assert!(!capabilities_for("codex", false, Some("codex-cli 0.144.4"), help, exec_help).write_mode);
    assert!(!capabilities_for("codex", true, Some("codex-cli 0.144.4"), help, "--json-renamed --output-last-message resume").write_mode);
}

#[test]
fn unsupported_capabilities_reject_every_dispatch_mode() {
    let capabilities = capabilities_for("codex", true, Some("codex-cli 999.0.0"), "", "");
    for mode in [ProviderMode::Ask, ProviderMode::Probe, ProviderMode::Review, ProviderMode::QuickEdit, ProviderMode::Ship] {
        assert!(require_provider_mode("codex", &capabilities, mode).is_err());
    }
}

#[test]
fn known_read_only_contracts_do_not_authorize_writes() {
    // Synthetic help fixtures exercise every required token. These are not
    // transcripts of a live installed CLI or evidence of OS containment.
    for (kind, version, help) in [
        ("claude", "2.1.221 (Claude Code)", "--print --exclude-dynamic-system-prompt-sections --permission-mode (plan, auto) --tools --verbose --output-format (text, json, stream-json) --include-partial-messages --model --effort --append-system-prompt --resume --session-id"),
        ("cursor", "2026.08.04-aaa8809", "--print --output-format (text, json, stream-json) --stream-partial-output --workspace --trust --mode (ask, plan) --model --resume create-chat --sandbox (enabled, disabled)"),
    ] {
        let capabilities = capabilities_for(kind, true, Some(version), help, "");
        assert!(capabilities.non_interactive_turn, "{kind}");
        assert!(!capabilities.write_mode, "{kind}");
        for mode in [ProviderMode::Ask, ProviderMode::Probe, ProviderMode::Review] {
            assert!(require_provider_mode(kind, &capabilities, mode).is_ok());
        }
        for mode in [ProviderMode::QuickEdit, ProviderMode::Ship] {
            assert!(require_provider_mode(kind, &capabilities, mode).is_err());
        }
        let missing = help.replace("--print", "--print-renamed");
        assert!(!capabilities_for(kind, true, Some(version), &missing, "").non_interactive_turn);
    }
}

#[test]
fn antigravity_probe_includes_the_cli_installer_location() {
    assert!(provider_fallback_paths("antigravity")
        .iter()
        .any(|path| path.ends_with(Path::new("agy").join("bin").join("agy.exe"))));
}

#[test]
fn model_discovery_keeps_selectable_lines_and_skips_headings() {
    let models = parse_provider_model_list(
        "Fetching available models...\n\
         gemini-3.1-pro-high\tGemini 3.1 Pro (High)\n\
         gemini-3.5-flash-low\tGemini 3.5 Flash (Low)\n\
         Use /model to choose\n",
    );
    assert_eq!(
        models,
        vec![
            "gemini-3.1-pro-high".to_owned(),
            "gemini-3.5-flash-low".to_owned(),
        ]
    );
}

#[test]
fn codex_model_catalog_uses_current_visible_ids_and_skips_hidden_entries() {
    let response = serde_json::json!({
        "result": {
            "data": [
                {"id": "gpt-5.6-sol", "hidden": false},
                {"model": "gpt-5.6-terra", "hidden": false},
                {"id": "legacy-hidden", "hidden": true}
            ]
        }
    });
    assert_eq!(
        parse_codex_model_catalog(&response).expect("parse Codex model catalogue"),
        vec!["gpt-5.6-sol".to_owned(), "gpt-5.6-terra".to_owned()]
    );
}

#[test]
fn cursor_model_discovery_keeps_compound_presets_verbatim() {
    let models = parse_provider_model_list(
        "Available models\n\
         auto - Auto (current, default)\n\
         claude-opus-5-thinking-high - Opus 5 1M Thinking\n\
         claude-opus-5-thinking-high-fast - Opus 5 1M Thinking Fast\n\
         claude-opus-5-low - Opus 5 1M Low\n\
         claude-opus-4-8-thinking-xhigh-fast - Opus 4.8 Extra High Thinking Fast\n\
         gpt-5.6-terra-extra-high-fast - GPT-5.6 Terra Extra High Fast\n\
         sonnet-4-thinking-fast - Sonnet 4 Thinking Fast\n\
         composer-2.5-fast - Composer 2.5 Fast\n",
    );
    assert_eq!(
        models,
        vec![
            "auto".to_owned(),
            "claude-opus-4-8-thinking-xhigh-fast".to_owned(),
            "claude-opus-5-low".to_owned(),
            "claude-opus-5-thinking-high".to_owned(),
            "claude-opus-5-thinking-high-fast".to_owned(),
            "composer-2.5-fast".to_owned(),
            "gpt-5.6-terra-extra-high-fast".to_owned(),
            "sonnet-4-thinking-fast".to_owned(),
        ]
    );
}

#[test]
fn objective_terms_ignore_noise_and_normalize_case() {
    let terms = objective_terms("Use $Frontend-Design for THE review");
    assert!(terms.contains("frontend-design"));
    assert!(terms.contains("review"));
    assert!(!terms.contains("the"));
}
