use crate::*;

pub(crate) fn command_output<I, S>(
    executable: &Path,
    args: I,
    working_directory: Option<&Path>,
) -> Option<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = StdCommand::new(executable);
    hide_std_command_window(&mut command);
    command.args(args);
    if let Some(path) = working_directory {
        command.current_dir(path);
    }
    let output = command.output().ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if !stdout.is_empty() {
        Some(stdout)
    } else if !stderr.is_empty() {
        Some(stderr)
    } else {
        None
    }
}

#[cfg(windows)]
pub(crate) fn hide_std_command_window(command: &mut StdCommand) {
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
pub(crate) fn hide_std_command_window(_command: &mut StdCommand) {}

#[cfg(windows)]
pub(crate) fn hide_tokio_command_window(command: &mut Command) {
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
pub(crate) fn hide_tokio_command_window(_command: &mut Command) {}

pub(crate) fn find_executable(names: &[&str]) -> Option<PathBuf> {
    names.iter().find_map(|name| which::which(name).ok())
}

pub(crate) fn provider_fallback_paths(kind: &str) -> Vec<PathBuf> {
    let user_profile = std::env::var_os("USERPROFILE").map(PathBuf::from);
    let local_app_data = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let mut paths = Vec::new();
    match kind {
        "codex" => {
            if let Some(root) = local_app_data.as_ref() {
                paths.push(
                    root.join("Programs")
                        .join("OpenAI")
                        .join("Codex")
                        .join("bin")
                        .join("codex.exe"),
                );
            }
        }
        "claude" => {
            if let Some(root) = user_profile.as_ref() {
                paths.push(root.join(".local").join("bin").join("claude.exe"));
            }
        }
        "cursor" => {
            if let Some(root) = local_app_data.as_ref() {
                paths.push(root.join("cursor-agent").join("cursor-agent.cmd"));
                paths.push(root.join("cursor-agent").join("agent.cmd"));
            }
        }
        "antigravity" => {
            if let Some(root) = user_profile.as_ref() {
                paths.push(root.join(".local").join("bin").join("agy.exe"));
                paths.push(root.join(".antigravity").join("bin").join("agy.exe"));
            }
            if let Some(root) = local_app_data.as_ref() {
                paths.push(root.join("agy").join("bin").join("agy.exe"));
                paths.push(root.join("antigravity").join("agy.exe"));
                paths.push(
                    root.join("Programs")
                        .join("antigravity")
                        .join("bin")
                        .join("agy.exe"),
                );
            }
        }
        _ => {}
    }
    paths
}

pub(crate) fn find_provider_executable(kind: &str) -> Option<PathBuf> {
    let (_, names) = provider_names(kind);
    find_executable(names).or_else(|| {
        provider_fallback_paths(kind)
            .into_iter()
            .find(|path| path.is_file())
    })
}

pub(crate) fn antigravity_desktop_present() -> bool {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|root| {
            root.join("Programs")
                .join("antigravity")
                .join("Antigravity.exe")
                .is_file()
        })
        .unwrap_or(false)
}

pub(crate) fn provider_names(kind: &str) -> (&'static str, &'static [&'static str]) {
    match kind {
        "codex" => ("Codex", &["codex"]),
        "claude" => ("Claude Code", &["claude"]),
        "cursor" => ("Cursor Agent", &["cursor-agent"]),
        "antigravity" => ("Antigravity", &["agy"]),
        _ => ("Unknown", &[]),
    }
}

pub(crate) fn capabilities_for(
    kind: &str,
    installed: bool,
    version: Option<&str>,
    _help: &str,
    _subcommand_help: &str,
) -> ProviderCapabilities {
    let version_text = version.unwrap_or("version unavailable");
    match kind {
        "codex" => {
            let non_interactive = installed;
            let approval = installed;
            let sandbox = installed;
            let streaming = installed;
            let exact_resume = installed;
            let output = installed;
            let ready =
                non_interactive && approval && sandbox && streaming && exact_resume && output;
            ProviderCapabilities {
                non_interactive_turn: non_interactive,
                streaming,
                structured_output: streaming,
                exact_resume,
                cancellation: installed,
                write_mode: sandbox,
                approval_bridge: approval,
                usage_reporting: streaming,
                repository_scoping: installed,
                warm_session: false,
                autonomy_mode: if ready {
                    "isolated-auto"
                } else if installed {
                    "manual"
                } else {
                    "unavailable"
                }
                .to_owned(),
                autonomy_note: if ready {
                    "The verified capability table declares non-interactive exec, never-ask approval, workspace sandboxing, JSONL, output capture, and resume."
                } else if installed {
                    "Codex is installed, but this version does not prove every flag required for safe unattended work."
                } else {
                    "Install Codex CLI to enable this participant."
                }
                .to_owned(),
                capability_proof: vec![
                    format!("Version: {version_text}"),
                    format!("exec: {non_interactive}"),
                    format!("sandbox: {sandbox}"),
                    format!("never-ask approval: {approval}"),
                    format!("JSONL + resume + final output: {}", streaming && exact_resume && output),
                ],
            }
        }
        "claude" => {
            let non_interactive = installed;
            let streaming = installed;
            let approval = installed;
            let exact_resume = installed;
            let ready = non_interactive && streaming && approval && exact_resume;
            ProviderCapabilities {
                non_interactive_turn: non_interactive,
                streaming,
                structured_output: streaming,
                exact_resume,
                cancellation: installed,
                write_mode: approval,
                approval_bridge: approval,
                usage_reporting: streaming,
                repository_scoping: installed,
                warm_session: false,
                autonomy_mode: if ready {
                    "reviewed-auto"
                } else if installed {
                    "manual"
                } else {
                    "unavailable"
                }
                .to_owned(),
                autonomy_note: if ready {
                    "The verified capability table declares print mode, stream JSON, native auto permission mediation, and session resume. Agent Room supplies the outer time and revision bounds."
                } else if installed {
                    "Claude Code is installed, but its current help does not prove every unattended-mode flag."
                } else {
                    "Install Claude Code to prove auto permission mode on this machine."
                }
                .to_owned(),
                capability_proof: vec![
                    format!("Version: {version_text}"),
                    format!("print + stream JSON: {}", non_interactive && streaming),
                    format!("permissionMode auto: {approval}"),
                    format!("resume: {exact_resume}"),
                ],
            }
        }
        "cursor" => {
            let non_interactive = installed;
            let streaming = installed;
            let write_mode = installed;
            let exact_resume = installed;
            let sandbox = installed;
            let ask_mode = installed;
            let partial_stream = installed;
            let ready = non_interactive && streaming && write_mode && exact_resume && sandbox;
            ProviderCapabilities {
                non_interactive_turn: non_interactive,
                streaming,
                structured_output: streaming,
                exact_resume,
                cancellation: installed,
                write_mode,
                approval_bridge: false,
                usage_reporting: false,
                repository_scoping: sandbox,
                warm_session: false,
                autonomy_mode: if ready {
                    "isolated-auto"
                } else if installed {
                    "manual"
                } else {
                    "unavailable"
                }
                .to_owned(),
                autonomy_note: if ready {
                    "The verified capability table declares print mode, stream JSON, resume, force writes, and an explicit sandbox. Force is confined to the managed worktree."
                } else if installed {
                    "Cursor Agent is installed, but this version does not prove every required automation flag."
                } else {
                    "The Cursor editor alone is not the Cursor Agent automation CLI."
                }
                .to_owned(),
                capability_proof: vec![
                    format!("Version: {version_text}"),
                    format!("print + stream JSON: {}", non_interactive && streaming),
                    format!(
                        "read-only Chat + partial stream: {}",
                        ask_mode && sandbox && partial_stream
                    ),
                    format!("force write: {write_mode}"),
                    format!("resume + sandbox: {}", exact_resume && sandbox),
                ],
            }
        }
        "antigravity" => {
            let non_interactive = installed;
            let sandbox = installed;
            let write_mode = installed;
            let workspace = installed;
            let exact_resume = installed;
            let ready = non_interactive && sandbox && write_mode && workspace;
            ProviderCapabilities {
                non_interactive_turn: non_interactive,
                streaming: false,
                structured_output: false,
                exact_resume,
                cancellation: installed,
                write_mode,
                approval_bridge: false,
                usage_reporting: false,
                repository_scoping: sandbox,
                warm_session: false,
                autonomy_mode: if ready {
                    "unattended-bypass"
                } else if installed {
                    "manual"
                } else {
                    "unavailable"
                }
                .to_owned(),
                autonomy_note: if ready {
                    "The verified capability table declares print mode, explicit managed-worktree attachment, and sandboxed unattended execution. Permission bypass remains a visible downgrade."
                } else if antigravity_desktop_present() {
                    "Antigravity Desktop is installed, but the agy automation CLI is not present. The desktop executable is never substituted for the CLI."
                } else {
                    "Install the agy automation CLI; text-only output remains a declared downgrade."
                }
                .to_owned(),
                capability_proof: vec![
                    format!("Version: {version_text}"),
                    format!("agy automation executable: {installed}"),
                    format!("print + sandbox: {}", non_interactive && sandbox),
                    format!("explicit workspace attachment: {workspace}"),
                    format!("conversation resume: {exact_resume}"),
                ],
            }
        }
        _ => ProviderCapabilities {
            non_interactive_turn: false,
            streaming: false,
            structured_output: false,
            exact_resume: false,
            cancellation: false,
            write_mode: false,
            approval_bridge: false,
            usage_reporting: false,
            repository_scoping: false,
            warm_session: false,
            autonomy_mode: "unavailable".to_owned(),
            autonomy_note: "Unknown provider.".to_owned(),
            capability_proof: vec![],
        },
    }
}

pub(crate) fn provider_effort_options(kind: &str) -> Vec<String> {
    match kind {
        "codex" => vec!["low", "medium", "high", "xhigh", "max", "ultra"],
        "claude" | "cursor" => vec!["low", "medium", "high", "xhigh", "max"],
        "antigravity" => vec!["low", "medium", "high"],
        _ => vec![],
    }
    .into_iter()
    .map(str::to_owned)
    .collect()
}

pub(crate) fn model_options(
    kind: &str,
    executable: Option<&Path>,
) -> (Vec<String>, String, Vec<String>) {
    if executable.is_none() {
        return (
            vec![],
            "Install the CLI before choosing a model.".to_owned(),
            vec![],
        );
    };
    let effort_options = provider_effort_options(kind);
    let (models, note) = match kind {
        "codex" => (
            ["gpt-5.6-sol", "gpt-5.6-terra", "gpt-5.6-luna", "gpt-5.5"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            "Seeded Codex model IDs. Model refresh is intentionally a no-op because this CLI has no models command; enter a newly released ID directly.".to_owned(),
        ),
        "claude" => (
            [
                "claude-opus-5",
                "claude-opus-4-8",
                "claude-sonnet-5",
                "claude-fable-5",
                "opus",
                "sonnet",
                "fable",
            ]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            "Exact Claude model IDs, including the model generation and version.".to_owned(),
        ),
        "cursor" => (
            [
                "gpt-5.6-sol",
                "gpt-5.6-terra",
                "gpt-5.5",
                "gpt-5.3-codex",
                "claude-opus-5",
                "claude-opus-4-8",
                "claude-sonnet-5",
                "claude-fable-5",
                "composer-2.5",
                "cursor-grok-4.5",
            ]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            "Base model IDs only. Refresh after signing in; Agent Room groups Cursor's effort and speed presets under each model."
                .to_owned(),
        ),
        "antigravity" => (
            [
                "Gemini 3.1 Pro (high)",
                "Gemini 3.1 Pro (low)",
                "Gemini 3 Flash",
                "Claude Sonnet 4.6 (thinking)",
                "Claude Opus 4.6 (thinking)",
                "GPT-OSS-120b",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            "Documented model names. Refresh models after signing in to load the account catalogue."
                .to_owned(),
        ),
        _ => (
            vec![],
            "Enter an exact model identifier or leave blank for the provider default. Agent Room does not run account model discovery during startup."
                .to_owned(),
        ),
    };
    (models, note, effort_options)
}

pub(crate) fn parse_provider_model_list(output: &str) -> Vec<String> {
    let mut models = BTreeSet::new();
    for line in output.lines() {
        let raw_value = line
            .trim()
            .trim_start_matches(|character: char| {
                character.is_ascii_digit() || matches!(character, '.' | ')' | '-' | '*' | ' ')
            })
            .trim();
        let value = raw_value
            .split_once(" - ")
            .map(|(identifier, _)| identifier.trim())
            .unwrap_or(raw_value);
        if value.is_empty()
            || value.ends_with(':')
            || value.eq_ignore_ascii_case("models")
            || value.eq_ignore_ascii_case("available models")
            || value.starts_with("Use ")
            || value.starts_with("Run ")
        {
            continue;
        }
        if value.len() <= 160
            && value
                .chars()
                .any(|character| character.is_ascii_alphanumeric())
        {
            models.insert(value.to_owned());
        }
    }
    models.into_iter().collect()
}

pub(crate) fn normalize_cursor_model_id(model: &str) -> Option<String> {
    if model.eq_ignore_ascii_case("auto") {
        return None;
    }
    let mut segments = model.split('-').collect::<Vec<_>>();
    let mut removed_effort = false;
    loop {
        let last = segments.last().copied()?;
        match last {
            "fast" => {
                segments.pop();
            }
            "thinking"
                if removed_effort
                    || matches!(
                        segments.get(segments.len().saturating_sub(2)).copied(),
                        Some("none" | "low" | "medium" | "high" | "xhigh" | "max")
                    ) =>
            {
                segments.pop();
            }
            "none" | "low" | "medium" | "high" | "xhigh" | "max" => {
                segments.pop();
                if last == "high" && segments.last() == Some(&"extra") {
                    segments.pop();
                }
                removed_effort = true;
            }
            _ => break,
        }
    }
    (!segments.is_empty()).then(|| segments.join("-"))
}

pub(crate) fn parse_cursor_model_list(output: &str) -> Vec<String> {
    parse_provider_model_list(output)
        .into_iter()
        .filter_map(|model| normalize_cursor_model_id(&model))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(crate) fn probe_provider(kind: &str) -> Participant {
    let (name, _) = provider_names(kind);
    let path = find_provider_executable(kind);
    let version = path
        .as_deref()
        .and_then(|executable| command_output(executable, ["--version"], None))
        .and_then(|value| value.lines().next().map(str::to_owned));
    let installed = path.is_some();
    let capabilities = capabilities_for(kind, installed, version.as_deref(), "", "");
    let (models, model_discovery_note, effort_options) = model_options(kind, path.as_deref());
    let supports_effort = !effort_options.is_empty();
    let ready = !matches!(
        capabilities.autonomy_mode.as_str(),
        "manual" | "unavailable"
    );
    Participant {
        kind: kind.to_owned(),
        name: name.to_owned(),
        installed,
        version: version.clone(),
        executable_path: path
            .as_ref()
            .map(|value| value.to_string_lossy().into_owned()),
        models,
        model_discovery_note,
        supports_effort,
        effort_options,
        state: if ready {
            "ready"
        } else if installed {
            "manual"
        } else {
            "unavailable"
        }
        .to_owned(),
        connection_status: if installed {
            "unverified"
        } else {
            "not-installed"
        }
        .to_owned(),
        connection_detail: if installed {
            "Connection has not been tested in Agent Room.".to_owned()
        } else {
            "CLI executable was not found.".to_owned()
        },
        last_verified_at: None,
        capabilities,
    }
}

pub(crate) async fn cached_provider(
    runtime: &RuntimeState,
    kind: &str,
    force_refresh: bool,
) -> Result<Participant, String> {
    if !force_refresh {
        if let Some(participant) = runtime.provider_cache.lock().await.get(kind).cloned() {
            return Ok(participant);
        }
    }
    let kind_owned = kind.to_owned();
    let participant = tokio::task::spawn_blocking(move || probe_provider(&kind_owned))
        .await
        .map_err(|error| error.to_string())?;
    runtime
        .provider_cache
        .lock()
        .await
        .insert(kind.to_owned(), participant.clone());
    Ok(participant)
}

pub(crate) async fn cached_participants(
    runtime: &RuntimeState,
    force_refresh: bool,
) -> Result<Vec<Participant>, String> {
    let kinds = ["codex", "claude", "cursor", "antigravity"];
    if !force_refresh {
        let cache = runtime.provider_cache.lock().await;
        if kinds.iter().all(|kind| cache.contains_key(*kind)) {
            return kinds
                .iter()
                .map(|kind| {
                    cache
                        .get(*kind)
                        .cloned()
                        .ok_or_else(|| format!("Provider cache lost {kind}."))
                })
                .collect();
        }
    }
    let handles = kinds.map(|kind| {
        let kind = kind.to_owned();
        tokio::task::spawn_blocking(move || probe_provider(&kind))
    });
    let mut participants = Vec::new();
    for handle in handles {
        participants.push(handle.await.map_err(|error| error.to_string())?);
    }
    let mut cache = runtime.provider_cache.lock().await;
    for participant in &participants {
        cache.insert(participant.kind.clone(), participant.clone());
    }
    Ok(participants)
}

pub(crate) fn detected_connection(participant: &Participant) -> ProviderConnection {
    if !participant.installed {
        return ProviderConnection {
            status: "not-installed".to_owned(),
            detail: "CLI executable was not found.".to_owned(),
            last_verified_at: None,
        };
    }
    let status_output = match participant.kind.as_str() {
        "codex" => participant
            .executable_path
            .as_deref()
            .and_then(|path| command_output(Path::new(path), ["login", "status"], None)),
        "claude" => participant
            .executable_path
            .as_deref()
            .and_then(|path| command_output(Path::new(path), ["auth", "status"], None)),
        _ => None,
    }
    .unwrap_or_default();
    let connected = match participant.kind.as_str() {
        "codex" => status_output.to_ascii_lowercase().contains("logged in"),
        "claude" => status_output.contains("\"loggedIn\": true"),
        _ => false,
    };
    if connected {
        ProviderConnection {
            status: "unverified".to_owned(),
            detail: "Native sign-in detected. Run Test connection before using this CLI."
                .to_owned(),
            last_verified_at: None,
        }
    } else if matches!(participant.kind.as_str(), "codex" | "claude") {
        ProviderConnection {
            status: "sign-in-required".to_owned(),
            detail: "Native CLI is installed but not signed in for this desktop user.".to_owned(),
            last_verified_at: None,
        }
    } else {
        ProviderConnection {
            status: "unverified".to_owned(),
            detail: "Run Test connection before using this CLI.".to_owned(),
            last_verified_at: None,
        }
    }
}

pub(crate) fn saved_connection(
    database: &Database,
    project_id: &str,
    participant: &str,
) -> Result<Option<ProviderConnection>, String> {
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    connection
        .query_row(
            "SELECT status, detail, last_verified_at FROM provider_connections
             WHERE project_id = ?1 AND participant_kind = ?2",
            params![project_id, participant],
            |row| {
                Ok(ProviderConnection {
                    status: row.get(0)?,
                    detail: row.get(1)?,
                    last_verified_at: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(|error| error.to_string())
}

pub(crate) fn save_connection(
    database: &Database,
    project_id: &str,
    participant: &str,
    connection_state: &ProviderConnection,
) -> Result<(), String> {
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    connection
        .execute(
            "INSERT INTO provider_connections
             (project_id, participant_kind, status, detail, last_verified_at)
             VALUES (?1, ?2, ?3, ?4, CASE WHEN ?3 = 'connected' THEN CURRENT_TIMESTAMP ELSE NULL END)
             ON CONFLICT(project_id, participant_kind) DO UPDATE SET
               status = excluded.status,
               detail = excluded.detail,
               last_verified_at = excluded.last_verified_at",
            params![project_id, participant, connection_state.status, connection_state.detail],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) fn participant_for_project(
    database: &Database,
    project_id: &str,
    mut participant: Participant,
) -> Result<Participant, String> {
    let saved = saved_connection(database, project_id, &participant.kind)?;
    let connection = if !participant.installed {
        detected_connection(&participant)
    } else {
        saved.unwrap_or_else(|| detected_connection(&participant))
    };
    participant.connection_status = connection.status.clone();
    participant.connection_detail = connection.detail;
    participant.last_verified_at = connection.last_verified_at;
    participant.state = if participant.connection_status == "connected"
        && !matches!(
            participant.capabilities.autonomy_mode.as_str(),
            "manual" | "unavailable"
        ) {
        "ready"
    } else if participant.installed {
        "manual"
    } else {
        "unavailable"
    }
    .to_owned();
    Ok(participant)
}

pub(crate) fn participants_with_connections(
    database: &Database,
    project_id: &str,
    participants: Vec<Participant>,
) -> Result<Vec<Participant>, String> {
    participants
        .into_iter()
        .map(|participant| participant_for_project(database, project_id, participant))
        .collect()
}
