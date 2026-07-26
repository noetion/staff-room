use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    ffi::OsStr,
    path::{Path, PathBuf},
    process::Command as StdCommand,
    sync::Mutex,
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_notification::NotificationExt;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
    sync::{watch, Mutex as AsyncMutex},
    time::sleep,
};
use uuid::Uuid;

const CONTEXT_BUDGET_BYTES: usize = 48 * 1024;
const SOURCE_BUDGET_BYTES: usize = 16 * 1024;
const PROCESS_OUTPUT_LIMIT: usize = 2 * 1024 * 1024;
const PROCESS_TIMEOUT_SECONDS: u64 = 20 * 60;
const PROCESS_IDLE_TIMEOUT_SECONDS: u64 = 5 * 60;
const MAX_RECOVERY_ATTEMPTS: u32 = 2;
const MAX_SELECTED_SKILLS: usize = 3;
const HANDOFF_START: &str = "AGENT_ROOM_RESULT_START";
const HANDOFF_END: &str = "AGENT_ROOM_RESULT_END";

struct Database(Mutex<Connection>);

#[derive(Default)]
struct RuntimeState {
    cancellations: AsyncMutex<HashMap<String, watch::Sender<bool>>>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ProviderCapabilities {
    non_interactive_turn: bool,
    streaming: bool,
    structured_output: bool,
    exact_resume: bool,
    cancellation: bool,
    write_mode: bool,
    approval_bridge: bool,
    usage_reporting: bool,
    repository_scoping: bool,
    autonomy_mode: String,
    autonomy_note: String,
    capability_proof: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Participant {
    kind: String,
    name: String,
    installed: bool,
    version: Option<String>,
    executable_path: Option<String>,
    state: String,
    capabilities: ProviderCapabilities,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeEnvironment {
    native: bool,
    repository_path: String,
    branch: String,
    participants: Vec<Participant>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct RunEvent {
    run_id: String,
    event_type: String,
    phase: String,
    state: String,
    agent: Option<String>,
    title: String,
    detail: String,
    context_bytes: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct VerificationResult {
    label: String,
    status: String,
    detail: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StartRunResult {
    run_id: String,
    state: String,
    summary: String,
    builder: String,
    reviewer: String,
    degraded_review: bool,
    session_id: Option<String>,
    changed_files: Vec<String>,
    git_status: String,
    verification: Vec<VerificationResult>,
    stopped: bool,
    promoted: bool,
    worktree_path: Option<String>,
    branch: String,
    context_bytes: usize,
    attention_reason: Option<String>,
    artifact_path: Option<String>,
    instruction_files: Vec<String>,
    skill_files: Vec<String>,
    recovery_count: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartRunRequest {
    run_id: String,
    project_id: String,
    objective: String,
    repository_path: String,
    requested_agent: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectInput {
    id: String,
    name: String,
    goal: String,
    repository_path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredMessage {
    id: String,
    kind: String,
    sender: String,
    body: String,
    created_at: String,
    run_id: Option<String>,
    changed_files: Vec<String>,
    verification: Vec<VerificationResult>,
    reason: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredRun {
    id: String,
    objective: String,
    state: String,
    current_owner: Option<String>,
    writer: Option<String>,
    reviewer: Option<String>,
    review_count: u32,
    revision_count: u32,
    started_at: String,
    stop_reason: Option<String>,
    native_session_id: Option<String>,
    worktree_path: Option<String>,
    branch: Option<String>,
    context_bytes: usize,
    degraded_review: bool,
    artifact_path: Option<String>,
    instruction_files: Vec<String>,
    skill_files: Vec<String>,
    recovery_count: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RoomSnapshot {
    messages: Vec<StoredMessage>,
    latest_run: Option<StoredRun>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Build,
    Review,
    Revise,
    FinalReview,
}

impl Phase {
    fn as_str(self) -> &'static str {
        match self {
            Self::Build => "build",
            Self::Review => "review",
            Self::Revise => "revise",
            Self::FinalReview => "final-review",
        }
    }

    fn writes(self) -> bool {
        matches!(self, Self::Build | Self::Revise)
    }
}

#[derive(Debug)]
struct ProviderRun {
    summary: String,
    session_id: Option<String>,
    success: bool,
    stopped: bool,
    timed_out: bool,
    idle_timed_out: bool,
    stderr: String,
    handoff: Option<AgentHandoff>,
    stdout_log_path: String,
    stderr_log_path: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AgentHandoff {
    schema_version: u32,
    status: String,
    summary: String,
    #[serde(default)]
    changed_files: Vec<String>,
    #[serde(default)]
    checks: Vec<String>,
    #[serde(default)]
    findings: Vec<String>,
    next_action: String,
}

#[derive(Debug)]
struct SelectedSkill {
    relative_path: String,
    name: String,
    description: String,
    score: usize,
}

fn migrate(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "
        PRAGMA journal_mode = WAL;
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS projects (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            goal TEXT NOT NULL,
            repository_path TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS messages (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            run_id TEXT,
            sender_kind TEXT NOT NULL,
            message_kind TEXT NOT NULL,
            body TEXT NOT NULL,
            changed_files_json TEXT NOT NULL DEFAULT '[]',
            verification_json TEXT NOT NULL DEFAULT '[]',
            reason TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY(project_id) REFERENCES projects(id)
        );

        CREATE TABLE IF NOT EXISTS runs (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            objective TEXT NOT NULL,
            state TEXT NOT NULL,
            current_owner TEXT,
            writer TEXT,
            reviewer TEXT,
            review_count INTEGER NOT NULL DEFAULT 0,
            revision_count INTEGER NOT NULL DEFAULT 0,
            native_session_id TEXT,
            worktree_path TEXT,
            branch TEXT,
            base_head TEXT,
            context_bytes INTEGER NOT NULL DEFAULT 0,
            degraded_review INTEGER NOT NULL DEFAULT 0,
            artifact_path TEXT,
            instruction_files_json TEXT NOT NULL DEFAULT '[]',
            skill_files_json TEXT NOT NULL DEFAULT '[]',
            recovery_count INTEGER NOT NULL DEFAULT 0,
            stop_reason TEXT,
            started_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            finished_at TEXT,
            FOREIGN KEY(project_id) REFERENCES projects(id)
        );

        CREATE TABLE IF NOT EXISTS provider_sessions (
            project_id TEXT NOT NULL,
            participant_kind TEXT NOT NULL,
            provider_session_id TEXT NOT NULL,
            last_used_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY(project_id, participant_kind),
            FOREIGN KEY(project_id) REFERENCES projects(id)
        );

        CREATE TABLE IF NOT EXISTS activations (
            id TEXT PRIMARY KEY,
            run_id TEXT NOT NULL,
            phase TEXT NOT NULL,
            participant_kind TEXT NOT NULL,
            state TEXT NOT NULL,
            session_id TEXT,
            context_bytes INTEGER NOT NULL DEFAULT 0,
            summary TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            finished_at TEXT,
            FOREIGN KEY(run_id) REFERENCES runs(id)
        );

        CREATE TABLE IF NOT EXISTS handoffs (
            id TEXT PRIMARY KEY,
            run_id TEXT NOT NULL,
            phase TEXT NOT NULL,
            participant_kind TEXT NOT NULL,
            handoff_json TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY(run_id) REFERENCES runs(id)
        );
        ",
    )?;

    for statement in [
        "ALTER TABLE messages ADD COLUMN changed_files_json TEXT NOT NULL DEFAULT '[]'",
        "ALTER TABLE messages ADD COLUMN verification_json TEXT NOT NULL DEFAULT '[]'",
        "ALTER TABLE messages ADD COLUMN reason TEXT",
        "ALTER TABLE runs ADD COLUMN writer TEXT",
        "ALTER TABLE runs ADD COLUMN reviewer TEXT",
        "ALTER TABLE runs ADD COLUMN review_count INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE runs ADD COLUMN revision_count INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE runs ADD COLUMN native_session_id TEXT",
        "ALTER TABLE runs ADD COLUMN worktree_path TEXT",
        "ALTER TABLE runs ADD COLUMN branch TEXT",
        "ALTER TABLE runs ADD COLUMN base_head TEXT",
        "ALTER TABLE runs ADD COLUMN context_bytes INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE runs ADD COLUMN degraded_review INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE runs ADD COLUMN artifact_path TEXT",
        "ALTER TABLE runs ADD COLUMN instruction_files_json TEXT NOT NULL DEFAULT '[]'",
        "ALTER TABLE runs ADD COLUMN skill_files_json TEXT NOT NULL DEFAULT '[]'",
        "ALTER TABLE runs ADD COLUMN recovery_count INTEGER NOT NULL DEFAULT 0",
    ] {
        let _ = connection.execute(statement, []);
    }
    Ok(())
}

fn command_output<I, S>(
    executable: &Path,
    args: I,
    working_directory: Option<&Path>,
) -> Option<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = StdCommand::new(executable);
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

fn find_executable(names: &[&str]) -> Option<PathBuf> {
    names.iter().find_map(|name| which::which(name).ok())
}

fn provider_fallback_paths(kind: &str) -> Vec<PathBuf> {
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

fn find_provider_executable(kind: &str) -> Option<PathBuf> {
    let (_, names) = provider_names(kind);
    find_executable(names).or_else(|| {
        provider_fallback_paths(kind)
            .into_iter()
            .find(|path| path.is_file())
    })
}

fn antigravity_desktop_present() -> bool {
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

fn provider_names(kind: &str) -> (&'static str, &'static [&'static str]) {
    match kind {
        "codex" => ("Codex", &["codex"]),
        "claude" => ("Claude Code", &["claude"]),
        "cursor" => ("Cursor Agent", &["cursor-agent"]),
        "antigravity" => ("Antigravity", &["agy"]),
        _ => ("Unknown", &[]),
    }
}

fn has_help(help: &str, value: &str) -> bool {
    help.to_ascii_lowercase()
        .contains(&value.to_ascii_lowercase())
}

fn capabilities_for(
    kind: &str,
    installed: bool,
    version: Option<&str>,
    help: &str,
    subcommand_help: &str,
) -> ProviderCapabilities {
    let version_text = version.unwrap_or("version unavailable");
    match kind {
        "codex" => {
            let non_interactive = installed && has_help(help, "exec");
            let approval = has_help(help, "--ask-for-approval");
            let sandbox = has_help(help, "--sandbox");
            let streaming = has_help(subcommand_help, "--json");
            let exact_resume = has_help(subcommand_help, "resume");
            let output = has_help(subcommand_help, "--output-last-message");
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
                repository_scoping: has_help(help, "--cd") || has_help(help, "-c, --cd"),
                autonomy_mode: if ready {
                    "isolated-auto"
                } else if installed {
                    "manual"
                } else {
                    "unavailable"
                }
                .to_owned(),
                autonomy_note: if ready {
                    "Live help proves non-interactive exec, never-ask approval, workspace sandboxing, JSONL, output capture, and resume."
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
            let non_interactive = installed && has_help(help, "--print");
            let streaming = has_help(help, "--output-format") && has_help(help, "stream-json");
            let approval = has_help(help, "--permission-mode") && has_help(help, "\"auto\"");
            let exact_resume = has_help(help, "--resume");
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
                autonomy_mode: if ready {
                    "reviewed-auto"
                } else if installed {
                    "manual"
                } else {
                    "unavailable"
                }
                .to_owned(),
                autonomy_note: if ready {
                    "Live help proves print mode, stream JSON, native auto permission mediation, and session resume. Agent Room supplies the outer time and revision bounds."
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
            let non_interactive = installed && has_help(help, "--print");
            let streaming = has_help(help, "--output-format") && has_help(help, "stream-json");
            let write_mode = has_help(help, "--force") || has_help(help, "--yolo");
            let exact_resume = has_help(help, "--resume");
            let sandbox = has_help(help, "--sandbox");
            let ready = non_interactive && streaming && write_mode && exact_resume;
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
                autonomy_mode: if ready {
                    "isolated-auto"
                } else if installed {
                    "manual"
                } else {
                    "unavailable"
                }
                .to_owned(),
                autonomy_note: if ready {
                    "Live help proves print mode, stream JSON, resume, force writes, and an explicit sandbox. Force is confined to the managed worktree."
                } else if installed {
                    "Cursor Agent is installed, but this version does not prove every required automation flag."
                } else {
                    "The Cursor editor alone is not the Cursor Agent automation CLI."
                }
                .to_owned(),
                capability_proof: vec![
                    format!("Version: {version_text}"),
                    format!("print + stream JSON: {}", non_interactive && streaming),
                    format!("force write: {write_mode}"),
                    format!("resume + sandbox: {}", exact_resume && sandbox),
                ],
            }
        }
        "antigravity" => {
            let non_interactive = installed && has_help(help, "--print");
            let sandbox = has_help(help, "--sandbox");
            let write_mode = has_help(help, "--dangerously-skip-permissions");
            let exact_resume = has_help(help, "--conversation");
            let ready = non_interactive && sandbox && write_mode;
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
                autonomy_mode: if ready {
                    "unattended-bypass"
                } else if installed {
                    "manual"
                } else {
                    "unavailable"
                }
                .to_owned(),
                autonomy_note: if ready {
                    "Live help proves print mode and sandboxed unattended writes. Permission bypass remains a visible downgrade."
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
            autonomy_mode: "unavailable".to_owned(),
            autonomy_note: "Unknown provider.".to_owned(),
            capability_proof: vec![],
        },
    }
}

fn probe_provider(kind: &str) -> Participant {
    let (name, _) = provider_names(kind);
    let path = find_provider_executable(kind);
    let version = path
        .as_deref()
        .and_then(|executable| command_output(executable, ["--version"], None))
        .and_then(|value| value.lines().next().map(str::to_owned));
    let help = path
        .as_deref()
        .and_then(|executable| command_output(executable, ["--help"], None))
        .unwrap_or_default();
    let subcommand_help = if kind == "codex" {
        path.as_deref()
            .and_then(|executable| command_output(executable, ["exec", "--help"], None))
            .unwrap_or_default()
    } else {
        String::new()
    };
    let installed = path.is_some();
    let capabilities =
        capabilities_for(kind, installed, version.as_deref(), &help, &subcommand_help);
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
        state: if ready {
            "ready"
        } else if installed {
            "manual"
        } else {
            "unavailable"
        }
        .to_owned(),
        capabilities,
    }
}

fn all_participants() -> Vec<Participant> {
    ["codex", "claude", "cursor", "antigravity"]
        .iter()
        .map(|kind| probe_provider(kind))
        .collect()
}

fn git(repository: &Path, args: &[String]) -> Result<String, String> {
    let executable = find_executable(&["git"]).ok_or_else(|| "Git is not installed.".to_owned())?;
    let safe = format!("safe.directory={}", repository.to_string_lossy());
    let output = StdCommand::new(executable)
        .arg("-c")
        .arg(safe)
        .args(args)
        .current_dir(repository)
        .output()
        .map_err(|error| format!("Failed to run Git: {error}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if output.status.success() {
        Ok(stdout)
    } else {
        Err(if stderr.is_empty() {
            format!("Git command failed with status {}", output.status)
        } else {
            stderr
        })
    }
}

fn git_static(repository: &Path, args: &[&str]) -> Result<String, String> {
    git(
        repository,
        &args
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>(),
    )
}

fn git_status(repository: &Path) -> String {
    git_static(repository, &["status", "--short", "--branch"])
        .unwrap_or_else(|error| format!("Git evidence unavailable: {error}"))
}

fn changed_files(repository: &Path, base_head: &str) -> Vec<String> {
    let mut files = BTreeSet::new();
    if let Ok(output) = git_static(repository, &["status", "--short"]) {
        for line in output.lines() {
            if let Some(path) = line.get(3..) {
                if !path.trim().is_empty() {
                    files.insert(path.trim().to_owned());
                }
            }
        }
    }
    if let Ok(output) = git_static(repository, &["diff", "--name-only", base_head, "HEAD"]) {
        files.extend(
            output
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(str::to_owned),
        );
    }
    files.into_iter().collect()
}

fn truncate_utf8(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_owned();
    }
    const MARKER: &str = "\n[truncated by Agent Room]";
    if max_bytes <= MARKER.len() {
        let mut end = max_bytes;
        while end > 0 && !value.is_char_boundary(end) {
            end -= 1;
        }
        return value[..end].to_owned();
    }
    let mut end = max_bytes - MARKER.len();
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{}", &value[..end], MARKER)
}

fn append_capped(output: &mut String, line: &str) {
    if output.len() >= PROCESS_OUTPUT_LIMIT {
        return;
    }
    let remaining = PROCESS_OUTPUT_LIMIT - output.len();
    let value = format!("{line}\n");
    output.push_str(&truncate_utf8(&value, remaining));
}

fn read_context_file(path: &Path, max_bytes: usize) -> String {
    std::fs::read_to_string(path)
        .map(|value| truncate_utf8(&value, max_bytes))
        .unwrap_or_default()
}

fn relative_path(repository: &Path, path: &Path) -> String {
    path.strip_prefix(repository)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn collect_named_files(
    repository: &Path,
    directory: &Path,
    names: &[&str],
    depth: usize,
    maximum: usize,
    files: &mut Vec<PathBuf>,
) {
    if depth > 8 || files.len() >= maximum {
        return;
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        if files.len() >= maximum {
            break;
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_file() && names.iter().any(|candidate| *candidate == name) {
            files.push(path);
        } else if file_type.is_dir()
            && !matches!(
                name.as_str(),
                ".git" | "node_modules" | "target" | "dist" | ".vite" | "artifacts"
            )
            && path.starts_with(repository)
        {
            collect_named_files(repository, &path, names, depth + 1, maximum, files);
        }
    }
}

fn discover_instruction_files(repository: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_named_files(
        repository,
        repository,
        &["AGENTS.md", "CLAUDE.md"],
        0,
        80,
        &mut files,
    );
    files.sort_by_key(|path| relative_path(repository, path));
    files
}

fn instruction_applies(instruction: &Path, repository: &Path, changed_files: &[String]) -> bool {
    let directory = instruction.parent().unwrap_or(repository);
    if directory == repository {
        return true;
    }
    let prefix = relative_path(repository, directory);
    changed_files.iter().any(|changed| {
        let normalized = changed.replace('\\', "/");
        normalized == prefix || normalized.starts_with(&format!("{prefix}/"))
    })
}

fn repository_instructions(repository: &Path, changed_files: &[String]) -> (String, Vec<String>) {
    let discovered = discover_instruction_files(repository);
    let paths = discovered
        .iter()
        .map(|path| relative_path(repository, path))
        .collect::<Vec<_>>();
    let mut sections = discovered
        .iter()
        .filter(|path| instruction_applies(path, repository, changed_files))
        .map(|path| {
            let relative = relative_path(repository, path);
            format!(
                "## {relative}\n{}",
                read_context_file(path, SOURCE_BUDGET_BYTES)
            )
        })
        .collect::<Vec<_>>();
    let nested = discovered
        .iter()
        .filter(|path| !instruction_applies(path, repository, changed_files))
        .map(|path| format!("- {}", relative_path(repository, path)))
        .collect::<Vec<_>>();
    if !nested.is_empty() {
        sections.push(format!(
            "## Nested instruction inventory\nOpen and follow a nested instruction file before editing files in its directory scope:\n{}",
            nested.join("\n")
        ));
    }
    (sections.join("\n\n"), paths)
}

fn parse_skill_metadata(path: &Path) -> (String, String) {
    let content = read_context_file(path, 4 * 1024);
    let mut name = path
        .parent()
        .and_then(Path::file_name)
        .and_then(OsStr::to_str)
        .unwrap_or("project-skill")
        .to_owned();
    let mut description = String::new();
    if content.starts_with("---") {
        for line in content.lines().skip(1) {
            let trimmed = line.trim();
            if trimmed == "---" {
                break;
            }
            if let Some(value) = trimmed.strip_prefix("name:") {
                name = value.trim().trim_matches(['"', '\'']).to_owned();
            } else if let Some(value) = trimmed.strip_prefix("description:") {
                description = value.trim().trim_matches(['"', '\'']).to_owned();
            }
        }
    }
    (name, description)
}

fn objective_terms(objective: &str) -> HashSet<String> {
    const STOP_WORDS: &[&str] = &[
        "and", "for", "from", "into", "the", "this", "that", "then", "use", "with",
    ];
    objective
        .split(|character: char| !character.is_alphanumeric() && character != '-')
        .map(str::to_ascii_lowercase)
        .filter(|term| term.len() >= 3 && !STOP_WORDS.contains(&term.as_str()))
        .collect()
}

fn select_project_skills(repository: &Path, objective: &str) -> Vec<SelectedSkill> {
    let terms = objective_terms(objective);
    let objective_lower = objective.to_ascii_lowercase();
    let mut candidates = Vec::new();
    for root in [
        ".codex/skills",
        ".agents/skills",
        ".claude/skills",
        "skills",
    ] {
        let path = repository.join(root);
        if !path.is_dir() {
            continue;
        }
        let mut skill_files = Vec::new();
        collect_named_files(repository, &path, &["SKILL.md"], 0, 80, &mut skill_files);
        for skill_path in skill_files {
            let (name, description) = parse_skill_metadata(&skill_path);
            let haystack = format!("{} {}", name, description).to_ascii_lowercase();
            let mut score = terms
                .iter()
                .filter(|term| haystack.contains(term.as_str()))
                .count();
            if objective_lower.contains(&format!("${}", name.to_ascii_lowercase()))
                || objective_lower.contains(&name.to_ascii_lowercase())
            {
                score += 8;
            }
            if score > 0 {
                candidates.push(SelectedSkill {
                    relative_path: relative_path(repository, &skill_path),
                    name,
                    description,
                    score,
                });
            }
        }
    }
    candidates.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.relative_path.cmp(&right.relative_path))
    });
    candidates.truncate(MAX_SELECTED_SKILLS);
    candidates
}

fn skill_context(skills: &[SelectedSkill]) -> String {
    if skills.is_empty() {
        return String::new();
    }
    let entries = skills
        .iter()
        .map(|skill| {
            format!(
                "- `{}`: {}{}",
                skill.relative_path,
                skill.name,
                if skill.description.is_empty() {
                    String::new()
                } else {
                    format!(" — {}", skill.description)
                }
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "Agent Room selected these repository-local skills for this objective:\n{entries}\n\nBefore taking action, open every selected SKILL.md from the managed worktree and read it completely. Follow referenced resources only when the skill routes the current task to them. Provider-global skills remain provider-owned and are not copied into the context packet."
    )
}

fn project_memory(repository: &Path) -> String {
    read_context_file(
        &repository.join(".agent-room").join("memory.md"),
        SOURCE_BUDGET_BYTES / 2,
    )
}

fn assemble_packet(sections: &[(&str, String)]) -> (String, usize) {
    let mut packet = String::new();
    for (title, content) in sections {
        if content.trim().is_empty() {
            continue;
        }
        let remaining = CONTEXT_BUDGET_BYTES.saturating_sub(packet.len());
        if remaining < 128 {
            packet.push_str("\n\n[context budget reached]");
            break;
        }
        let section = format!(
            "\n\n# {title}\n{}",
            truncate_utf8(content, SOURCE_BUDGET_BYTES)
        );
        packet.push_str(&truncate_utf8(&section, remaining));
    }
    let bytes = packet.len();
    (packet.trim().to_owned(), bytes)
}

fn handoff_contract(phase: Phase) -> String {
    let statuses = if matches!(phase, Phase::Review | Phase::FinalReview) {
        "`approved` or `changes_required`"
    } else {
        "`completed`, `blocked`, or `failed`"
    };
    format!(
        "Your final response must end with exactly this machine-readable record. Do not place Markdown fences around it.\n\
         {HANDOFF_START}\n\
         {{\"schemaVersion\":1,\"status\":\"STATUS\",\"summary\":\"concise factual summary\",\"changedFiles\":[\"path\"],\"checks\":[\"command: result\"],\"findings\":[\"material finding\"],\"nextAction\":\"one explicit next action or none\"}}\n\
         {HANDOFF_END}\n\
         Allowed status values for this phase: {statuses}. All fields are required. Use empty arrays when needed."
    )
}

fn extract_handoff(value: &str) -> Result<AgentHandoff, String> {
    let start = value
        .rfind(HANDOFF_START)
        .ok_or_else(|| format!("Missing {HANDOFF_START} marker."))?;
    let body_start = start + HANDOFF_START.len();
    let remaining = &value[body_start..];
    let end = remaining
        .find(HANDOFF_END)
        .ok_or_else(|| format!("Missing {HANDOFF_END} marker."))?;
    let json = remaining[..end].trim();
    let handoff = serde_json::from_str::<AgentHandoff>(json)
        .map_err(|error| format!("Invalid handoff JSON: {error}"))?;
    if handoff.schema_version != 1 {
        return Err(format!(
            "Unsupported handoff schema version {}.",
            handoff.schema_version
        ));
    }
    if handoff.summary.trim().is_empty() || handoff.next_action.trim().is_empty() {
        return Err("Handoff summary and nextAction must be non-empty.".to_owned());
    }
    if !matches!(
        handoff.status.as_str(),
        "completed" | "blocked" | "failed" | "approved" | "changes_required"
    ) {
        return Err(format!("Unsupported handoff status `{}`.", handoff.status));
    }
    Ok(handoff)
}

fn handoff_status_allowed(handoff: &AgentHandoff, phase: Phase) -> bool {
    match phase {
        Phase::Build | Phase::Revise => {
            matches!(handoff.status.as_str(), "completed" | "blocked" | "failed")
        }
        Phase::Review | Phase::FinalReview => {
            matches!(handoff.status.as_str(), "approved" | "changes_required")
        }
    }
}

fn review_handoff_decision(run: &ProviderRun) -> Option<bool> {
    match run.handoff.as_ref()?.status.as_str() {
        "approved" => Some(true),
        "changes_required" => Some(false),
        _ => None,
    }
}

fn parse_session_id(value: &Value) -> Option<String> {
    match value {
        Value::Object(map) => {
            for key in [
                "thread_id",
                "threadId",
                "session_id",
                "sessionId",
                "conversation_id",
            ] {
                if let Some(Value::String(id)) = map.get(key) {
                    return Some(id.clone());
                }
            }
            map.values().find_map(parse_session_id)
        }
        Value::Array(values) => values.iter().find_map(parse_session_id),
        _ => None,
    }
}

fn parse_result_text(value: &Value) -> Option<String> {
    if let Value::Object(map) = value {
        if let Some(Value::String(result)) = map.get("result") {
            return Some(result.clone());
        }
        if let Some(Value::Object(message)) = map.get("message") {
            if let Some(Value::Array(content)) = message.get("content") {
                let text = content
                    .iter()
                    .filter_map(|block| block.get("text").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join("");
                if !text.is_empty() {
                    return Some(text);
                }
            }
        }
    }
    None
}

fn apply_provider_environment(command: &mut Command) {
    const EXACT: &[&str] = &[
        "PATH",
        "Path",
        "PATHEXT",
        "USERPROFILE",
        "HOME",
        "APPDATA",
        "LOCALAPPDATA",
        "PROGRAMDATA",
        "SYSTEMROOT",
        "SystemRoot",
        "TEMP",
        "TMP",
        "COMSPEC",
        "CARGO_HOME",
        "RUSTUP_HOME",
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "NO_PROXY",
    ];
    const PREFIXES: &[&str] = &[
        "CODEX_",
        "OPENAI_",
        "ANTHROPIC_",
        "CLAUDE_",
        "CURSOR_",
        "GOOGLE_",
        "GEMINI_",
        "AGY_",
    ];
    command.env_clear();
    for (key, value) in std::env::vars_os() {
        let name = key.to_string_lossy();
        if EXACT.iter().any(|candidate| *candidate == name)
            || PREFIXES.iter().any(|prefix| name.starts_with(prefix))
        {
            command.env(key, value);
        }
    }
    command.env("NO_COLOR", "1");
}

#[allow(clippy::too_many_arguments)]
fn emit_event(
    app: &AppHandle,
    run_id: &str,
    event_type: &str,
    phase: &str,
    state: &str,
    agent: Option<&str>,
    title: &str,
    detail: &str,
    context_bytes: Option<usize>,
) {
    let _ = app.emit(
        "run-event",
        RunEvent {
            run_id: run_id.to_owned(),
            event_type: event_type.to_owned(),
            phase: phase.to_owned(),
            state: state.to_owned(),
            agent: agent.map(str::to_owned),
            title: title.to_owned(),
            detail: detail.to_owned(),
            context_bytes,
        },
    );
}

async fn wait_for_idle(mut activity: watch::Receiver<u64>) {
    loop {
        tokio::select! {
            changed = activity.changed() => {
                if changed.is_err() {
                    return;
                }
            }
            _ = sleep(Duration::from_secs(PROCESS_IDLE_TIMEOUT_SECONDS)) => return,
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn invoke_provider(
    app: &AppHandle,
    run_id: &str,
    kind: &str,
    phase: Phase,
    prompt: &str,
    repository: &Path,
    session_id: Option<&str>,
    final_output_path: &Path,
    mut cancellation: watch::Receiver<bool>,
) -> Result<ProviderRun, String> {
    let executable = find_provider_executable(kind)
        .ok_or_else(|| format!("{} CLI is not installed.", provider_names(kind).0))?;
    let participant = probe_provider(kind);
    if matches!(
        participant.capabilities.autonomy_mode.as_str(),
        "manual" | "unavailable"
    ) {
        return Err(format!(
            "{} cannot prove a safe unattended mode: {}",
            participant.name, participant.capabilities.autonomy_note
        ));
    }
    let artifact_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|error| error.to_string())?
        .join("runs")
        .join(run_id);
    std::fs::create_dir_all(&artifact_dir).map_err(|error| error.to_string())?;
    let stdout_log_path = artifact_dir.join(format!("{}.stdout.log", phase.as_str()));
    let stderr_log_path = artifact_dir.join(format!("{}.stderr.log", phase.as_str()));
    let mut command = Command::new(executable);
    let mut stdin_prompt = None;

    match kind {
        "codex" => {
            command
                .arg("-a")
                .arg("never")
                .arg("-c")
                .arg("approval_policy=\"never\"")
                .arg("-s")
                .arg(if phase.writes() {
                    "workspace-write"
                } else {
                    "read-only"
                })
                .arg("-C")
                .arg(repository)
                .arg("exec");
            if let Some(id) = session_id {
                command
                    .arg("resume")
                    .arg("--json")
                    .arg("-o")
                    .arg(final_output_path)
                    .arg(id)
                    .arg("-");
            } else {
                command
                    .arg("--json")
                    .arg("-o")
                    .arg(final_output_path)
                    .arg("-");
            }
            stdin_prompt = Some(prompt.to_owned());
        }
        "claude" => {
            command
                .arg("--print")
                .arg("--verbose")
                .arg("--output-format")
                .arg("stream-json")
                .arg("--permission-mode")
                .arg("auto");
            if participant
                .capabilities
                .capability_proof
                .iter()
                .any(|proof| proof.contains("max-turns: true"))
            {
                command.arg("--max-turns").arg("30");
            }
            if let Some(id) = session_id {
                command.arg("--resume").arg(id);
            }
            command.arg("-p").arg("-");
            stdin_prompt = Some(prompt.to_owned());
        }
        "cursor" => {
            command
                .arg("--print")
                .arg("--output-format")
                .arg("stream-json");
            if phase.writes() {
                command.arg("--force");
            }
            if let Some(id) = session_id {
                command.arg("--resume").arg(id);
            }
            command.arg(prompt);
        }
        "antigravity" => {
            command.arg("--sandbox");
            if phase.writes() {
                command.arg("--dangerously-skip-permissions");
            }
            if let Some(id) = session_id {
                command.arg("--conversation").arg(id);
            }
            command.arg("--print").arg(prompt);
        }
        _ => return Err(format!("Unsupported provider: {kind}")),
    }

    apply_provider_environment(&mut command);
    command
        .current_dir(repository)
        .kill_on_drop(true)
        .stdin(if stdin_prompt.is_some() {
            std::process::Stdio::piped()
        } else {
            std::process::Stdio::null()
        })
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|error| format!("Failed to start {}: {error}", provider_names(kind).0))?;

    if let Some(content) = stdin_prompt {
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(content.as_bytes())
                .await
                .map_err(|error| format!("Failed to send the context packet: {error}"))?;
            let _ = stdin.shutdown().await;
        }
    }

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Provider stdout was unavailable.".to_owned())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Provider stderr was unavailable.".to_owned())?;

    let stdout_app = app.clone();
    let stdout_run = run_id.to_owned();
    let stdout_phase = phase.as_str().to_owned();
    let stdout_agent = kind.to_owned();
    let (activity_sender, activity_receiver) = watch::channel(0_u64);
    let stdout_activity = activity_sender.clone();
    let stdout_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        let mut session_id = None;
        let mut result_text = String::new();
        let mut raw = String::new();
        while let Ok(Some(line)) = lines.next_line().await {
            stdout_activity.send_modify(|value| *value = value.saturating_add(1));
            append_capped(&mut raw, &line);
            if let Ok(value) = serde_json::from_str::<Value>(&line) {
                session_id = session_id.or_else(|| parse_session_id(&value));
                if let Some(text) = parse_result_text(&value) {
                    result_text = text;
                }
            }
            emit_event(
                &stdout_app,
                &stdout_run,
                "stream",
                &stdout_phase,
                "running",
                Some(&stdout_agent),
                "Provider event",
                &truncate_utf8(&line, 4 * 1024),
                None,
            );
        }
        (session_id, result_text, raw)
    });

    let stderr_app = app.clone();
    let stderr_run = run_id.to_owned();
    let stderr_phase = phase.as_str().to_owned();
    let stderr_agent = kind.to_owned();
    let stderr_activity = activity_sender;
    let stderr_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        let mut raw = String::new();
        while let Ok(Some(line)) = lines.next_line().await {
            stderr_activity.send_modify(|value| *value = value.saturating_add(1));
            append_capped(&mut raw, &line);
            emit_event(
                &stderr_app,
                &stderr_run,
                "stream",
                &stderr_phase,
                "running",
                Some(&stderr_agent),
                "Provider diagnostic",
                &truncate_utf8(&line, 4 * 1024),
                None,
            );
        }
        raw
    });

    let mut stopped = false;
    let mut timed_out = false;
    let mut idle_timed_out = false;
    let status = tokio::select! {
        result = child.wait() => result.map_err(|error| error.to_string())?,
        _ = cancellation.changed() => {
            stopped = true;
            let _ = child.kill().await;
            child.wait().await.map_err(|error| error.to_string())?
        },
        _ = sleep(Duration::from_secs(PROCESS_TIMEOUT_SECONDS)) => {
            timed_out = true;
            let _ = child.kill().await;
            child.wait().await.map_err(|error| error.to_string())?
        },
        _ = wait_for_idle(activity_receiver) => {
            idle_timed_out = true;
            let _ = child.kill().await;
            child.wait().await.map_err(|error| error.to_string())?
        }
    };

    let (parsed_session, parsed_result, raw_stdout) =
        stdout_task.await.map_err(|error| error.to_string())?;
    let raw_stderr = stderr_task.await.map_err(|error| error.to_string())?;
    std::fs::write(&stdout_log_path, &raw_stdout).map_err(|error| error.to_string())?;
    std::fs::write(&stderr_log_path, &raw_stderr).map_err(|error| error.to_string())?;
    let file_result = std::fs::read_to_string(final_output_path).unwrap_or_default();
    let raw_result = if !file_result.trim().is_empty() {
        file_result.trim().to_owned()
    } else if !parsed_result.trim().is_empty() {
        parsed_result.trim().to_owned()
    } else {
        raw_stdout.trim().to_owned()
    };
    let handoff_result = extract_handoff(&raw_result).and_then(|handoff| {
        if handoff_status_allowed(&handoff, phase) {
            Ok(handoff)
        } else {
            Err(format!(
                "Handoff status `{}` is invalid for {}.",
                handoff.status,
                phase.as_str()
            ))
        }
    });
    let (handoff, schema_error) = match handoff_result {
        Ok(handoff) => (Some(handoff), None),
        Err(error) => (None, Some(error)),
    };
    let summary = handoff
        .as_ref()
        .map(|value| value.summary.clone())
        .unwrap_or_else(|| truncate_utf8(&raw_result, SOURCE_BUDGET_BYTES));
    let logical_success = handoff.as_ref().is_some_and(|value| match phase {
        Phase::Build | Phase::Revise => value.status == "completed",
        Phase::Review | Phase::FinalReview => {
            matches!(value.status.as_str(), "approved" | "changes_required")
        }
    });
    let diagnostic = [
        raw_stderr.trim(),
        schema_error.as_deref().unwrap_or_default(),
    ]
    .into_iter()
    .filter(|value| !value.is_empty())
    .collect::<Vec<_>>()
    .join("\n");

    Ok(ProviderRun {
        summary,
        session_id: parsed_session,
        success: status.success() && !stopped && !timed_out && !idle_timed_out && logical_success,
        stopped,
        timed_out,
        idle_timed_out,
        stderr: diagnostic,
        handoff,
        stdout_log_path: stdout_log_path.to_string_lossy().into_owned(),
        stderr_log_path: stderr_log_path.to_string_lossy().into_owned(),
    })
}

fn provider_log_note(run: &ProviderRun) -> String {
    format!(
        "Durable logs:\nstdout: {}\nstderr: {}",
        run.stdout_log_path, run.stderr_log_path
    )
}

#[allow(clippy::too_many_arguments)]
fn persist_message(
    database: &Database,
    project_id: &str,
    run_id: &str,
    sender: &str,
    kind: &str,
    body: &str,
    changed_files: &[String],
    verification: &[VerificationResult],
    reason: Option<&str>,
) -> Result<(), String> {
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    connection
        .execute(
            "INSERT INTO messages
             (id, project_id, run_id, sender_kind, message_kind, body, changed_files_json, verification_json, reason)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                Uuid::new_v4().to_string(),
                project_id,
                run_id,
                sender,
                kind,
                body,
                serde_json::to_string(changed_files).unwrap_or_else(|_| "[]".to_owned()),
                serde_json::to_string(verification).unwrap_or_else(|_| "[]".to_owned()),
                reason
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn update_run(
    database: &Database,
    run_id: &str,
    state: &str,
    current_owner: Option<&str>,
    review_count: u32,
    revision_count: u32,
    session_id: Option<&str>,
    context_bytes: usize,
    stop_reason: Option<&str>,
    finished: bool,
) -> Result<(), String> {
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    connection
        .execute(
            "UPDATE runs SET
               state = ?1,
               current_owner = ?2,
               review_count = ?3,
               revision_count = ?4,
               native_session_id = COALESCE(?5, native_session_id),
               context_bytes = ?6,
               stop_reason = ?7,
               finished_at = CASE WHEN ?8 THEN CURRENT_TIMESTAMP ELSE finished_at END
             WHERE id = ?9",
            params![
                state,
                current_owner,
                review_count,
                revision_count,
                session_id,
                context_bytes as i64,
                stop_reason,
                finished,
                run_id
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn persist_activation(
    database: &Database,
    run_id: &str,
    phase: Phase,
    participant: &str,
    state: &str,
    session_id: Option<&str>,
    context_bytes: usize,
    summary: Option<&str>,
) -> Result<(), String> {
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    connection
        .execute(
            "INSERT INTO activations
             (id, run_id, phase, participant_kind, state, session_id, context_bytes, summary, finished_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, CASE WHEN ?5 = 'running' THEN NULL ELSE CURRENT_TIMESTAMP END)",
            params![
                Uuid::new_v4().to_string(),
                run_id,
                phase.as_str(),
                participant,
                state,
                session_id,
                context_bytes as i64,
                summary
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn save_provider_session(
    database: &Database,
    project_id: &str,
    participant: &str,
    session_id: Option<&str>,
) -> Result<(), String> {
    let Some(session_id) = session_id else {
        return Ok(());
    };
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    connection
        .execute(
            "INSERT INTO provider_sessions (project_id, participant_kind, provider_session_id)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(project_id, participant_kind) DO UPDATE SET
               provider_session_id = excluded.provider_session_id,
               last_used_at = CURRENT_TIMESTAMP",
            params![project_id, participant, session_id],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn persist_handoff(
    database: &Database,
    run_id: &str,
    phase: Phase,
    participant: &str,
    handoff: Option<&AgentHandoff>,
) -> Result<(), String> {
    let Some(handoff) = handoff else {
        return Ok(());
    };
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    connection
        .execute(
            "INSERT INTO handoffs (id, run_id, phase, participant_kind, handoff_json)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                Uuid::new_v4().to_string(),
                run_id,
                phase.as_str(),
                participant,
                serde_json::to_string(handoff).map_err(|error| error.to_string())?
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn run_artifact_directory(app: &AppHandle, run_id: &str) -> Result<PathBuf, String> {
    let path = app
        .path()
        .app_local_data_dir()
        .map_err(|error| error.to_string())?
        .join("runs")
        .join(run_id);
    std::fs::create_dir_all(&path).map_err(|error| error.to_string())?;
    Ok(path)
}

fn create_worktree(
    app: &AppHandle,
    repository: &Path,
    run_id: &str,
) -> Result<(PathBuf, String, String, String), String> {
    let base_head = git_static(repository, &["rev-parse", "HEAD"])?;
    let base_branch = git_static(repository, &["branch", "--show-current"])?;
    if base_branch.trim().is_empty() {
        return Err("Agent Room requires an attached branch, not a detached HEAD.".to_owned());
    }
    let short = run_id
        .chars()
        .filter(|value| *value != '-')
        .take(10)
        .collect::<String>();
    let branch = format!("agent-room/{short}");
    let root = app
        .path()
        .app_cache_dir()
        .map_err(|error| error.to_string())?
        .join("worktrees");
    std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    let worktree = root.join(&short);
    if worktree.exists() {
        return Err(format!(
            "Managed worktree path already exists: {}",
            worktree.to_string_lossy()
        ));
    }
    git(
        repository,
        &[
            "worktree".to_owned(),
            "add".to_owned(),
            "-b".to_owned(),
            branch.clone(),
            worktree.to_string_lossy().into_owned(),
            base_head.clone(),
        ],
    )?;
    Ok((worktree, branch, base_branch, base_head))
}

fn commit_managed_changes(worktree: &Path, objective: &str) -> Result<bool, String> {
    let status = git_static(worktree, &["status", "--porcelain"])?;
    if status.trim().is_empty() {
        return Ok(false);
    }
    git_static(worktree, &["add", "-A"])?;
    let subject = objective
        .lines()
        .next()
        .unwrap_or("Complete objective")
        .trim();
    let subject = truncate_utf8(subject, 72);
    git(
        worktree,
        &[
            "-c".to_owned(),
            "user.name=Agent Room".to_owned(),
            "-c".to_owned(),
            "user.email=agent-room@local".to_owned(),
            "commit".to_owned(),
            "-m".to_owned(),
            format!("Agent Room: {subject}"),
        ],
    )?;
    Ok(true)
}

fn detect_verification(repository: &Path) -> Vec<(String, PathBuf, Vec<String>)> {
    let mut commands = Vec::new();
    let package = repository.join("package.json");
    if package.is_file() {
        if let Ok(contents) = std::fs::read_to_string(&package) {
            if let Ok(value) = serde_json::from_str::<Value>(&contents) {
                let scripts = value.get("scripts").and_then(Value::as_object);
                let npm = find_executable(&["npm", "npm.cmd"]);
                if let (Some(scripts), Some(npm)) = (scripts, npm) {
                    for script in ["test", "build", "lint"] {
                        if scripts.contains_key(script) {
                            commands.push((
                                format!("npm {script}"),
                                npm.clone(),
                                vec!["run".to_owned(), script.to_owned()],
                            ));
                        }
                    }
                }
            }
        }
    }
    if repository.join("src-tauri").join("Cargo.toml").is_file() {
        if let Some(cargo) = find_executable(&["cargo"]) {
            commands.push((
                "cargo check".to_owned(),
                cargo,
                vec![
                    "check".to_owned(),
                    "--manifest-path".to_owned(),
                    "src-tauri/Cargo.toml".to_owned(),
                ],
            ));
        }
    } else if repository.join("Cargo.toml").is_file() {
        if let Some(cargo) = find_executable(&["cargo"]) {
            commands.push(("cargo check".to_owned(), cargo, vec!["check".to_owned()]));
        }
    }
    commands.truncate(4);
    commands
}

async fn run_verification(
    app: &AppHandle,
    run_id: &str,
    repository: &Path,
    mut cancellation: watch::Receiver<bool>,
) -> Vec<VerificationResult> {
    let commands = detect_verification(repository);
    if commands.is_empty() {
        return vec![VerificationResult {
            label: "Project checks".to_owned(),
            status: "not-run".to_owned(),
            detail: "No supported verification commands were detected.".to_owned(),
        }];
    }

    let mut results = Vec::new();
    for (label, executable, args) in commands {
        emit_event(
            app,
            run_id,
            "phase",
            "verify",
            "verifying",
            None,
            &format!("Running {label}"),
            "Verification is executed directly in the managed worktree.",
            None,
        );
        let mut command = Command::new(executable);
        command
            .args(&args)
            .current_dir(repository)
            .kill_on_drop(true)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        match command.spawn() {
            Ok(child) => {
                let outcome = tokio::select! {
                    output = child.wait_with_output() => output.ok(),
                    _ = cancellation.changed() => None,
                    _ = sleep(Duration::from_secs(10 * 60)) => None,
                };
                match outcome {
                    Some(output) => {
                        let combined = format!(
                            "{}\n{}",
                            String::from_utf8_lossy(&output.stdout),
                            String::from_utf8_lossy(&output.stderr)
                        );
                        results.push(VerificationResult {
                            label,
                            status: if output.status.success() {
                                "passed"
                            } else {
                                "failed"
                            }
                            .to_owned(),
                            detail: truncate_utf8(combined.trim(), 3 * 1024),
                        });
                    }
                    None => results.push(VerificationResult {
                        label,
                        status: if *cancellation.borrow() {
                            "not-run"
                        } else {
                            "failed"
                        }
                        .to_owned(),
                        detail: if *cancellation.borrow() {
                            "Verification stopped with the run."
                        } else {
                            "Verification exceeded the 10 minute limit."
                        }
                        .to_owned(),
                    }),
                }
            }
            Err(error) => results.push(VerificationResult {
                label,
                status: "failed".to_owned(),
                detail: format!("Failed to start verification: {error}"),
            }),
        }
        if *cancellation.borrow() {
            break;
        }
    }
    results
}

fn verification_passed(results: &[VerificationResult]) -> bool {
    !results.iter().any(|result| result.status == "failed")
}

fn verification_summary(results: &[VerificationResult]) -> String {
    results
        .iter()
        .map(|result| format!("- {}: {}\n  {}", result.label, result.status, result.detail))
        .collect::<Vec<_>>()
        .join("\n")
}

fn diff_evidence(worktree: &Path, base_head: &str) -> String {
    let stat = git_static(worktree, &["diff", "--stat", base_head, "HEAD"]).unwrap_or_default();
    let diff = git_static(
        worktree,
        &["diff", "--no-ext-diff", "--unified=2", base_head, "HEAD"],
    )
    .unwrap_or_default();
    truncate_utf8(&format!("{stat}\n\n{diff}"), SOURCE_BUDGET_BYTES * 2)
}

fn promote_worktree(
    base_repository: &Path,
    worktree: &Path,
    branch: &str,
    base_head: &str,
) -> Result<(), String> {
    let base_status = git_static(base_repository, &["status", "--porcelain"])?;
    if !base_status.trim().is_empty() {
        return Err("The base checkout changed while the run was active.".to_owned());
    }
    let current_head = git_static(base_repository, &["rev-parse", "HEAD"])?;
    if current_head != base_head {
        return Err("The base branch advanced while the run was active.".to_owned());
    }
    git(
        base_repository,
        &[
            "merge".to_owned(),
            "--ff-only".to_owned(),
            branch.to_owned(),
        ],
    )?;
    git(
        base_repository,
        &[
            "worktree".to_owned(),
            "remove".to_owned(),
            worktree.to_string_lossy().into_owned(),
        ],
    )?;
    git(
        base_repository,
        &["branch".to_owned(), "-d".to_owned(), branch.to_owned()],
    )?;
    Ok(())
}

fn notify(app: &AppHandle, title: &str, body: &str) {
    let _ = app.notification().builder().title(title).body(body).show();
}

#[allow(clippy::too_many_arguments)]
fn final_failure(
    app: &AppHandle,
    database: &Database,
    project_id: &str,
    run_id: &str,
    state: &str,
    reason: &str,
    worktree: Option<&Path>,
    review_count: u32,
    revision_count: u32,
    context_bytes: usize,
) -> Result<(), String> {
    let recovery = worktree.map(|path| path.to_string_lossy().into_owned());
    let detail = if let Some(path) = recovery.as_deref() {
        format!("{reason}\nRecoverable worktree: {path}")
    } else {
        reason.to_owned()
    };
    persist_message(
        database,
        project_id,
        run_id,
        "system",
        if state == "stopped" {
            "status"
        } else {
            "error"
        },
        if state == "stopped" {
            "The run was stopped."
        } else {
            "Agent Room needs attention."
        },
        &[],
        &[],
        Some(&detail),
    )?;
    update_run(
        database,
        run_id,
        state,
        None,
        review_count,
        revision_count,
        None,
        context_bytes,
        Some(&detail),
        true,
    )?;
    emit_event(
        app,
        run_id,
        "attention",
        "complete",
        state,
        None,
        if state == "stopped" {
            "Run stopped"
        } else {
            "Attention required"
        },
        &detail,
        Some(context_bytes),
    );
    notify(
        app,
        if state == "stopped" {
            "Agent Room stopped"
        } else {
            "Agent Room needs attention"
        },
        &truncate_utf8(&detail, 240),
    );
    Ok(())
}

#[tauri::command]
fn get_environment() -> Result<NativeEnvironment, String> {
    let current_directory = std::env::current_dir().map_err(|error| error.to_string())?;
    let repository = find_executable(&["git"])
        .and_then(|git_executable| {
            command_output(
                &git_executable,
                ["-c", "safe.directory=*", "rev-parse", "--show-toplevel"],
                Some(&current_directory),
            )
        })
        .map(PathBuf::from)
        .filter(|path| path.is_dir())
        .unwrap_or(current_directory);
    let branch = git_static(&repository, &["branch", "--show-current"]).unwrap_or_default();
    Ok(NativeEnvironment {
        native: true,
        repository_path: repository.to_string_lossy().into_owned(),
        branch,
        participants: all_participants(),
    })
}

#[tauri::command]
fn save_project(database: State<'_, Database>, project: ProjectInput) -> Result<(), String> {
    let repository = PathBuf::from(&project.repository_path);
    git_static(&repository, &["rev-parse", "--show-toplevel"])
        .map_err(|_| "The attached path is not a Git repository.".to_owned())?;
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    connection
        .execute(
            "INSERT INTO projects (id, name, goal, repository_path)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET
               name = excluded.name,
               goal = excluded.goal,
               repository_path = excluded.repository_path,
               updated_at = CURRENT_TIMESTAMP",
            params![
                project.id,
                project.name,
                project.goal,
                project.repository_path
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
fn load_room(database: State<'_, Database>, project_id: String) -> Result<RoomSnapshot, String> {
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    let mut statement = connection
        .prepare(
            "SELECT id, message_kind, sender_kind, body, created_at, run_id,
                    changed_files_json, verification_json, reason
             FROM messages WHERE project_id = ?1 ORDER BY created_at ASC LIMIT 200",
        )
        .map_err(|error| error.to_string())?;
    let messages = statement
        .query_map([&project_id], |row| {
            let changed_json: String = row.get(6)?;
            let verification_json: String = row.get(7)?;
            Ok(StoredMessage {
                id: row.get(0)?,
                kind: row.get(1)?,
                sender: row.get(2)?,
                body: row.get(3)?,
                created_at: row.get(4)?,
                run_id: row.get(5)?,
                changed_files: serde_json::from_str(&changed_json).unwrap_or_default(),
                verification: serde_json::from_str(&verification_json).unwrap_or_default(),
                reason: row.get(8)?,
            })
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    drop(statement);

    let latest_run = connection
        .query_row(
            "SELECT id, objective, state, current_owner, writer, reviewer,
                    review_count, revision_count, started_at, stop_reason,
                    native_session_id, worktree_path, branch, context_bytes, degraded_review,
                    artifact_path, instruction_files_json, skill_files_json, recovery_count
             FROM runs WHERE project_id = ?1 ORDER BY started_at DESC LIMIT 1",
            [&project_id],
            |row| {
                Ok(StoredRun {
                    id: row.get(0)?,
                    objective: row.get(1)?,
                    state: row.get(2)?,
                    current_owner: row.get(3)?,
                    writer: row.get(4)?,
                    reviewer: row.get(5)?,
                    review_count: row.get(6)?,
                    revision_count: row.get(7)?,
                    started_at: row.get(8)?,
                    stop_reason: row.get(9)?,
                    native_session_id: row.get(10)?,
                    worktree_path: row.get(11)?,
                    branch: row.get(12)?,
                    context_bytes: row.get::<_, i64>(13)?.max(0) as usize,
                    degraded_review: row.get::<_, i64>(14)? != 0,
                    artifact_path: row.get(15)?,
                    instruction_files: serde_json::from_str(&row.get::<_, String>(16)?)
                        .unwrap_or_default(),
                    skill_files: serde_json::from_str(&row.get::<_, String>(17)?)
                        .unwrap_or_default(),
                    recovery_count: row.get::<_, i64>(18)?.max(0) as u32,
                })
            },
        )
        .optional()
        .map_err(|error| error.to_string())?;
    Ok(RoomSnapshot {
        messages,
        latest_run,
    })
}

#[tauri::command]
async fn stop_run(runtime: State<'_, RuntimeState>, run_id: String) -> Result<bool, String> {
    let sender = runtime.cancellations.lock().await.get(&run_id).cloned();
    Ok(sender
        .map(|value| value.send(true).is_ok())
        .unwrap_or(false))
}

#[tauri::command]
async fn start_room_run(
    app: AppHandle,
    database: State<'_, Database>,
    runtime: State<'_, RuntimeState>,
    request: StartRunRequest,
) -> Result<StartRunResult, String> {
    let database = database.inner();
    let base_repository = PathBuf::from(&request.repository_path);
    git_static(&base_repository, &["rev-parse", "--show-toplevel"])
        .map_err(|_| "The attached path is not a Git repository.".to_owned())?;

    let participants = all_participants();
    let ready = participants
        .iter()
        .filter(|participant| {
            participant.installed
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
        u32,
    );
    let existing = {
        let connection = database.0.lock().map_err(|error| error.to_string())?;
        connection
            .query_row(
                "SELECT objective, state, worktree_path, branch, base_head, writer,
                        native_session_id, recovery_count
                 FROM runs WHERE id = ?1",
                [&request.run_id],
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
                    ))
                },
            )
            .optional()
            .map_err(|error| error.to_string())?
    };
    let is_recovery = existing.is_some();
    let requested_builder = existing
        .as_ref()
        .and_then(|row: &RecoveryRow| row.5.as_deref())
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
    let (cancel_sender, cancel_receiver) = watch::channel(false);
    runtime
        .cancellations
        .lock()
        .await
        .insert(request.run_id.clone(), cancel_sender);

    let (worktree, branch, base_branch, base_head, recovery_session, recovery_count) =
        if let Some((
            stored_objective,
            state,
            worktree_path,
            stored_branch,
            stored_base_head,
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
            let current_branch =
                git_static(&base_repository, &["branch", "--show-current"]).unwrap_or_default();
            (
                path,
                stored_branch,
                current_branch,
                stored_base_head,
                session,
                prior_recovery_count + 1,
            )
        } else {
            let worktree_result = create_worktree(&app, &base_repository, &request.run_id);
            let (worktree, branch, base_branch, base_head) = match worktree_result {
                Ok(result) => result,
                Err(error) => {
                    runtime.cancellations.lock().await.remove(&request.run_id);
                    return Err(error);
                }
            };
            (worktree, branch, base_branch, base_head, None, 0)
        };
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
                "INSERT OR IGNORE INTO projects (id, name, goal, repository_path)
                 VALUES (?1, 'Agent Room', 'Remove manual context transfer between coding agents.', ?2)",
                params![request.project_id, request.repository_path],
            )
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "INSERT INTO runs
                 (id, project_id, objective, state, current_owner, writer, reviewer,
                  worktree_path, branch, base_head, degraded_review, artifact_path,
                  instruction_files_json, skill_files_json, recovery_count)
                 VALUES (?1, ?2, ?3, 'working', ?4, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 0)",
                params![
                    request.run_id,
                    request.project_id,
                    request.objective,
                    builder.kind,
                    reviewer.kind,
                    worktree.to_string_lossy(),
                    branch,
                    base_head,
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
            "{} isolated run on `{branch}` from `{base_branch}`.",
            if is_recovery {
                "Resumed the"
            } else {
                "Created an"
            }
        ),
        &[],
        &[],
        Some(&format!(
            "Managed worktree: {}\nArtifacts: {}\nInstructions: {}\nSelected skills: {}",
            worktree.to_string_lossy(),
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
        &format!("{} will build on {branch}.", builder.name),
        None,
    );

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
    let (build_packet, build_context_bytes) = assemble_packet(&[
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
    ]);
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
    let build_result = invoke_provider(
        &app,
        &request.run_id,
        &builder.kind,
        Phase::Build,
        &build_packet,
        &worktree,
        recovery_session.as_deref(),
        &build_output_path,
        cancel_receiver.clone(),
    )
    .await?;
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
        if build_result.success {
            "complete"
        } else {
            "failed"
        },
        build_result.session_id.as_deref(),
        build_context_bytes,
        Some(&truncate_utf8(&build_result.summary, 8 * 1024)),
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
        } else {
            "failed"
        };
        let reason = if build_result.stopped {
            "Stopped by you.".to_owned()
        } else if build_result.timed_out {
            "The builder exceeded the 20 minute phase limit.".to_owned()
        } else if build_result.idle_timed_out {
            "The builder produced no output for 5 minutes and was stopped.".to_owned()
        } else {
            format!(
                "{} failed before completing the build. {}",
                builder.name, build_result.stderr
            )
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
            changed_files: changed_files(&worktree, &base_head),
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
    let mut files = changed_files(&worktree, &base_head);
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
    let mut verification =
        run_verification(&app, &request.run_id, &worktree, cancel_receiver.clone()).await;
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
    let (review_instructions, _) = repository_instructions(&worktree, &files);
    let (review_packet, review_context_bytes) = assemble_packet(&[
        ("Objective", request.objective.clone()),
        ("Review assignment", review_assignment),
        ("Applicable repository instructions", review_instructions),
        ("Selected project skills", selected_skill_context.clone()),
        ("Project memory", memory.clone()),
        ("Builder handoff", review_handoff),
        (
            "Focused repository delta",
            diff_evidence(&worktree, &base_head),
        ),
    ]);
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
    let review_result = invoke_provider(
        &app,
        &request.run_id,
        &reviewer.kind,
        Phase::Review,
        &review_packet,
        &worktree,
        None,
        &review_output_path,
        cancel_receiver.clone(),
    )
    .await?;
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
            format!("The reviewer failed: {}", review_result.stderr)
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
        let revision_assignment = format!(
            "Revise the implementation once to address the review and verification evidence.\n\
             Preserve correct existing work. Do not broaden scope.\n\
             Read the selected skills and applicable instruction files before editing.\n\n\
             Review findings:\n{}\n\nVerification:\n{}\n\n{}",
            truncate_utf8(&review_result.summary, SOURCE_BUDGET_BYTES),
            verification_summary(&verification),
            handoff_contract(Phase::Revise)
        );
        let (revision_instructions, _) = repository_instructions(&worktree, &files);
        let (revision_packet, revision_context_bytes) = assemble_packet(&[
            ("Objective", request.objective.clone()),
            ("Revision assignment", revision_assignment),
            ("Applicable repository instructions", revision_instructions),
            ("Selected project skills", selected_skill_context.clone()),
            ("Current changed files", files.join("\n")),
            ("Focused delta", diff_evidence(&worktree, &base_head)),
            (
                "Run limit",
                "This is the only automatic revision. Resolve all material findings before stopping."
                    .to_owned(),
            ),
        ]);
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
        let revision_result = invoke_provider(
            &app,
            &request.run_id,
            &builder.kind,
            Phase::Revise,
            &revision_packet,
            &worktree,
            build_result.session_id.as_deref(),
            &revision_output_path,
            cancel_receiver.clone(),
        )
        .await?;
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
            revision_context_bytes,
            Some(&truncate_utf8(&revision_result.summary, 8 * 1024)),
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
                format!("The bounded revision failed: {}", revision_result.stderr)
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
                changed_files: changed_files(&worktree, &base_head),
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
        files = changed_files(&worktree, &base_head);
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
        verification =
            run_verification(&app, &request.run_id, &worktree, cancel_receiver.clone()).await;
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
        let final_assignment = format!(
            "Perform the final read-only review after one bounded revision.\n\
             Confirm whether the original material findings and verification failures are resolved.\n\
             Do not request optional improvements.\n\n{}",
            handoff_contract(Phase::FinalReview)
        );
        let (final_packet, final_context_bytes) = assemble_packet(&[
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
                diff_evidence(&worktree, &base_head),
            ),
        ]);
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
        let final_result = invoke_provider(
            &app,
            &request.run_id,
            &reviewer.kind,
            Phase::FinalReview,
            &final_packet,
            &worktree,
            review_result.session_id.as_deref(),
            &final_output_path,
            cancel_receiver.clone(),
        )
        .await?;
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
            final_context_bytes,
            Some(&truncate_utf8(&final_result.summary, 8 * 1024)),
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
                format!("Final review failed: {}", final_result.stderr)
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
    let has_changes = git_static(&worktree, &["rev-parse", "HEAD"])? != base_head;
    let promoted = if has_changes {
        match promote_worktree(&base_repository, &worktree, &branch, &base_head) {
            Ok(()) => true,
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
        let _ = git(
            &base_repository,
            &[
                "worktree".to_owned(),
                "remove".to_owned(),
                worktree.to_string_lossy().into_owned(),
            ],
        );
        let _ = git(
            &base_repository,
            &["branch".to_owned(), "-D".to_owned(), branch.clone()],
        );
        false
    };

    persist_message(
        database,
        &request.project_id,
        &request.run_id,
        "system",
        "status",
        if promoted {
            "Verification and review passed. The managed branch was fast-forwarded into the base checkout."
        } else {
            "Verification and review passed. The objective intentionally produced no repository change."
        },
        &files,
        &verification,
        if degraded_review {
            Some("Completed with a same-provider review downgrade.")
        } else {
            Some("Completed with independent provider review.")
        },
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
        if promoted {
            "Verified work was promoted safely."
        } else {
            "The reviewed objective required no repository change."
        },
        Some(max_context_bytes),
    );
    notify(
        &app,
        "Agent Room complete",
        if promoted {
            "Verified work was promoted to your base branch."
        } else {
            "The reviewed objective completed without repository changes."
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

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let data_directory = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_directory)?;
            let connection = Connection::open(data_directory.join("agent-room.db"))?;
            migrate(&connection)?;
            app.manage(Database(Mutex::new(connection)));
            app.manage(RuntimeState::default());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_environment,
            save_project,
            load_room,
            start_room_run,
            stop_run
        ])
        .run(tauri::generate_context!())
        .expect("error while running Agent Room");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_packet_is_bounded_and_explicitly_truncated() {
        let oversized = "x".repeat(CONTEXT_BUDGET_BYTES * 2);
        let (packet, bytes) = assemble_packet(&[("Large", oversized)]);
        assert!(bytes <= CONTEXT_BUDGET_BYTES);
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
    fn unavailable_providers_do_not_claim_execution_capabilities() {
        let participant = capabilities_for("cursor", false, None, "", "");
        assert!(!participant.non_interactive_turn);
        assert_eq!(participant.autonomy_mode, "unavailable");
    }

    #[test]
    fn antigravity_probe_includes_the_cli_installer_location() {
        assert!(provider_fallback_paths("antigravity")
            .iter()
            .any(|path| path.ends_with(Path::new("agy").join("bin").join("agy.exe"))));
    }

    #[test]
    fn objective_terms_ignore_noise_and_normalize_case() {
        let terms = objective_terms("Use $Frontend-Design for THE review");
        assert!(terms.contains("frontend-design"));
        assert!(terms.contains("review"));
        assert!(!terms.contains("the"));
    }
}
