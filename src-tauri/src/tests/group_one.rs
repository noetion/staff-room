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
             VALUES ('agent-room', 'Agent Room', 'legacy', ?1)",
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
fn declared_antigravity_capabilities_do_not_depend_on_help_text() {
    let ready = capabilities_for(
        "antigravity",
        true,
        Some("test"),
        "--print --sandbox --dangerously-skip-permissions --add-dir",
        "",
    );
    assert_eq!(ready.autonomy_mode, "unattended-bypass");
    let renamed_help = capabilities_for("antigravity", true, Some("test"), "renamed help", "");
    assert_eq!(renamed_help.autonomy_mode, "unattended-bypass");
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
        "Available models:\n1. Gemini 3.1 Pro (High)\n2. Gemini 3.1 Flash\nUse /model to choose\n",
    );
    assert_eq!(
        models,
        vec![
            "Gemini 3.1 Flash".to_owned(),
            "Gemini 3.1 Pro (High)".to_owned(),
        ]
    );
}

#[test]
fn cursor_model_discovery_groups_compound_presets_by_base_model() {
    let models = parse_cursor_model_list(
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
            "claude-opus-4-8".to_owned(),
            "claude-opus-5".to_owned(),
            "composer-2.5".to_owned(),
            "gpt-5.6-terra".to_owned(),
            "sonnet-4-thinking".to_owned(),
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
