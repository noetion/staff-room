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
    let executable = executable.to_owned();
    let args = args.into_iter().map(|arg| arg.as_ref().to_owned()).collect::<Vec<_>>();
    let working_directory = working_directory.map(Path::to_owned);
    // Also called from synchronous database/status paths inside a Tokio task.
    // Own a short-lived runtime on a separate thread, never nest block_on.
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread().enable_all().build().ok()?
            .block_on(bounded_provider_probe(&executable, &args, working_directory.as_deref()))
            .ok()
    }).join().ok().flatten()
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

pub(crate) fn provider_names(kind: &str) -> (&'static str, &'static [&'static str]) {
    match kind {
        "codex" => ("Codex", &["codex"]),
        "claude" => ("Claude Code", &["claude"]),
        "cursor" => ("Cursor Agent", &["cursor-agent"]),
        "antigravity" => ("Antigravity", &["agy"]),
        _ => ("Unknown", &[]),
    }
}

// These exact versions are recorded in docs/providers and the v1 acceptance record.
// Help proves command syntax only; it cannot prove containment or authentication.
fn contains_all(text: &str, required: &[&str]) -> bool {
    let tokens = text.split(|c: char| c.is_whitespace() || matches!(c, ',' | '[' | ']' | '(' | ')' | '=' | '<' | '>' | ':' | '|' | '\'' | '"'))
        .filter(|token| !token.is_empty()).collect::<HashSet<_>>();
    required.iter().all(|token| tokens.contains(token))
}

pub(crate) fn capabilities_for(
    kind: &str,
    installed: bool,
    version: Option<&str>,
    help: &str,
    subcommand_help: &str,
) -> ProviderCapabilities {
    let known_version = matches!(
        (kind, version),
        ("codex", Some("codex-cli 0.144.4"))
            | ("claude", Some("2.1.221 (Claude Code)"))
            | ("cursor", Some("2026.08.04-aaa8809"))
    );
    let syntax = match kind {
        "codex" => contains_all(help, &["exec", "--ask-for-approval", "never", "--sandbox", "read-only", "workspace-write", "--cd", "--config", "--model"])
            && contains_all(subcommand_help, &["resume", "--json", "--output-last-message"]),
        "claude" => contains_all(help, &["--print", "--exclude-dynamic-system-prompt-sections", "--permission-mode", "plan", "--tools", "--verbose", "--output-format", "stream-json", "--include-partial-messages", "--model", "--effort", "--append-system-prompt", "--resume", "--session-id"]),
        "cursor" => contains_all(help, &["--print", "--output-format", "stream-json", "--stream-partial-output", "--workspace", "--trust", "--mode", "ask", "--model", "--resume", "create-chat"])
            && (cfg!(windows) || contains_all(help, &["--sandbox", "enabled"])),
        _ => false,
    };
    let supported = installed && known_version && syntax;
    let write_mode = supported && kind == "codex";
    let note = if !installed {
        "CLI executable was not found."
    } else if kind == "antigravity" {
        "Antigravity routes are disabled: terminal sandbox and permission bypass do not establish a filesystem write boundary or a read-only contract."
    } else if !known_version {
        "CLI version is not in the tested command contract table; execution is disabled."
    } else if !syntax {
        "Required CLI help contract is incomplete or unavailable; execution is disabled."
    } else if kind == "codex" {
        "Known Codex command contract: read-only or workspace-write filesystem sandbox with never-ask approvals. The installed executable remains trusted; live connection and native boundary acceptance are separate checks."
    } else if kind == "claude" {
        "Read-only tools in plan mode are supported. Quick Edit and Ship writes are disabled: auto permission mediation is not filesystem containment."
    } else {
        "Ask mode is supported. Quick Edit and Ship writes are disabled: no accepted platform filesystem write boundary (Cursor sandbox is unavailable on Windows)."
    };
    ProviderCapabilities {
        non_interactive_turn: supported,
        streaming: supported,
        structured_output: supported,
        exact_resume: supported,
        cancellation: supported,
        write_mode,
        approval_bridge: supported && kind == "codex",
        usage_reporting: supported && kind != "cursor",
        repository_scoping: supported && kind == "codex",
        warm_session: false,
        autonomy_mode: if write_mode { "isolated-auto" } else if installed { "manual" } else { "unavailable" }.to_owned(),
        autonomy_note: note.to_owned(),
        capability_proof: vec![
            format!("Version: {}", version.unwrap_or("unavailable")),
            format!("Known command version: {known_version}"),
            format!("Required help syntax: {syntax}"),
            format!("Write route supported: {write_mode}"),
            "Help checks do not verify sign-in, live readiness, filesystem containment, or network isolation.".to_owned(),
        ],
    }
}

pub(crate) fn require_provider_mode(kind: &str, capabilities: &ProviderCapabilities, mode: ProviderMode) -> Result<(), String> {
    if !capabilities.non_interactive_turn
        || (!super::is_read_only(mode) && !capabilities.write_mode)
    {
        return Err(format!("{} cannot run this route: {}", provider_names(kind).0, capabilities.autonomy_note));
    }
    Ok(())
}

pub(crate) fn provider_effort_options(kind: &str) -> Vec<String> {
    match kind {
        "codex" => vec!["low", "medium", "high", "xhigh", "max", "ultra"],
        "claude" => vec!["low", "medium", "high", "xhigh", "max"],
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
            "Seeded Codex model IDs for startup. Refresh uses the Codex app-server catalogue when available.".to_owned(),
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
            "Seeded Claude model IDs and aliases. Claude Code does not expose a non-interactive account catalogue; enter a newly released ID directly.".to_owned(),
        ),
        "cursor" => (
            vec![],
            "Refresh after signing in to load exact model identifiers from Cursor Agent. Each identifier already encodes its effort, thinking, and speed preset."
                .to_owned(),
        ),
        "antigravity" => (
            [
                "gemini-3.1-pro-high",
                "gemini-3.1-pro-low",
                "gemini-3.5-flash-low",
                "gemini-3.6-flash-low",
                "claude-sonnet-4-6",
                "claude-opus-4-6-thinking",
                "gpt-oss-120b-medium",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            "Documented model names. Refresh models after signing in to load the account catalogue."
                .to_owned(),
        ),
        _ => (
            vec![],
            "Enter an exact model identifier or leave blank for the provider default. The Staff Room does not run account model discovery during startup."
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
            .split_once('\t')
            .or_else(|| raw_value.split_once(" - "))
            .map(|(identifier, _)| identifier.trim())
            .unwrap_or(raw_value);
        if value.is_empty()
            || value.ends_with(':')
            || value.eq_ignore_ascii_case("models")
            || value.eq_ignore_ascii_case("available models")
            || value.eq_ignore_ascii_case("fetching available models...")
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

pub(crate) fn probe_provider(kind: &str) -> Participant {
    probe_provider_at(kind, find_provider_executable(kind))
}

fn probe_provider_at(kind: &str, path: Option<PathBuf>) -> Participant {
    let (name, _) = provider_names(kind);
    let version = path
        .as_deref()
        .and_then(|executable| command_output(executable, ["--version"], None))
        .and_then(|value| value.lines().next().map(str::to_owned));
    let installed = path.is_some();
    let help = path.as_deref()
        .and_then(|executable| command_output(executable, ["--help"], None))
        .unwrap_or_default();
    let subcommand_help = if kind == "codex" {
        path.as_deref().and_then(|executable| {
            let exec = command_output(executable, ["exec", "--help"], None)?;
            let resume = command_output(executable, ["exec", "resume", "--help"], None)?;
            // Both fresh and resumed turns use these options.
            if !contains_all(&resume, &["--json", "--output-last-message"]) {
                return None;
            }
            Some(exec)
        }).unwrap_or_default()
    } else { String::new() };
    let capabilities = capabilities_for(kind, installed, version.as_deref(), &help, &subcommand_help);
    let (models, model_discovery_note, effort_options) = model_options(kind, path.as_deref());
    let supports_effort = !effort_options.is_empty();
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
        state: if installed {
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
            "Connection has not been tested in The Staff Room.".to_owned()
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

#[cfg(test)]
mod probe_tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn fake_cli_detection_separates_presence_contract_and_connection() {
        let root = std::env::temp_dir().join(format!("staff-room-probe-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let executable = root.join(if cfg!(windows) { "codex.cmd" } else { "codex" });
        // Synthetic CLI fixture, never an authenticated provider call.
        let script = if cfg!(windows) {
            "@echo off\r\nif \"%*\"==\"--version\" (echo codex-cli 0.144.4& exit /b 0)\r\nif \"%*\"==\"--help\" (echo exec --ask-for-approval never --sandbox read-only workspace-write --cd --config --model& exit /b 0)\r\nif \"%*\"==\"exec --help\" (echo resume --json --output-last-message& exit /b 0)\r\nif \"%*\"==\"exec resume --help\" (echo --json --output-last-message& exit /b 0)\r\nexit /b 91\r\n"
        } else {
            "#!/bin/sh\ncase \"$*\" in\n'--version') echo 'codex-cli 0.144.4';;\n'--help') echo 'exec --ask-for-approval never --sandbox read-only workspace-write --cd --config --model';;\n'exec --help') echo 'resume --json --output-last-message';;\n'exec resume --help') echo '--json --output-last-message';;\n*) exit 91;;\nesac\n"
        };
        std::fs::write(&executable, script).unwrap();
        #[cfg(unix)]
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
        let participant = probe_provider_at("codex", Some(executable.clone()));
        assert!(participant.installed);
        assert!(participant.capabilities.write_mode);
        assert_eq!(participant.state, "manual");
        assert_eq!(participant.connection_status, "unverified");
        assert_eq!(detected_connection(&participant).status, "sign-in-required");
        // A changed resume contract must invalidate all execution even when
        // root help, version, and a previous participant remain valid.
        let changed_script = script
            .replace("echo '--json --output-last-message'", "echo '--json-renamed --output-last-message'")
            .replace("echo --json --output-last-message&", "echo --json-renamed --output-last-message&");
        std::fs::write(&executable, changed_script).unwrap();
        let changed = probe_provider_at("codex", Some(executable));
        for mode in [ProviderMode::Ask, ProviderMode::Probe, ProviderMode::Review, ProviderMode::QuickEdit, ProviderMode::Ship] {
            assert!(require_provider_mode("codex", &changed.capabilities, mode).is_err());
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
