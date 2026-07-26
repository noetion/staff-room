mod providers;

use providers::{Mode as ProviderMode, TurnRequest};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    ffi::OsStr,
    io::Read,
    path::{Path, PathBuf},
    process::Command as StdCommand,
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_notification::NotificationExt;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
    sync::{watch, Mutex as AsyncMutex},
    time::{sleep, timeout},
};
use uuid::Uuid;

#[cfg(windows)]
use std::os::windows::process::CommandExt as _;

const CONTEXT_BUDGET_BYTES: usize = 48 * 1024;
const SOURCE_BUDGET_BYTES: usize = 16 * 1024;
const PROCESS_OUTPUT_LIMIT: usize = 2 * 1024 * 1024;
const PROCESS_TIMEOUT_SECONDS: u64 = 20 * 60;
const PROCESS_IDLE_TIMEOUT_SECONDS: u64 = 5 * 60;
const MAX_RECOVERY_ATTEMPTS: u32 = 2;
const MAX_SELECTED_SKILLS: usize = 3;
const CHAT_HANDOFF_BUDGET_BYTES: usize = 4 * 1024;
const HANDOFF_START: &str = "AGENT_ROOM_RESULT_START";
const HANDOFF_END: &str = "AGENT_ROOM_RESULT_END";
const SHIP_INTENT_START: &str = "AGENT_ROOM_SHIP_INTENT_START";
const SHIP_INTENT_END: &str = "AGENT_ROOM_SHIP_INTENT_END";
const AUTONOMOUS_SHIP_SKILL_PATH: &str = ".agents/skills/autonomous-ship/SKILL.md";
const AUTONOMOUS_SHIP_SKILL: &str = include_str!("../../.agents/skills/autonomous-ship/SKILL.md");
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

struct Database(Mutex<Connection>);

#[derive(Default)]
struct RuntimeState {
    cancellations: AsyncMutex<HashMap<String, watch::Sender<bool>>>,
    provider_cache: AsyncMutex<HashMap<String, Participant>>,
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
    models: Vec<String>,
    model_discovery_note: String,
    supports_effort: bool,
    effort_options: Vec<String>,
    state: String,
    connection_status: String,
    connection_detail: String,
    last_verified_at: Option<String>,
    capabilities: ProviderCapabilities,
}

#[derive(Debug, Clone)]
struct ProviderConnection {
    status: String,
    detail: String,
    last_verified_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelDiscoveryResult {
    models: Vec<String>,
    detail: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelDiscoveryRequest {
    participant_kind: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeEnvironment {
    native: bool,
    attached: bool,
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

#[derive(Debug)]
struct IsolationContext {
    worktree: PathBuf,
    branch: String,
    base_branch: String,
    base_head: String,
    snapshot_head: String,
    isolation_kind: String,
    workspace_fingerprint: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PromotionMode {
    FastForward,
    WorkingTree,
}

#[derive(Debug)]
struct PromotionResult {
    mode: PromotionMode,
    cleanup_warning: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatRequest {
    run_id: String,
    project_id: String,
    message: String,
    repository_path: String,
    requested_agent: Option<String>,
    active_run_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatResult {
    run_id: String,
    participant: String,
    summary: String,
    session_id: Option<String>,
    actual_model: Option<String>,
    stopped: bool,
    ship_intent: Option<ShipIntent>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ShipIntent {
    schema_version: u32,
    objective: String,
    reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConnectionTestRequest {
    project_id: String,
    repository_path: String,
    participant_kind: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Project {
    id: String,
    name: String,
    goal: String,
    repository_path: String,
    branch: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct ProjectSettings {
    autonomous_ship_enabled: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectSettingsInput {
    project_id: String,
    autonomous_ship_enabled: bool,
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
    receipts: Vec<ExecutionReceipt>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct ProviderUsage {
    input_tokens: Option<u64>,
    cached_input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    total_cost_usd: Option<f64>,
    num_turns: Option<u64>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ExecutionReceipt {
    id: String,
    phase: String,
    participant: String,
    provider_version: Option<String>,
    requested_model: Option<String>,
    requested_effort: Option<String>,
    actual_model: Option<String>,
    session_id: Option<String>,
    context_bytes: usize,
    usage: ProviderUsage,
    usage_note: String,
    created_at: String,
    preflight_ms: Option<u64>,
    process_start_ms: Option<u64>,
    first_output_ms: Option<u64>,
    total_ms: Option<u64>,
    session_resumed: bool,
    packet_bytes_saved: usize,
    stdout_log_path: Option<String>,
    stderr_log_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct ProviderProfile {
    participant_kind: String,
    route: String,
    model: Option<String>,
    effort: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderProfileInput {
    project_id: String,
    participant_kind: String,
    route: String,
    model: Option<String>,
    effort: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Chat,
    Build,
    Review,
    Revise,
    FinalReview,
}

impl Phase {
    fn as_str(self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Build => "build",
            Self::Review => "review",
            Self::Revise => "revise",
            Self::FinalReview => "final-review",
        }
    }
}

fn idle_timeout_seconds(_phase: Phase) -> u64 {
    PROCESS_IDLE_TIMEOUT_SECONDS
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
    actual_model: Option<String>,
    usage: ProviderUsage,
    stdout_log_path: String,
    stderr_log_path: String,
    process_start_ms: u64,
    first_output_ms: Option<u64>,
    session_resumed: bool,
}

struct ChatReceiptMetrics {
    context_bytes: usize,
    preflight_ms: u64,
    total_ms: u64,
}

struct ReceiptMetrics {
    context_bytes: usize,
    preflight_ms: u64,
    total_ms: u64,
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
    backup_v1_database_before_migration(connection)?;
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

        CREATE TABLE IF NOT EXISTS app_state (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
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
            base_branch TEXT,
            snapshot_head TEXT,
            isolation_kind TEXT NOT NULL DEFAULT 'worktree',
            workspace_fingerprint TEXT,
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

        CREATE TABLE IF NOT EXISTS chat_sessions (
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

        CREATE TABLE IF NOT EXISTS provider_profiles (
            project_id TEXT NOT NULL,
            participant_kind TEXT NOT NULL,
            model TEXT,
            effort TEXT,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY(project_id, participant_kind),
            FOREIGN KEY(project_id) REFERENCES projects(id)
        );

        CREATE TABLE IF NOT EXISTS provider_route_profiles (
            project_id TEXT NOT NULL,
            participant_kind TEXT NOT NULL,
            route TEXT NOT NULL,
            model TEXT,
            effort TEXT,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY(project_id, participant_kind, route),
            FOREIGN KEY(project_id) REFERENCES projects(id)
        );

        CREATE TABLE IF NOT EXISTS project_settings (
            project_id TEXT PRIMARY KEY,
            autonomous_ship_enabled INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY(project_id) REFERENCES projects(id)
        );

        CREATE TABLE IF NOT EXISTS provider_connections (
            project_id TEXT NOT NULL,
            participant_kind TEXT NOT NULL,
            status TEXT NOT NULL,
            detail TEXT NOT NULL,
            last_verified_at TEXT,
            PRIMARY KEY(project_id, participant_kind),
            FOREIGN KEY(project_id) REFERENCES projects(id)
        );

        CREATE TABLE IF NOT EXISTS provider_capabilities (
            provider TEXT NOT NULL,
            version TEXT NOT NULL,
            capabilities_json TEXT NOT NULL,
            verified_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY(provider, version)
        );

        CREATE TABLE IF NOT EXISTS execution_receipts (
            id TEXT PRIMARY KEY,
            run_id TEXT NOT NULL,
            phase TEXT NOT NULL,
            participant_kind TEXT NOT NULL,
            provider_version TEXT,
            requested_model TEXT,
            requested_effort TEXT,
            actual_model TEXT,
            session_id TEXT,
            context_bytes INTEGER NOT NULL DEFAULT 0,
            usage_json TEXT NOT NULL DEFAULT '{}',
            usage_note TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY(run_id) REFERENCES runs(id)
        );

        CREATE TABLE IF NOT EXISTS chat_receipts (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            chat_id TEXT NOT NULL,
            phase TEXT NOT NULL,
            participant_kind TEXT NOT NULL,
            provider_version TEXT,
            requested_model TEXT,
            requested_effort TEXT,
            actual_model TEXT,
            session_id TEXT,
            context_bytes INTEGER NOT NULL DEFAULT 0,
            usage_json TEXT NOT NULL DEFAULT '{}',
            usage_note TEXT NOT NULL,
            preflight_ms INTEGER,
            first_output_ms INTEGER,
            total_ms INTEGER,
            stdout_log_path TEXT,
            stderr_log_path TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY(project_id) REFERENCES projects(id)
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
        "ALTER TABLE runs ADD COLUMN base_branch TEXT",
        "ALTER TABLE runs ADD COLUMN snapshot_head TEXT",
        "ALTER TABLE runs ADD COLUMN isolation_kind TEXT NOT NULL DEFAULT 'worktree'",
        "ALTER TABLE runs ADD COLUMN workspace_fingerprint TEXT",
        "ALTER TABLE runs ADD COLUMN context_bytes INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE runs ADD COLUMN degraded_review INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE runs ADD COLUMN artifact_path TEXT",
        "ALTER TABLE runs ADD COLUMN instruction_files_json TEXT NOT NULL DEFAULT '[]'",
        "ALTER TABLE runs ADD COLUMN skill_files_json TEXT NOT NULL DEFAULT '[]'",
        "ALTER TABLE runs ADD COLUMN recovery_count INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE chat_receipts ADD COLUMN preflight_ms INTEGER",
        "ALTER TABLE execution_receipts ADD COLUMN preflight_ms INTEGER",
        "ALTER TABLE execution_receipts ADD COLUMN process_start_ms INTEGER",
        "ALTER TABLE execution_receipts ADD COLUMN first_output_ms INTEGER",
        "ALTER TABLE execution_receipts ADD COLUMN total_ms INTEGER",
        "ALTER TABLE execution_receipts ADD COLUMN session_resumed INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE execution_receipts ADD COLUMN packet_bytes_saved INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE execution_receipts ADD COLUMN stdout_log_path TEXT",
        "ALTER TABLE execution_receipts ADD COLUMN stderr_log_path TEXT",
        "ALTER TABLE chat_receipts ADD COLUMN process_start_ms INTEGER",
        "ALTER TABLE chat_receipts ADD COLUMN session_resumed INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE chat_receipts ADD COLUMN packet_bytes_saved INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE chat_receipts ADD COLUMN first_output_ms INTEGER",
        "ALTER TABLE chat_receipts ADD COLUMN total_ms INTEGER",
        "ALTER TABLE chat_receipts ADD COLUMN stdout_log_path TEXT",
        "ALTER TABLE chat_receipts ADD COLUMN stderr_log_path TEXT",
    ] {
        let _ = connection.execute(statement, []);
    }
    migrate_v1_to_v2(connection)?;

    connection.execute_batch(
        "
        INSERT OR IGNORE INTO provider_route_profiles
            (project_id, participant_kind, route, model, effort)
        SELECT profile.project_id, profile.participant_kind, route.name,
               profile.model, profile.effort
        FROM provider_profiles AS profile
        CROSS JOIN (
            SELECT 'chat' AS name
            UNION ALL SELECT 'build'
            UNION ALL SELECT 'review'
        ) AS route;
        ",
    )?;
    Ok(())
}

const PROJECT_SCOPED_TABLES: &[&str] = &[
    "projects",
    "messages",
    "runs",
    "chat_sessions",
    "provider_sessions",
    "provider_profiles",
    "provider_route_profiles",
    "project_settings",
    "provider_connections",
    "chat_receipts",
];

fn project_id_for_root(repository: &Path) -> String {
    let digest = Sha256::digest(repository.to_string_lossy().as_bytes());
    digest
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn canonical_repository_root(path: &Path) -> Result<PathBuf, String> {
    let canonical_path = std::fs::canonicalize(path)
        .map_err(|error| format!("Cannot read repository path: {error}"))?;
    if !canonical_path.is_dir() {
        return Err("The attached path is not a directory.".to_owned());
    }
    let root = git_static(&canonical_path, &["rev-parse", "--show-toplevel"])
        .map_err(|_| "The attached path is not a Git repository.".to_owned())?;
    std::fs::canonicalize(root)
        .map_err(|error| format!("Cannot read Git repository root: {error}"))
}

fn project_from_parts(id: String, name: String, goal: String, repository_path: String) -> Project {
    let branch = git_static(Path::new(&repository_path), &["branch", "--show-current"])
        .unwrap_or_default();
    Project {
        id,
        name,
        goal,
        repository_path,
        branch,
    }
}

fn active_project(connection: &Connection) -> Result<Option<Project>, String> {
    connection
        .query_row(
            "SELECT projects.id, projects.name, projects.goal, projects.repository_path
             FROM app_state JOIN projects ON projects.id = app_state.value
             WHERE app_state.key = 'active_project_id'",
            [],
            |row| Ok(project_from_parts(row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(|error| error.to_string())
}

fn attach_project(database: &Database, path: &str) -> Result<Project, String> {
    let root = canonical_repository_root(Path::new(path))?;
    let repository_path = root.to_string_lossy().into_owned();
    let id = project_id_for_root(&root);
    let name = root
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("Repository")
        .to_owned();
    let project = project_from_parts(id.clone(), name, String::new(), repository_path);
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    connection
        .execute(
            "INSERT OR IGNORE INTO projects (id, name, goal, repository_path) VALUES (?1, ?2, ?3, ?4)",
            params![project.id, project.name, project.goal, project.repository_path],
        )
        .map_err(|error| error.to_string())?;
    connection
        .execute(
            "INSERT INTO app_state (key, value) VALUES ('active_project_id', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [&id],
        )
        .map_err(|error| error.to_string())?;
    active_project(&connection)?.ok_or_else(|| "Attached project could not be loaded.".to_owned())
}

fn project_row_counts(connection: &Connection) -> Result<Vec<(&'static str, i64)>, rusqlite::Error> {
    PROJECT_SCOPED_TABLES
        .iter()
        .map(|table| {
            connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get(0))
                .map(|count| (*table, count))
        })
        .collect()
}

fn backup_v1_database(connection: &Connection) -> Result<(), String> {
    let Some(path) = connection
        .path()
        .filter(|path| *path != ":memory:")
        .map(PathBuf::from)
    else {
        return Ok(());
    };
    let backup = PathBuf::from(format!("{}.v1.bak", path.to_string_lossy()));
    if !path.is_file() {
        return Ok(());
    }
    if !backup.exists() {
        std::fs::copy(&path, &backup)
            .map_err(|error| format!("Failed to back up v1 database: {error}"))?;
    }
    Ok(())
}

fn backup_v1_database_before_migration(connection: &Connection) -> rusqlite::Result<()> {
    let has_projects_table: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'projects')",
        [],
        |row| row.get(0),
    )?;
    if !has_projects_table {
        return Ok(());
    }
    let legacy_path = connection
        .query_row(
            "SELECT repository_path FROM projects WHERE id = 'agent-room'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    if legacy_path
        .as_deref()
        .and_then(|path| canonical_repository_root(Path::new(path)).ok())
        .is_none()
    {
        return Ok(());
    }
    backup_v1_database(connection)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(error.into()))
}

fn migrate_v1_to_v2(connection: &Connection) -> rusqlite::Result<()> {
    let legacy_path = connection
        .query_row(
            "SELECT repository_path FROM projects WHERE id = 'agent-room'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let Some(legacy_path) = legacy_path else {
        return Ok(());
    };
    let root = match canonical_repository_root(Path::new(&legacy_path)) {
        Ok(root) => root,
        Err(_) => return Ok(()),
    };
    let project_id = project_id_for_root(&root);
    let row_counts = project_row_counts(connection)?;
    connection.execute_batch("PRAGMA foreign_keys = OFF;")?;
    let result = (|| {
        let transaction = connection.unchecked_transaction()?;
        for table in PROJECT_SCOPED_TABLES {
            let column = if *table == "projects" { "id" } else { "project_id" };
            transaction.execute(
                &format!("UPDATE {table} SET {column} = ?1 WHERE {column} = 'agent-room'"),
                [&project_id],
            )?;
        }
        transaction.execute(
            "INSERT INTO app_state (key, value) VALUES ('active_project_id', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [&project_id],
        )?;
        if project_row_counts(&transaction)? != row_counts {
            return Err(rusqlite::Error::InvalidQuery);
        }
        transaction.commit()
    })();
    let _ = connection.execute_batch("PRAGMA foreign_keys = ON;");
    result
}

fn reconcile_interrupted_runs(connection: &Connection) -> rusqlite::Result<usize> {
    connection.execute(
        "UPDATE activations
         SET state = 'failed', finished_at = CURRENT_TIMESTAMP
         WHERE state = 'running'",
        [],
    )?;
    connection.execute(
        "UPDATE runs
         SET state = 'stopped',
             current_owner = NULL,
             stop_reason = 'Agent Room closed or restarted while this run was active. The managed worktree was preserved for recovery.',
             finished_at = CURRENT_TIMESTAMP
         WHERE state IN ('selecting', 'working', 'verifying', 'reviewing', 'revising', 'promoting')",
        [],
    )
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
fn hide_std_command_window(command: &mut StdCommand) {
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_std_command_window(_command: &mut StdCommand) {}

#[cfg(windows)]
fn hide_tokio_command_window(command: &mut Command) {
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_tokio_command_window(_command: &mut Command) {}

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

fn capabilities_for(
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
            autonomy_mode: "unavailable".to_owned(),
            autonomy_note: "Unknown provider.".to_owned(),
            capability_proof: vec![],
        },
    }
}

fn provider_effort_options(kind: &str) -> Vec<String> {
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

fn model_options(kind: &str, executable: Option<&Path>) -> (Vec<String>, String, Vec<String>) {
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

fn parse_provider_model_list(output: &str) -> Vec<String> {
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

fn normalize_cursor_model_id(model: &str) -> Option<String> {
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

fn parse_cursor_model_list(output: &str) -> Vec<String> {
    parse_provider_model_list(output)
        .into_iter()
        .filter_map(|model| normalize_cursor_model_id(&model))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn probe_provider(kind: &str) -> Participant {
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

async fn cached_provider(
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

async fn cached_participants(
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

fn detected_connection(participant: &Participant) -> ProviderConnection {
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

fn saved_connection(
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

fn save_connection(
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

fn participant_for_project(
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

fn participants_with_connections(
    database: &Database,
    project_id: &str,
    participants: Vec<Participant>,
) -> Result<Vec<Participant>, String> {
    participants
        .into_iter()
        .map(|participant| participant_for_project(database, project_id, participant))
        .collect()
}

fn git_bytes(repository: &Path, args: &[String]) -> Result<Vec<u8>, String> {
    let executable = find_executable(&["git"]).ok_or_else(|| "Git is not installed.".to_owned())?;
    let safe = format!("safe.directory={}", repository.to_string_lossy());
    let mut command = StdCommand::new(executable);
    hide_std_command_window(&mut command);
    let output = command
        .arg("-c")
        .arg(safe)
        .args(args)
        .current_dir(repository)
        .output()
        .map_err(|error| format!("Failed to run Git: {error}"))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let command = format!("git {}", args.join(" "));
        let detail = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            "Git returned no diagnostic output.".to_owned()
        };
        Err(format!(
            "{command} failed with {}.\n{detail}",
            output.status
        ))
    }
}

fn git(repository: &Path, args: &[String]) -> Result<String, String> {
    Ok(String::from_utf8_lossy(&git_bytes(repository, args)?)
        .trim()
        .to_owned())
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
        Phase::Chat => false,
        Phase::Build | Phase::Revise => {
            matches!(handoff.status.as_str(), "completed" | "blocked" | "failed")
        }
        Phase::Review | Phase::FinalReview => {
            matches!(handoff.status.as_str(), "approved" | "changes_required")
        }
    }
}

fn handoff_attention_reason(handoff: &AgentHandoff) -> String {
    let findings = if handoff.findings.is_empty() {
        String::new()
    } else {
        format!("\n{}", handoff.findings.join("\n"))
    };
    format!(
        "{}{findings}\nNext action: {}",
        handoff.summary, handoff.next_action
    )
}

fn extract_phase_handoff(value: &str, phase: Phase) -> Result<AgentHandoff, String> {
    if phase == Phase::Chat {
        return Err("Chat responses do not require an Agent Room handoff.".to_owned());
    }
    let handoff = extract_handoff(value)?;
    if handoff_status_allowed(&handoff, phase) {
        Ok(handoff)
    } else {
        Err(format!(
            "Handoff status `{}` is invalid for {}.",
            handoff.status,
            phase.as_str()
        ))
    }
}

fn extract_ship_intent(value: &str) -> Result<Option<ShipIntent>, String> {
    let Some(start) = value.find(SHIP_INTENT_START) else {
        return Ok(None);
    };
    let json_start = start + SHIP_INTENT_START.len();
    let end = value[json_start..]
        .find(SHIP_INTENT_END)
        .map(|offset| json_start + offset)
        .ok_or_else(|| "Autonomous Ship intent is missing its closing marker.".to_owned())?;
    let intent = serde_json::from_str::<ShipIntent>(value[json_start..end].trim())
        .map_err(|error| format!("Invalid autonomous Ship intent: {error}"))?;
    if intent.schema_version != 1 {
        return Err(format!(
            "Unsupported autonomous Ship intent schema version {}.",
            intent.schema_version
        ));
    }
    if intent.objective.trim().is_empty() || intent.reason.trim().is_empty() {
        return Err("Autonomous Ship intent requires an objective and reason.".to_owned());
    }
    Ok(Some(intent))
}

fn visible_chat_response(value: &str) -> String {
    let Some(start) = value.find(SHIP_INTENT_START) else {
        return value.trim().to_owned();
    };
    let json_start = start + SHIP_INTENT_START.len();
    let Some(end_offset) = value[json_start..].find(SHIP_INTENT_END) else {
        let before = value[..start].trim();
        return if before.is_empty() {
            "The provider returned an invalid autonomous Ship intent, so no repository work was started."
                .to_owned()
        } else {
            before.to_owned()
        };
    };
    let end = json_start + end_offset + SHIP_INTENT_END.len();
    let visible = format!("{}\n{}", value[..start].trim(), value[end..].trim())
        .trim()
        .to_owned();
    if visible.is_empty() {
        "This request is ready for the autonomous Ship workflow.".to_owned()
    } else {
        visible
    }
}

fn authentication_attention(summary: &str, stderr: &str) -> Option<String> {
    let summary = summary.to_ascii_lowercase();
    let stderr = stderr.to_ascii_lowercase();
    let onboarding = [
        "you are currently not signed in",
        "signing in...",
        "terms of service & data use",
        "welcome to antigravity cli!",
    ];
    let provider_error = ["authentication required", "not logged in", "oauth"];
    if onboarding.iter().any(|signal| summary.contains(signal))
        || onboarding.iter().any(|signal| stderr.contains(signal))
        || provider_error.iter().any(|signal| stderr.contains(signal))
    {
        Some("The provider requires sign-in or onboarding before Agent Room can use it.".to_owned())
    } else {
        None
    }
}

fn connection_test_ready(result: &ProviderRun) -> bool {
    let normalized = result
        .summary
        .replace("```", "")
        .chars()
        .filter(|character| character.is_alphanumeric() || character.is_whitespace())
        .collect::<String>();
    result.success
        && authentication_attention(&result.summary, &result.stderr).is_none()
        && normalized.to_ascii_lowercase().contains("ready")
}

fn cache_declared_capabilities(database: &Database, participants: &[Participant]) -> Result<(), String> {
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    for participant in participants.iter().filter(|participant| participant.installed) {
        let Some(version) = participant.version.as_deref() else { continue; };
        connection
            .execute(
                "INSERT OR IGNORE INTO provider_capabilities (provider, version, capabilities_json) VALUES (?1, ?2, ?3)",
                params![participant.kind, version, serde_json::to_string(&participant.capabilities).map_err(|error| error.to_string())?],
            )
            .map_err(|error| error.to_string())?;
    }
    Ok(())
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

fn json_u64(value: &Value, key: &str) -> Option<u64> {
    value
        .get(key)
        .and_then(|value| value.as_u64().or_else(|| value.as_str()?.parse().ok()))
}

fn json_f64(value: &Value, key: &str) -> Option<f64> {
    value
        .get(key)
        .and_then(|value| value.as_f64().or_else(|| value.as_str()?.parse().ok()))
}

fn first_u64(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|key| json_u64(value, key))
}

fn first_f64(value: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|key| json_f64(value, key))
}

fn merge_usage(target: &mut ProviderUsage, value: &Value) {
    for source in [
        Some(value),
        value.get("usage"),
        value.get("usage_info"),
        value.get("usageInfo"),
        value
            .get("message")
            .and_then(|message| message.get("usage")),
    ]
    .into_iter()
    .flatten()
    {
        target.input_tokens =
            first_u64(source, &["input_tokens", "inputTokens"]).or(target.input_tokens);
        target.cached_input_tokens = first_u64(
            source,
            &[
                "cached_input_tokens",
                "cachedInputTokens",
                "cache_read_input_tokens",
            ],
        )
        .or(target.cached_input_tokens);
        target.output_tokens =
            first_u64(source, &["output_tokens", "outputTokens"]).or(target.output_tokens);
        target.total_cost_usd =
            first_f64(source, &["total_cost_usd", "totalCostUsd"]).or(target.total_cost_usd);
        target.num_turns = first_u64(source, &["num_turns", "numTurns"]).or(target.num_turns);
    }
}

fn reported_model(value: &Value) -> Option<String> {
    ["model", "model_id", "modelId"]
        .iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .filter(|model| !model.trim().is_empty())
        .map(str::to_owned)
}

fn chat_response_is_complete(value: &str) -> bool {
    !value.trim().is_empty()
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

fn text_blocks(value: &Value) -> Option<String> {
    let blocks = value.as_array()?;
    let text = blocks
        .iter()
        .filter_map(|block| {
            let block_type = block.get("type").and_then(Value::as_str);
            if block_type.is_some_and(|kind| !matches!(kind, "text" | "output_text")) {
                return None;
            }
            block
                .get("text")
                .and_then(Value::as_str)
                .or_else(|| block.get("content").and_then(Value::as_str))
        })
        .collect::<Vec<_>>()
        .join("");
    (!text.trim().is_empty()).then_some(text)
}

fn provider_chat_text(kind: &str, value: &Value) -> Option<String> {
    let event_type = value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    match kind {
        "codex" => {
            let item = value.get("item")?;
            (item.get("type").and_then(Value::as_str) == Some("agent_message"))
                .then(|| item.get("text").and_then(Value::as_str).map(str::to_owned))
                .flatten()
        }
        "claude" => match event_type {
            "assistant" => value
                .get("message")
                .and_then(|message| message.get("content"))
                .and_then(text_blocks),
            "result" => value
                .get("result")
                .and_then(Value::as_str)
                .map(str::to_owned),
            _ => None,
        },
        "cursor" => match event_type {
            "result" => value
                .get("result")
                .and_then(Value::as_str)
                .map(str::to_owned),
            _ => None,
        },
        _ => None,
    }
    .filter(|text| !text.trim().is_empty())
}

fn provider_chat_fragment(kind: &str, value: &Value) -> Option<String> {
    let event_type = value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if kind == "cursor" && event_type == "assistant" {
        return value
            .get("message")
            .and_then(|message| message.get("content"))
            .and_then(text_blocks);
    }
    let text = match kind {
        "claude" if event_type == "stream_event" => value
            .get("event")
            .and_then(|event| event.get("delta"))
            .and_then(|delta| delta.get("text"))
            .and_then(Value::as_str),
        "cursor" if event_type == "text" => {
            value.get("text").and_then(Value::as_str).or_else(|| {
                value
                    .get("part")
                    .and_then(|part| part.get("text"))
                    .and_then(Value::as_str)
            })
        }
        _ => None,
    }?;
    (!text.is_empty()).then(|| text.to_owned())
}

fn apply_provider_environment(command: &mut Command, repository: &Path) {
    command.env("NO_COLOR", "1");
    command
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "safe.directory")
        .env("GIT_CONFIG_VALUE_0", repository);
    hide_tokio_command_window(command);
}

fn provider_activity(kind: &str, value: &Value) -> Option<(String, String)> {
    let event_type = value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    match kind {
        "codex" if matches!(event_type, "item.started" | "item.completed") => {
            let item = value.get("item")?;
            let item_type = item.get("type").and_then(Value::as_str).unwrap_or_default();
            match item_type {
                "reasoning" => item
                    .get("text")
                    .and_then(Value::as_str)
                    .filter(|text| !text.trim().is_empty())
                    .map(|text| ("Thinking".to_owned(), truncate_utf8(text.trim(), 2 * 1024))),
                "command_execution" => item.get("command").and_then(Value::as_str).map(|command| {
                    let status = item
                        .get("status")
                        .and_then(Value::as_str)
                        .unwrap_or("running");
                    (
                        if status == "completed" {
                            "Command finished"
                        } else {
                            "Running command"
                        }
                        .to_owned(),
                        truncate_utf8(command, 1024),
                    )
                }),
                "file_change" => Some((
                    "Updating files".to_owned(),
                    "Applying repository changes in the managed worktree.".to_owned(),
                )),
                "mcp_tool_call" => Some((
                    "Using tool".to_owned(),
                    item.get("tool")
                        .or_else(|| item.get("name"))
                        .and_then(Value::as_str)
                        .unwrap_or("Provider tool")
                        .to_owned(),
                )),
                _ => None,
            }
        }
        "claude" if event_type == "stream_event" => {
            let event = value.get("event")?;
            let native_type = event
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if native_type == "content_block_start" {
                let block = event.get("content_block")?;
                if block.get("type").and_then(Value::as_str) == Some("tool_use") {
                    return Some((
                        "Using tool".to_owned(),
                        block
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or("Claude tool")
                            .to_owned(),
                    ));
                }
            }
            if native_type == "content_block_delta" {
                let delta = event.get("delta")?;
                if delta.get("type").and_then(Value::as_str) == Some("thinking_delta") {
                    return delta
                        .get("thinking")
                        .and_then(Value::as_str)
                        .filter(|text| !text.trim().is_empty())
                        .map(|text| ("Thinking".to_owned(), truncate_utf8(text.trim(), 2 * 1024)));
                }
            }
            None
        }
        "cursor" => {
            let tool = value
                .get("tool")
                .or_else(|| value.get("name"))
                .and_then(Value::as_str);
            if event_type.contains("tool") || event_type.contains("command") {
                Some((
                    if event_type.contains("command") {
                        "Running command"
                    } else {
                        "Using tool"
                    }
                    .to_owned(),
                    tool.unwrap_or("Cursor tool").to_owned(),
                ))
            } else {
                None
            }
        }
        _ => None,
    }
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

async fn preallocate_cursor_session(executable: &Path) -> Result<String, String> {
    let output = timeout(Duration::from_secs(15), {
        let mut command = Command::new(executable);
        hide_tokio_command_window(&mut command);
        command.arg("create-chat").output()
    })
    .await
    .map_err(|_| "Cursor chat creation timed out after 15 seconds.".to_owned())?
    .map_err(|error| format!("Could not create a Cursor chat: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "Cursor could not create a chat: {}",
            truncate_utf8(&String::from_utf8_lossy(&output.stderr), 500)
        ));
    }
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .next()
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| "Cursor did not return a chat ID from create-chat.".to_owned())
}

async fn wait_for_idle(mut activity: watch::Receiver<u64>, timeout_seconds: u64) {
    loop {
        tokio::select! {
            changed = activity.changed() => {
                if changed.is_err() {
                    return;
                }
            }
            _ = sleep(Duration::from_secs(timeout_seconds)) => return,
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn invoke_provider(
    app: &AppHandle,
    run_id: &str,
    participant: &Participant,
    phase: Phase,
    allow_writes: bool,
    prompt: &str,
    repository: &Path,
    session_id: Option<&str>,
    requested_model: Option<&str>,
    requested_effort: Option<&str>,
    final_output_path: &Path,
    mut cancellation: watch::Receiver<bool>,
) -> Result<ProviderRun, String> {
    let kind = participant.kind.as_str();
    let executable = find_provider_executable(kind)
        .ok_or_else(|| format!("{} CLI is not installed.", provider_names(kind).0))?;
    let structured_chat = phase != Phase::Chat || participant.capabilities.structured_output;
    if phase != Phase::Chat
        && matches!(
            participant.capabilities.autonomy_mode.as_str(),
            "manual" | "unavailable"
        )
    {
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
    let prompt_path = artifact_dir.join(format!("{}.prompt.txt", phase.as_str()));
    std::fs::write(&prompt_path, prompt).map_err(|error| error.to_string())?;
    let argv_prompt = format!(
        "Read the file at {} in full. It contains your complete assignment. Follow it exactly.",
        prompt_path.display()
    );
    let preallocated_cursor_session = if kind == "cursor" && session_id.is_none() {
        Some(preallocate_cursor_session(&executable).await?)
    } else {
        None
    };
    let effective_session = preallocated_cursor_session.clone().or_else(|| session_id.map(str::to_owned));
    let effective_session_id = effective_session.as_deref();
    let mode = if !allow_writes {
        ProviderMode::Ask
    } else if phase == Phase::Chat {
        ProviderMode::QuickEdit
    } else {
        ProviderMode::Ship
    };
    let handoff_contract = (phase != Phase::Chat).then(|| handoff_contract(phase));
    let prepared = providers::build_command(
        kind,
        &TurnRequest {
            mode,
            phase: phase.as_str(),
            prompt: if matches!(kind, "cursor" | "antigravity") { &argv_prompt } else { prompt },
            repository,
            session_id: effective_session_id,
            model: requested_model,
            effort: requested_effort,
            final_output_path,
            structured_output: structured_chat,
            handoff_contract: handoff_contract.as_deref(),
        },
    )?;
    let assigned_session_id = prepared
        .assigned_session_id
        .clone()
        .or(preallocated_cursor_session.clone());
    let mut command = Command::new(executable);
    command.args(&prepared.args);
    let stdin_prompt = prepared.stdin;

    apply_provider_environment(&mut command, repository);
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

    let provider_started = Instant::now();
    let mut child = command
        .spawn()
        .map_err(|error| format!("Failed to start {}: {error}", provider_names(kind).0))?;
    let process_start_ms = provider_started.elapsed().as_millis() as u64;

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
    let (completion_sender, mut completion_receiver) = watch::channel(false);
    let stdout_activity = activity_sender.clone();
    let completion_phase = phase;
    let stdout_started = provider_started;
    let stdout_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        let mut session_id = None;
        let mut result_text = String::new();
        let mut raw = String::new();
        let mut actual_model = None;
        let mut usage = ProviderUsage::default();
        let mut activity_emitted = false;
        let mut first_output_ms = None;
        while let Ok(Some(line)) = lines.next_line().await {
            first_output_ms.get_or_insert_with(|| stdout_started.elapsed().as_millis() as u64);
            stdout_activity.send_modify(|value| *value = value.saturating_add(1));
            append_capped(&mut raw, &line);
            if let Ok(value) = serde_json::from_str::<Value>(&line) {
                session_id = session_id.or_else(|| parse_session_id(&value));
                actual_model = actual_model.or_else(|| reported_model(&value));
                merge_usage(&mut usage, &value);
                if let Some((title, detail)) = provider_activity(&stdout_agent, &value) {
                    emit_event(
                        &stdout_app,
                        &stdout_run,
                        "stream",
                        &stdout_phase,
                        "running",
                        Some(&stdout_agent),
                        &title,
                        &detail,
                        None,
                    );
                    activity_emitted = true;
                }
                if completion_phase == Phase::Chat {
                    if let Some(fragment) = provider_chat_fragment(&stdout_agent, &value) {
                        result_text.push_str(&fragment);
                    } else if let Some(text) = provider_chat_text(&stdout_agent, &value) {
                        result_text = text;
                    } else if let Some(text) = parse_result_text(&value) {
                        result_text = text;
                    }
                } else if let Some(text) = parse_result_text(&value) {
                    result_text = text;
                }
            } else if completion_phase == Phase::Chat && !structured_chat {
                append_capped(&mut result_text, &line);
            }
            if raw.contains(HANDOFF_END) && extract_phase_handoff(&raw, completion_phase).is_ok() {
                completion_sender.send_replace(true);
            }
            if completion_phase == Phase::Chat {
                if result_text.trim().is_empty() {
                    if !activity_emitted {
                        emit_event(
                            &stdout_app,
                            &stdout_run,
                            "stream",
                            &stdout_phase,
                            "running",
                            Some(&stdout_agent),
                            "Chat activity",
                            &format!(
                                "{} is working in the attached repository.",
                                provider_names(&stdout_agent).0
                            ),
                            None,
                        );
                        activity_emitted = true;
                    }
                } else {
                    emit_event(
                        &stdout_app,
                        &stdout_run,
                        "stream",
                        &stdout_phase,
                        "running",
                        Some(&stdout_agent),
                        "Chat response",
                        &truncate_utf8(result_text.trim(), SOURCE_BUDGET_BYTES),
                        None,
                    );
                }
            } else if !line.trim_start().starts_with('{') {
                emit_event(
                    &stdout_app,
                    &stdout_run,
                    "stream",
                    &stdout_phase,
                    "running",
                    Some(&stdout_agent),
                    "Provider activity",
                    &truncate_utf8(&line, 2 * 1024),
                    None,
                );
            }
        }
        (
            session_id,
            result_text,
            raw,
            actual_model,
            usage,
            first_output_ms,
        )
    });

    let stderr_activity = activity_sender;
    let stderr_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        let mut raw = String::new();
        while let Ok(Some(line)) = lines.next_line().await {
            stderr_activity.send_modify(|value| *value = value.saturating_add(1));
            append_capped(&mut raw, &line);
        }
        raw
    });

    let mut stopped = false;
    let mut timed_out = false;
    let mut idle_timed_out = false;
    let mut completed_handoff = false;
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
        _ = wait_for_idle(activity_receiver, idle_timeout_seconds(phase)) => {
            idle_timed_out = true;
            let _ = child.kill().await;
            child.wait().await.map_err(|error| error.to_string())?
        },
        changed = completion_receiver.changed() => {
            if changed.is_ok() && *completion_receiver.borrow() {
                completed_handoff = true;
                let _ = child.kill().await;
            }
            child.wait().await.map_err(|error| error.to_string())?
        }
    };

    let (parsed_session, parsed_result, raw_stdout, actual_model, usage, first_output_ms) =
        stdout_task.await.map_err(|error| error.to_string())?;
    let raw_stderr = stderr_task.await.map_err(|error| error.to_string())?;
    std::fs::write(&stdout_log_path, &raw_stdout).map_err(|error| error.to_string())?;
    std::fs::write(&stderr_log_path, &raw_stderr).map_err(|error| error.to_string())?;
    let file_result = std::fs::read_to_string(final_output_path).unwrap_or_default();
    let raw_result = if !file_result.trim().is_empty() {
        file_result.trim().to_owned()
    } else if !parsed_result.trim().is_empty() {
        parsed_result.trim().to_owned()
    } else if phase != Phase::Chat || !structured_chat {
        raw_stdout.trim().to_owned()
    } else {
        String::new()
    };
    let (handoff, schema_error) = if phase == Phase::Chat {
        (None, None)
    } else {
        match extract_phase_handoff(&raw_result, phase) {
            Ok(handoff) => (Some(handoff), None),
            Err(error) => (None, Some(error)),
        }
    };
    let summary = handoff
        .as_ref()
        .map(|value| value.summary.clone())
        .unwrap_or_else(|| truncate_utf8(&raw_result, SOURCE_BUDGET_BYTES));
    let logical_success = if phase == Phase::Chat {
        chat_response_is_complete(&raw_result)
    } else {
        handoff.as_ref().is_some_and(|value| match phase {
            Phase::Build | Phase::Revise => value.status == "completed",
            Phase::Review | Phase::FinalReview => {
                matches!(value.status.as_str(), "approved" | "changes_required")
            }
            Phase::Chat => false,
        })
    };
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
        session_id: parsed_session.or(assigned_session_id),
        success: (status.success()
            || completed_handoff
            || (phase == Phase::Chat && idle_timed_out && logical_success))
            && !stopped
            && !timed_out
            && (!idle_timed_out || (phase == Phase::Chat && logical_success))
            && logical_success,
        stopped,
        timed_out,
        idle_timed_out,
        stderr: diagnostic,
        handoff,
        actual_model,
        usage,
        stdout_log_path: stdout_log_path.to_string_lossy().into_owned(),
        stderr_log_path: stderr_log_path.to_string_lossy().into_owned(),
        process_start_ms,
        first_output_ms,
        session_resumed: participant.capabilities.exact_resume && effective_session_id.is_some(),
    })
}

fn provider_log_note(run: &ProviderRun) -> String {
    format!(
        "Durable logs:\nstdout: {}\nstderr: {}",
        run.stdout_log_path, run.stderr_log_path
    )
}

fn provider_failure_reason(provider: &str, activity: &str, run: &ProviderRun) -> String {
    format!(
        "{provider} could not complete {activity}. {}",
        provider_log_note(run)
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

fn save_chat_session(
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
            "INSERT INTO chat_sessions (project_id, participant_kind, provider_session_id)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(project_id, participant_kind) DO UPDATE SET
               provider_session_id = excluded.provider_session_id,
               last_used_at = CURRENT_TIMESTAMP",
            params![project_id, participant, session_id],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn load_chat_session(
    database: &Database,
    project_id: &str,
    participant: &str,
) -> Result<Option<String>, String> {
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    connection
        .query_row(
            "SELECT provider_session_id FROM chat_sessions
             WHERE project_id = ?1 AND participant_kind = ?2",
            params![project_id, participant],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())
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

fn usage_note(usage: &ProviderUsage) -> String {
    if usage.input_tokens.is_some()
        || usage.cached_input_tokens.is_some()
        || usage.output_tokens.is_some()
        || usage.total_cost_usd.is_some()
    {
        "Provider-reported usage. Account quota and reset windows were not reported by the provider."
            .to_owned()
    } else {
        "Provider did not report token usage, account quota, or reset windows for this phase."
            .to_owned()
    }
}

fn persist_receipt(
    database: &Database,
    run_id: &str,
    phase: Phase,
    participant: &Participant,
    profile: &ProviderProfile,
    metrics: ReceiptMetrics,
    result: &ProviderRun,
) -> Result<(), String> {
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    connection
        .execute(
            "INSERT INTO execution_receipts
             (id, run_id, phase, participant_kind, provider_version, requested_model,
              requested_effort, actual_model, session_id, context_bytes, usage_json, usage_note,
              preflight_ms, process_start_ms, first_output_ms, total_ms, session_resumed,
              packet_bytes_saved, stdout_log_path, stderr_log_path)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
                     ?16, ?17, ?18, ?19, ?20)",
            params![
                Uuid::new_v4().to_string(),
                run_id,
                phase.as_str(),
                participant.kind,
                participant.version.as_deref(),
                profile.model.as_deref(),
                profile.effort.as_deref(),
                result.actual_model.as_deref(),
                result.session_id.as_deref(),
                metrics.context_bytes as i64,
                serde_json::to_string(&result.usage).map_err(|error| error.to_string())?,
                usage_note(&result.usage),
                metrics.preflight_ms as i64,
                result.process_start_ms as i64,
                result.first_output_ms.map(|milliseconds| {
                    (metrics.preflight_ms + milliseconds) as i64
                }),
                metrics.total_ms as i64,
                result.session_resumed as i64,
                0_i64,
                result.stdout_log_path,
                result.stderr_log_path,
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn persist_chat_receipt(
    database: &Database,
    project_id: &str,
    chat_id: &str,
    participant: &Participant,
    profile: &ProviderProfile,
    metrics: ChatReceiptMetrics,
    result: &ProviderRun,
) -> Result<(), String> {
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    connection
        .execute(
            "INSERT INTO chat_receipts
             (id, project_id, chat_id, phase, participant_kind, provider_version,
              requested_model, requested_effort, actual_model, session_id, context_bytes,
              usage_json, usage_note, preflight_ms, process_start_ms, first_output_ms, total_ms,
              session_resumed, packet_bytes_saved, stdout_log_path, stderr_log_path)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                     ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
            params![
                Uuid::new_v4().to_string(),
                project_id,
                chat_id,
                Phase::Chat.as_str(),
                participant.kind,
                participant.version.as_deref(),
                profile.model.as_deref(),
                profile.effort.as_deref(),
                result.actual_model.as_deref(),
                result.session_id.as_deref(),
                metrics.context_bytes as i64,
                serde_json::to_string(&result.usage).map_err(|error| error.to_string())?,
                usage_note(&result.usage),
                metrics.preflight_ms as i64,
                result.process_start_ms as i64,
                result
                    .first_output_ms
                    .map(|milliseconds| (metrics.preflight_ms + milliseconds) as i64),
                metrics.total_ms as i64,
                result.session_resumed as i64,
                0_i64,
                result.stdout_log_path,
                result.stderr_log_path,
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

fn git_paths(repository: &Path, args: &[&str]) -> Result<Vec<String>, String> {
    let arguments = args
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    Ok(git_bytes(repository, &arguments)?
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| String::from_utf8_lossy(path).into_owned())
        .collect())
}

fn workspace_fingerprint(repository: &Path) -> Result<String, String> {
    let status = git_bytes(
        repository,
        &[
            "status".to_owned(),
            "--porcelain=v1".to_owned(),
            "-z".to_owned(),
            "--untracked-files=all".to_owned(),
        ],
    )?;
    let mut paths = BTreeSet::new();
    for args in [
        &["ls-files", "-m", "-d", "-o", "--exclude-standard", "-z"][..],
        &["diff", "--cached", "--name-only", "-z"][..],
    ] {
        paths.extend(git_paths(repository, args)?);
    }

    let mut hasher = Sha256::new();
    hasher.update(&status);
    for relative in paths {
        hasher.update([0]);
        hasher.update(relative.as_bytes());
        let path = repository.join(&relative);
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                hasher.update(b"symlink");
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    hasher.update(metadata.mode().to_le_bytes());
                }
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    hasher.update(metadata.file_attributes().to_le_bytes());
                }
                hasher.update(
                    std::fs::read_link(&path)
                        .map_err(|error| format!("Failed to inspect {relative}: {error}"))?
                        .to_string_lossy()
                        .as_bytes(),
                );
            }
            Ok(metadata) if metadata.is_file() => {
                hasher.update(b"file");
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    hasher.update(metadata.mode().to_le_bytes());
                }
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    hasher.update(metadata.file_attributes().to_le_bytes());
                }
                let mut file = std::fs::File::open(&path)
                    .map_err(|error| format!("Failed to inspect {relative}: {error}"))?;
                let mut buffer = [0_u8; 64 * 1024];
                loop {
                    let count = file
                        .read(&mut buffer)
                        .map_err(|error| format!("Failed to inspect {relative}: {error}"))?;
                    if count == 0 {
                        break;
                    }
                    hasher.update(&buffer[..count]);
                }
            }
            Ok(_) => hasher.update(b"directory"),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => hasher.update(b"missing"),
            Err(error) => return Err(format!("Failed to inspect {relative}: {error}")),
        }
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn copy_workspace_file(source: &Path, destination: &Path) -> Result<(), String> {
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let metadata = std::fs::symlink_metadata(source).map_err(|error| error.to_string())?;
    if metadata.file_type().is_symlink() {
        let target = std::fs::read_link(source).map_err(|error| error.to_string())?;
        #[cfg(unix)]
        std::os::unix::fs::symlink(target, destination).map_err(|error| error.to_string())?;
        #[cfg(windows)]
        {
            if source.is_dir() {
                std::os::windows::fs::symlink_dir(target, destination)
                    .map_err(|error| error.to_string())?;
            } else {
                std::os::windows::fs::symlink_file(target, destination)
                    .map_err(|error| error.to_string())?;
            }
        }
    } else {
        std::fs::copy(source, destination).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn remove_managed_clone(root: &Path, worktree: &Path) -> Result<(), String> {
    let root = std::fs::canonicalize(root).map_err(|error| error.to_string())?;
    let parent = worktree
        .parent()
        .ok_or_else(|| "Managed snapshot clone has no parent directory.".to_owned())?;
    let parent = std::fs::canonicalize(parent).map_err(|error| error.to_string())?;
    if parent != root || !worktree.join(".git").is_dir() {
        return Err(format!(
            "Refused to remove an unverified managed snapshot clone: {}",
            worktree.to_string_lossy()
        ));
    }
    std::fs::remove_dir_all(worktree).map_err(|error| error.to_string())
}

fn create_isolation_at_root(
    repository: &Path,
    run_id: &str,
    root: &Path,
) -> Result<IsolationContext, String> {
    let base_changes = git_static(repository, &["status", "--porcelain"])?;
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
    std::fs::create_dir_all(root).map_err(|error| error.to_string())?;
    let worktree = root.join(&short);
    if worktree.exists() {
        return Err(format!(
            "Managed worktree path already exists: {}",
            worktree.to_string_lossy()
        ));
    }

    if base_changes.trim().is_empty() {
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
        return Ok(IsolationContext {
            worktree,
            branch,
            base_branch,
            snapshot_head: base_head.clone(),
            base_head,
            isolation_kind: "worktree".to_owned(),
            workspace_fingerprint: None,
        });
    }

    let fingerprint = workspace_fingerprint(repository)?;
    let patch = root.join(format!(".snapshot-{short}.patch"));
    let snapshot_result = (|| {
        git(
            repository,
            &[
                "diff".to_owned(),
                "--binary".to_owned(),
                "--no-ext-diff".to_owned(),
                "HEAD".to_owned(),
                format!("--output={}", patch.to_string_lossy()),
            ],
        )?;
        git(
            repository,
            &[
                "clone".to_owned(),
                "--no-checkout".to_owned(),
                repository.to_string_lossy().into_owned(),
                worktree.to_string_lossy().into_owned(),
            ],
        )?;
        git(
            &worktree,
            &[
                "checkout".to_owned(),
                "-b".to_owned(),
                branch.clone(),
                base_head.clone(),
            ],
        )?;
        if patch
            .metadata()
            .map(|value| value.len())
            .unwrap_or_default()
            > 0
        {
            git(
                &worktree,
                &[
                    "apply".to_owned(),
                    "--binary".to_owned(),
                    patch.to_string_lossy().into_owned(),
                ],
            )?;
        }
        for relative in git_paths(
            repository,
            &["ls-files", "--others", "--exclude-standard", "-z"],
        )? {
            copy_workspace_file(&repository.join(&relative), &worktree.join(&relative))?;
        }
        if !commit_managed_changes(&worktree, "Capture current workspace state")? {
            return Err(
                "Ship cannot safely snapshot these uncommitted changes. Nested repository or submodule changes must be committed or stashed first."
                    .to_owned(),
            );
        }
        let snapshot_head = git_static(&worktree, &["rev-parse", "HEAD"])?;
        Ok(IsolationContext {
            worktree: worktree.clone(),
            branch,
            base_branch,
            base_head,
            snapshot_head,
            isolation_kind: "snapshot-clone".to_owned(),
            workspace_fingerprint: Some(fingerprint),
        })
    })();
    let _ = std::fs::remove_file(&patch);
    if snapshot_result.is_err() && worktree.exists() {
        let _ = remove_managed_clone(root, &worktree);
    }
    snapshot_result
}

fn create_worktree(
    app: &AppHandle,
    repository: &Path,
    run_id: &str,
) -> Result<IsolationContext, String> {
    let root = app
        .path()
        .app_cache_dir()
        .map_err(|error| error.to_string())?
        .join("worktrees");
    create_isolation_at_root(repository, run_id, &root)
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
        apply_provider_environment(&mut command, repository);
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
    isolation: &IsolationContext,
    managed_root: &Path,
) -> Result<PromotionResult, String> {
    let worktree = &isolation.worktree;
    let branch = &isolation.branch;
    let base_head = &isolation.base_head;
    let snapshot_head = &isolation.snapshot_head;
    let current_branch = git_static(base_repository, &["branch", "--show-current"])?;
    if current_branch != isolation.base_branch {
        return Err(format!(
            "The attached checkout moved from branch `{}` to {} while Ship was active. The verified work remains isolated.",
            isolation.base_branch,
            if current_branch.is_empty() {
                "a detached HEAD".to_owned()
            } else {
                format!("branch `{current_branch}`")
            }
        ));
    }
    let current_head = git_static(base_repository, &["rev-parse", "HEAD"])?;
    if current_head != base_head.as_str() {
        return Err("The base branch advanced while the run was active.".to_owned());
    }

    match isolation.isolation_kind.as_str() {
        "worktree" => {
            let base_status = git_static(base_repository, &["status", "--porcelain"])?;
            if !base_status.trim().is_empty() {
                return Err("The base checkout changed while the run was active.".to_owned());
            }
            git(
                base_repository,
                &[
                    "merge".to_owned(),
                    "--ff-only".to_owned(),
                    branch.to_owned(),
                ],
            )?;
            let mut cleanup_errors = Vec::new();
            if let Err(error) = git(
                base_repository,
                &[
                    "worktree".to_owned(),
                    "remove".to_owned(),
                    worktree.to_string_lossy().into_owned(),
                ],
            ) {
                cleanup_errors.push(error);
            }
            if let Err(error) = git(
                base_repository,
                &["branch".to_owned(), "-d".to_owned(), branch.to_owned()],
            ) {
                cleanup_errors.push(error);
            }
            Ok(PromotionResult {
                mode: PromotionMode::FastForward,
                cleanup_warning: (!cleanup_errors.is_empty()).then(|| cleanup_errors.join("\n")),
            })
        }
        "snapshot-clone" => {
            let expected = isolation.workspace_fingerprint.as_deref().ok_or_else(|| {
                "The preserved workspace snapshot is missing its safety fingerprint.".to_owned()
            })?;
            if workspace_fingerprint(base_repository)? != expected {
                return Err(
                    "The attached checkout changed while Ship was active. The verified work remains isolated so your newer edits are not overwritten."
                        .to_owned(),
                );
            }
            let patch = managed_root.join(format!(".promote-{}.patch", Uuid::new_v4()));
            let apply_result = (|| {
                git(
                    worktree,
                    &[
                        "diff".to_owned(),
                        "--binary".to_owned(),
                        "--no-ext-diff".to_owned(),
                        snapshot_head.to_owned(),
                        "HEAD".to_owned(),
                        format!("--output={}", patch.to_string_lossy()),
                    ],
                )?;
                git(
                    base_repository,
                    &[
                        "apply".to_owned(),
                        "--check".to_owned(),
                        "--binary".to_owned(),
                        patch.to_string_lossy().into_owned(),
                    ],
                )?;
                git(
                    base_repository,
                    &[
                        "apply".to_owned(),
                        "--binary".to_owned(),
                        patch.to_string_lossy().into_owned(),
                    ],
                )
            })();
            let _ = std::fs::remove_file(&patch);
            apply_result?;
            let cleanup_warning = remove_managed_clone(managed_root, worktree).err();
            Ok(PromotionResult {
                mode: PromotionMode::WorkingTree,
                cleanup_warning,
            })
        }
        other => Err(format!("Unknown Ship isolation kind: {other}")),
    }
}

fn discard_isolation(
    base_repository: &Path,
    isolation: &IsolationContext,
    managed_root: &Path,
) -> Option<String> {
    if isolation.isolation_kind == "snapshot-clone" {
        return remove_managed_clone(managed_root, &isolation.worktree).err();
    }
    let mut errors = Vec::new();
    if let Err(error) = git(
        base_repository,
        &[
            "worktree".to_owned(),
            "remove".to_owned(),
            isolation.worktree.to_string_lossy().into_owned(),
        ],
    ) {
        errors.push(error);
    }
    if let Err(error) = git(
        base_repository,
        &[
            "branch".to_owned(),
            "-D".to_owned(),
            isolation.branch.clone(),
        ],
    ) {
        errors.push(error);
    }
    (!errors.is_empty()).then(|| errors.join("\n"))
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
async fn get_environment(
    database: State<'_, Database>,
    runtime: State<'_, RuntimeState>,
    force_refresh: Option<bool>,
) -> Result<NativeEnvironment, String> {
    let project = {
        let connection = database.0.lock().map_err(|error| error.to_string())?;
        active_project(&connection)?
    };
    let Some(project) = project else {
        return Ok(NativeEnvironment {
            native: true,
            attached: false,
            repository_path: String::new(),
            branch: String::new(),
            participants: Vec::new(),
        });
    };
    let participants = cached_participants(runtime.inner(), force_refresh.unwrap_or(false)).await?;
    cache_declared_capabilities(database.inner(), &participants)?;
    Ok(NativeEnvironment {
        native: true,
        attached: true,
        repository_path: project.repository_path,
        branch: project.branch,
        participants: participants_with_connections(database.inner(), &project.id, participants)?,
    })
}

#[tauri::command]
async fn project_pick(app: AppHandle) -> Result<Option<String>, String> {
    Ok(app
        .dialog()
        .file()
        .blocking_pick_folder()
        .map(|path| path.to_string()))
}

#[tauri::command]
fn project_attach(database: State<'_, Database>, path: String) -> Result<Project, String> {
    attach_project(database.inner(), &path)
}

#[tauri::command]
fn project_list(database: State<'_, Database>) -> Result<Vec<Project>, String> {
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    let mut statement = connection
        .prepare("SELECT id, name, goal, repository_path FROM projects ORDER BY updated_at DESC, name")
        .map_err(|error| error.to_string())?;
    let projects = statement
        .query_map([], |row| Ok(project_from_parts(row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    Ok(projects)
}

#[tauri::command]
fn project_select(database: State<'_, Database>, id: String) -> Result<Project, String> {
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    let project = connection
        .query_row(
            "SELECT id, name, goal, repository_path FROM projects WHERE id = ?1",
            [&id],
            |row| Ok(project_from_parts(row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Project not found.".to_owned())?;
    connection
        .execute(
            "INSERT INTO app_state (key, value) VALUES ('active_project_id', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [&id],
        )
        .map_err(|error| error.to_string())?;
    Ok(project)
}

#[tauri::command]
fn project_active(database: State<'_, Database>) -> Result<Option<Project>, String> {
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    active_project(&connection)
}

#[tauri::command]
fn load_project_settings(
    database: State<'_, Database>,
    project_id: String,
) -> Result<ProjectSettings, String> {
    project_settings(database.inner(), &project_id)
}

fn project_settings(database: &Database, project_id: &str) -> Result<ProjectSettings, String> {
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    connection
        .query_row(
            "SELECT autonomous_ship_enabled FROM project_settings WHERE project_id = ?1",
            [project_id],
            |row| {
                Ok(ProjectSettings {
                    autonomous_ship_enabled: row.get::<_, i64>(0)? != 0,
                })
            },
        )
        .optional()
        .map_err(|error| error.to_string())
        .map(|settings| settings.unwrap_or_default())
}

#[tauri::command]
fn save_project_settings(
    database: State<'_, Database>,
    settings: ProjectSettingsInput,
) -> Result<(), String> {
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    connection
        .execute(
            "INSERT INTO project_settings (project_id, autonomous_ship_enabled)
             VALUES (?1, ?2)
             ON CONFLICT(project_id) DO UPDATE SET
               autonomous_ship_enabled = excluded.autonomous_ship_enabled,
               updated_at = CURRENT_TIMESTAMP",
            params![
                settings.project_id,
                i64::from(settings.autonomous_ship_enabled)
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
fn load_provider_profiles(
    database: State<'_, Database>,
    project_id: String,
) -> Result<Vec<ProviderProfile>, String> {
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    let mut statement = connection
        .prepare(
            "SELECT participant_kind, route, model, effort
             FROM provider_route_profiles
             WHERE project_id = ?1 ORDER BY participant_kind, route",
        )
        .map_err(|error| error.to_string())?;
    let profiles = statement
        .query_map([project_id], |row| {
            Ok(ProviderProfile {
                participant_kind: row.get(0)?,
                route: row.get(1)?,
                model: row.get(2)?,
                effort: row.get(3)?,
            })
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    Ok(profiles)
}

#[tauri::command]
async fn save_provider_profile(
    database: State<'_, Database>,
    profile: ProviderProfileInput,
) -> Result<(), String> {
    if !matches!(
        profile.participant_kind.as_str(),
        "codex" | "claude" | "cursor" | "antigravity"
    ) || !matches!(profile.route.as_str(), "chat" | "build" | "review")
    {
        return Err("Unknown provider profile.".to_owned());
    }
    let model = profile.model.filter(|value| !value.trim().is_empty());
    let effort = profile.effort.filter(|value| !value.trim().is_empty());
    if let Some(value) = effort.as_deref() {
        let allowed = provider_effort_options(&profile.participant_kind);
        if !allowed.iter().any(|option| option == value) {
            return Err(format!(
                "{} does not support the `{value}` effort level.",
                provider_names(&profile.participant_kind).0
            ));
        }
        if profile.participant_kind == "cursor" && model.is_none() {
            return Err("Cursor requires an explicit model before effort can be set.".to_owned());
        }
    }
    {
        let connection = database.0.lock().map_err(|error| error.to_string())?;
        connection
            .execute(
                "INSERT INTO provider_route_profiles
                 (project_id, participant_kind, route, model, effort)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(project_id, participant_kind, route) DO UPDATE SET
                   model = excluded.model,
                   effort = excluded.effort,
                   updated_at = CURRENT_TIMESTAMP",
                params![
                    profile.project_id,
                    profile.participant_kind,
                    profile.route,
                    model,
                    effort
                ],
            )
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn provider_profile(
    database: &Database,
    project_id: &str,
    participant: &str,
    route: &str,
) -> Result<ProviderProfile, String> {
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    connection
        .query_row(
            "SELECT participant_kind, route, model, effort
             FROM provider_route_profiles
             WHERE project_id = ?1 AND participant_kind = ?2 AND route = ?3",
            params![project_id, participant, route],
            |row| {
                Ok(ProviderProfile {
                    participant_kind: row.get(0)?,
                    route: row.get(1)?,
                    model: row.get(2)?,
                    effort: row.get(3)?,
                })
            },
        )
        .optional()
        .map_err(|error| error.to_string())
        .map(|profile| {
            profile.unwrap_or(ProviderProfile {
                participant_kind: participant.to_owned(),
                route: route.to_owned(),
                model: None,
                effort: None,
            })
        })
}

fn recent_chat_handoff(
    database: &Database,
    project_id: &str,
    target_participant: &str,
    resumed_target_session: bool,
    exclude_run_id: Option<&str>,
) -> Result<Option<String>, String> {
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    let mut statement = connection
        .prepare(
            "SELECT sender_kind, message_kind, body
             FROM messages
             WHERE project_id = ?1 AND message_kind IN ('human', 'agent')
               AND (?2 IS NULL OR run_id IS NULL OR run_id <> ?2)
             ORDER BY created_at DESC, rowid DESC
             LIMIT 8",
        )
        .map_err(|error| error.to_string())?;
    let messages = statement
        .query_map(params![project_id, exclude_run_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    let Some(agent_index) = messages.iter().position(|(_, kind, _)| kind == "agent") else {
        return Ok(None);
    };
    let (agent, _, answer) = &messages[agent_index];
    if resumed_target_session && agent == target_participant {
        return Ok(None);
    }
    let prior_user = messages
        .iter()
        .skip(agent_index + 1)
        .find(|(_, kind, _)| kind == "human")
        .map(|(_, _, body)| body.as_str());
    let agent_name = provider_names(agent).0;
    let handoff = match prior_user {
        Some(user) => format!(
            "Recent room handoff from another provider or a non-resumable session:\nUser: {}\n{}: {}",
            truncate_utf8(user, CHAT_HANDOFF_BUDGET_BYTES / 3),
            agent_name,
            truncate_utf8(answer, CHAT_HANDOFF_BUDGET_BYTES * 2 / 3),
        ),
        None => format!(
            "Recent room handoff from another provider or a non-resumable session:\n{}: {}",
            agent_name,
            truncate_utf8(answer, CHAT_HANDOFF_BUDGET_BYTES),
        ),
    };
    Ok(Some(truncate_utf8(&handoff, CHAT_HANDOFF_BUDGET_BYTES)))
}

fn objective_needs_room_context(objective: &str) -> bool {
    let normalized = format!(" {} ", objective.trim().to_ascii_lowercase());
    objective.len() <= 180
        && [
            " it ",
            " this ",
            " that ",
            " those ",
            " above ",
            " add it ",
            " do it ",
            " fix it ",
            " go ahead ",
            " can you add ",
            " can you do ",
        ]
        .iter()
        .any(|signal| normalized.contains(signal))
}

#[tauri::command]
fn load_room(database: State<'_, Database>, project_id: String) -> Result<RoomSnapshot, String> {
    load_room_snapshot(database.inner(), &project_id)
}

fn load_room_snapshot(database: &Database, project_id: &str) -> Result<RoomSnapshot, String> {
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
    let mut receipts = latest_run
        .as_ref()
        .map(|run| {
            let mut statement = connection
                .prepare(
                    "SELECT id, phase, participant_kind, provider_version, requested_model,
                            requested_effort, actual_model, session_id, context_bytes, usage_json,
                            usage_note, created_at, preflight_ms, process_start_ms, first_output_ms,
                            total_ms, session_resumed, packet_bytes_saved, stdout_log_path, stderr_log_path
                     FROM execution_receipts WHERE run_id = ?1 ORDER BY created_at ASC",
                )
                .map_err(|error| error.to_string())?;
            let receipts = statement
                .query_map([&run.id], |row| {
                    let usage_json: String = row.get(9)?;
                    Ok(ExecutionReceipt {
                        id: row.get(0)?,
                        phase: row.get(1)?,
                        participant: row.get(2)?,
                        provider_version: row.get(3)?,
                        requested_model: row.get(4)?,
                        requested_effort: row.get(5)?,
                        actual_model: row.get(6)?,
                        session_id: row.get(7)?,
                        context_bytes: row.get::<_, i64>(8)?.max(0) as usize,
                        usage: serde_json::from_str(&usage_json).unwrap_or_default(),
                        usage_note: row.get(10)?,
                        created_at: row.get(11)?,
                        preflight_ms: row.get::<_, Option<i64>>(12)?.map(|value| value.max(0) as u64),
                        process_start_ms: row.get::<_, Option<i64>>(13)?.map(|value| value.max(0) as u64),
                        first_output_ms: row.get::<_, Option<i64>>(14)?.map(|value| value.max(0) as u64),
                        total_ms: row.get::<_, Option<i64>>(15)?.map(|value| value.max(0) as u64),
                        session_resumed: row.get::<_, i64>(16)? != 0,
                        packet_bytes_saved: row.get::<_, i64>(17)?.max(0) as usize,
                        stdout_log_path: row.get(18)?,
                        stderr_log_path: row.get(19)?,
                    })
                })
                .map_err(|error| error.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| error.to_string())?;
            Ok::<Vec<ExecutionReceipt>, String>(receipts)
        })
        .transpose()?
        .unwrap_or_default();
    let mut statement = connection
        .prepare(
            "SELECT id, phase, participant_kind, provider_version, requested_model,
                    requested_effort, actual_model, session_id, context_bytes, usage_json,
                    usage_note, created_at, preflight_ms, process_start_ms, first_output_ms, total_ms,
                    session_resumed, packet_bytes_saved, stdout_log_path, stderr_log_path
             FROM chat_receipts WHERE project_id = ?1
             ORDER BY created_at DESC LIMIT 50",
        )
        .map_err(|error| error.to_string())?;
    let chat_receipts = statement
        .query_map([&project_id], |row| {
            let usage_json: String = row.get(9)?;
            Ok(ExecutionReceipt {
                id: row.get(0)?,
                phase: row.get(1)?,
                participant: row.get(2)?,
                provider_version: row.get(3)?,
                requested_model: row.get(4)?,
                requested_effort: row.get(5)?,
                actual_model: row.get(6)?,
                session_id: row.get(7)?,
                context_bytes: row.get::<_, i64>(8)?.max(0) as usize,
                usage: serde_json::from_str(&usage_json).unwrap_or_default(),
                usage_note: row.get(10)?,
                created_at: row.get(11)?,
                preflight_ms: row
                    .get::<_, Option<i64>>(12)?
                    .map(|value| value.max(0) as u64),
                process_start_ms: row
                    .get::<_, Option<i64>>(13)?
                    .map(|value| value.max(0) as u64),
                first_output_ms: row
                    .get::<_, Option<i64>>(14)?
                    .map(|value| value.max(0) as u64),
                total_ms: row
                    .get::<_, Option<i64>>(15)?
                    .map(|value| value.max(0) as u64),
                session_resumed: row.get::<_, i64>(16)? != 0,
                packet_bytes_saved: row.get::<_, i64>(17)?.max(0) as usize,
                stdout_log_path: row.get(18)?,
                stderr_log_path: row.get(19)?,
            })
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    receipts.extend(chat_receipts);
    receipts.sort_by(|left, right| left.created_at.cmp(&right.created_at));
    Ok(RoomSnapshot {
        messages,
        latest_run,
        receipts,
    })
}

#[tauri::command]
async fn stop_run(runtime: State<'_, RuntimeState>, run_id: String) -> Result<bool, String> {
    let sender = runtime.cancellations.lock().await.get(&run_id).cloned();
    Ok(sender.map(|value| value.send(true).is_ok()).unwrap_or(false))
}

#[tauri::command]
async fn test_provider_connection(
    app: AppHandle,
    database: State<'_, Database>,
    runtime: State<'_, RuntimeState>,
    request: ConnectionTestRequest,
) -> Result<Participant, String> {
    let database = database.inner();
    let repository = PathBuf::from(&request.repository_path);
    git_static(&repository, &["rev-parse", "--show-toplevel"])
        .map_err(|_| "The attached path is not a Git repository.".to_owned())?;
    let participant = probe_provider(&request.participant_kind);
    runtime
        .provider_cache
        .lock()
        .await
        .insert(participant.kind.clone(), participant.clone());
    if !participant.installed {
        return Err(format!("{} CLI is not installed.", participant.name));
    }
    let profile = provider_profile(database, &request.project_id, &participant.kind, "chat")?;
    let run_id = format!("connection-{}", Uuid::new_v4());
    let output_path = run_artifact_directory(&app, &run_id)?.join("connection-test.final.txt");
    let (_, cancellation) = watch::channel(false);
    let result = invoke_provider(
        &app,
        &run_id,
        &participant,
        Phase::Chat,
        false,
        "This is an Agent Room connection test. Reply with exactly READY. Do not inspect or modify files.",
        &repository,
        None,
        profile.model.as_deref(),
        profile.effort.as_deref(),
        &output_path,
        cancellation,
    )
    .await?;
    let ready = connection_test_ready(&result);
    let authentication_detail = authentication_attention(&result.summary, &result.stderr);
    let connection = if ready {
        ProviderConnection {
            status: "connected".to_owned(),
            detail: "Native connection test passed.".to_owned(),
            last_verified_at: None,
        }
    } else {
        let status = if authentication_detail.is_some() {
            "sign-in-required"
        } else {
            "failed"
        };
        ProviderConnection {
            status: status.to_owned(),
            detail: if let Some(detail) = authentication_detail {
                detail
            } else if result.idle_timed_out {
                format!("No provider output within {PROCESS_IDLE_TIMEOUT_SECONDS} seconds.")
            } else if !result.summary.trim().is_empty() {
                format!(
                    "Connection test expected READY but received: {}",
                    truncate_utf8(result.summary.trim(), 320)
                )
            } else {
                provider_failure_reason(&participant.name, "the connection test", &result)
            },
            last_verified_at: None,
        }
    };
    save_connection(
        database,
        &request.project_id,
        &participant.kind,
        &connection,
    )?;
    if connection.status != "connected" {
        return Err(connection.detail);
    }
    participant_for_project(database, &request.project_id, participant)
}

#[tauri::command]
async fn discover_provider_models(
    request: ModelDiscoveryRequest,
) -> Result<ModelDiscoveryResult, String> {
    let executable = find_provider_executable(&request.participant_kind)
        .ok_or_else(|| "Install this CLI before refreshing its models.".to_owned())?;
    match request.participant_kind.as_str() {
        "antigravity" | "cursor" => {
            let output = timeout(Duration::from_secs(15), {
                let mut command = Command::new(executable);
                hide_tokio_command_window(&mut command);
                command.arg("models").output()
            })
            .await
            .map_err(|_| "Model discovery timed out after 15 seconds.".to_owned())?
            .map_err(|error| format!("Could not start model discovery: {error}"))?;
            let text = format!(
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            if !output.status.success() {
                return Err(truncate_utf8(&text, 500));
            }
            let models = if request.participant_kind == "cursor" {
                parse_cursor_model_list(&text)
            } else {
                parse_provider_model_list(&text)
            };
            if models.is_empty() {
                return Err("The CLI returned no selectable models for this account.".to_owned());
            }
            Ok(ModelDiscoveryResult {
                models,
                detail: if request.participant_kind == "cursor" {
                    "Fetched from the signed-in CLI. Effort, thinking, context, and speed variants are grouped under each base model.".to_owned()
                } else {
                    "Fetched from the signed-in CLI account just now.".to_owned()
                },
            })
        }
        "codex" | "claude" => {
            let (models, detail, _) =
                model_options(&request.participant_kind, Some(executable.as_path()));
            Ok(ModelDiscoveryResult { models, detail })
        }
        _ => Err("Unknown provider.".to_owned()),
    }
}

#[tauri::command]
async fn start_room_chat(
    app: AppHandle,
    database: State<'_, Database>,
    runtime: State<'_, RuntimeState>,
    request: ChatRequest,
) -> Result<ChatResult, String> {
    let chat_started = Instant::now();
    let database = database.inner();
    let active_run = if let Some(active_run_id) = request.active_run_id.as_deref() {
        let connection = database.0.lock().map_err(|error| error.to_string())?;
        connection
            .query_row(
                "SELECT worktree_path, current_owner, writer, reviewer, state
                 FROM runs WHERE id = ?1 AND project_id = ?2",
                params![active_run_id, request.project_id],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "The active Ship run could not be found.".to_owned())
            .and_then(|(worktree, current_owner, writer, reviewer, state)| {
                if !run_state_allows_side_chat(&state) {
                    return Err("This Ship run is no longer active.".to_owned());
                }
                let worktree = worktree
                    .ok_or_else(|| "The active Ship worktree is unavailable.".to_owned())?;
                let preferred_owner = if state == "reviewing" {
                    reviewer.clone()
                } else {
                    writer.clone()
                };
                let owner = current_owner
                    .or(preferred_owner)
                    .or(writer)
                    .or(reviewer)
                    .ok_or_else(|| {
                        "The active Ship run has no available participant.".to_owned()
                    })?;
                Ok((PathBuf::from(worktree), owner))
            })?
            .into()
    } else {
        None
    };
    let repository = active_run
        .as_ref()
        .map(|(repository, _)| repository.clone())
        .unwrap_or_else(|| PathBuf::from(&request.repository_path));
    git_static(&repository, &["rev-parse", "--show-toplevel"])
        .map_err(|_| "The attached path is not a Git repository.".to_owned())?;
    let message = request.message.trim();
    if message.is_empty() {
        return Err("Enter a message before sending.".to_owned());
    }
    let requested_agent = active_run
        .as_ref()
        .map(|(_, owner)| owner.as_str())
        .or(request.requested_agent.as_deref());
    let participant = if let Some(requested) = requested_agent {
        let participant = participant_for_project(
            database,
            &request.project_id,
            cached_provider(runtime.inner(), requested, false).await?,
        )?;
        if !participant.installed {
            return Err(format!("The requested {requested} CLI is not installed."));
        }
        participant
    } else {
        participants_with_connections(
            database,
            &request.project_id,
            cached_participants(runtime.inner(), false).await?,
        )?
        .into_iter()
        .find(|participant| participant.installed)
        .ok_or_else(|| "No installed coding-agent CLI is available for chat.".to_owned())?
    };
    let profile = provider_profile(database, &request.project_id, &participant.kind, "chat")?;
    let settings = project_settings(database, &request.project_id)?;
    let session_id = load_chat_session(database, &request.project_id, &participant.kind)?;
    let room_handoff = recent_chat_handoff(
        database,
        &request.project_id,
        &participant.kind,
        session_id.is_some(),
        None,
    )?;
    let (cancel_sender, cancel_receiver) = watch::channel(false);
    runtime
        .cancellations
        .lock()
        .await
        .insert(request.run_id.clone(), cancel_sender);

    persist_message(
        database,
        &request.project_id,
        &request.run_id,
        "human",
        "human",
        message,
        &[],
        &[],
        None,
    )?;
    let chat_mode_detail = if active_run.is_some() {
        "Read-only side chat attached to the active managed worktree. The autonomous Ship route continues separately."
    } else {
        "Instant project chat. No worktree, verification, review, or promotion."
    };
    emit_event(
        &app,
        &request.run_id,
        "phase",
        "chat",
        "working",
        Some(&participant.kind),
        &format!("{} is answering", participant.name),
        chat_mode_detail,
        None,
    );
    let chat_can_write = active_run.is_none() && participant.capabilities.write_mode;
    let mut prompt = if active_run.is_some() {
        format!(
            "You are in a read-only side chat attached to an active Agent Room Ship worktree. \
             Answer the user's question directly and concisely using the worktree's current state. \
             Do not modify files, interrupt or steer the active builder, start another Ship run, \
             run full project verification, or emit an Agent Room Ship intent or handoff.\n\nUser message:\n{message}"
        )
    } else {
        format!(
            "You are in the current repository's fast working chat. Answer questions directly and concisely. \
             When the user explicitly requests a small, bounded repository edit, make that edit directly in the attached checkout and report what changed. \
             Do not create a worktree, run broad project verification, or use an Agent Room handoff. \
             For substantial or unattended implementation work, do not edit first; use the autonomous Ship intent when its trigger matches.\n\nUser message:\n{message}"
        )
    };
    if let Some(handoff) = room_handoff {
        prompt.push_str("\n\n");
        prompt.push_str(&handoff);
    }
    if settings.autonomous_ship_enabled && active_run.is_none() {
        prompt.push_str(&format!(
            "\n\nAutonomous Ship is armed for this project. Apply the following skill when its trigger matches. The skill is `{AUTONOMOUS_SHIP_SKILL_PATH}`:\n\n{AUTONOMOUS_SHIP_SKILL}"
        ));
    }
    let artifact_dir = run_artifact_directory(&app, &request.run_id)?;
    let output_path = artifact_dir.join("chat.final.txt");
    let preflight_ms = chat_started.elapsed().as_millis() as u64;
    let first_result = invoke_provider(
        &app,
        &request.run_id,
        &participant,
        Phase::Chat,
        chat_can_write,
        &prompt,
        &repository,
        session_id.as_deref(),
        profile.model.as_deref(),
        profile.effort.as_deref(),
        &output_path,
        cancel_receiver.clone(),
    )
    .await;
    let result = first_result;
    runtime.cancellations.lock().await.remove(&request.run_id);
    let result = match result {
        Ok(result) => result,
        Err(error) => {
            runtime
                .provider_cache
                .lock()
                .await
                .remove(&participant.kind);
            return Err(error);
        }
    };
    let total_ms = chat_started.elapsed().as_millis() as u64;
    save_chat_session(
        database,
        &request.project_id,
        &participant.kind,
        result.session_id.as_deref(),
    )?;
    persist_chat_receipt(
        database,
        &request.project_id,
        &request.run_id,
        &participant,
        &profile,
        ChatReceiptMetrics {
            context_bytes: prompt.len(),
            preflight_ms,
            total_ms,
        },
        &result,
    )?;
    let authentication_detail = authentication_attention(&result.summary, &result.stderr);
    if !result.success || authentication_detail.is_some() {
        let reason = if let Some(detail) = authentication_detail {
            detail
        } else if result.stopped {
            "Chat stopped by you.".to_owned()
        } else if result.timed_out {
            "Chat exceeded the 20 minute limit.".to_owned()
        } else if result.idle_timed_out {
            format!(
                "Chat produced no output for {PROCESS_IDLE_TIMEOUT_SECONDS} seconds and was stopped."
            )
        } else {
            provider_failure_reason(&participant.name, "the chat response", &result)
        };
        let status = if authentication_attention(&result.summary, &result.stderr).is_some() {
            "sign-in-required"
        } else {
            "failed"
        };
        save_connection(
            database,
            &request.project_id,
            &participant.kind,
            &ProviderConnection {
                status: status.to_owned(),
                detail: reason.clone(),
                last_verified_at: None,
            },
        )?;
        persist_message(
            database,
            &request.project_id,
            &request.run_id,
            "system",
            "error",
            "Chat needs attention.",
            &[],
            &[],
            Some(&reason),
        )?;
        emit_event(
            &app,
            &request.run_id,
            "attention",
            "chat",
            if result.stopped { "stopped" } else { "failed" },
            Some(&participant.kind),
            "Chat needs attention",
            &reason,
            None,
        );
        return Err(reason);
    }

    save_connection(
        database,
        &request.project_id,
        &participant.kind,
        &ProviderConnection {
            status: "connected".to_owned(),
            detail: "Most recent native project chat completed.".to_owned(),
            last_verified_at: None,
        },
    )?;

    let (ship_intent, intent_diagnostic) = if settings.autonomous_ship_enabled {
        match extract_ship_intent(&result.summary) {
            Ok(intent) => (intent, None),
            Err(error) => (None, Some(error)),
        }
    } else {
        (None, None)
    };
    let visible_summary = visible_chat_response(&result.summary);
    persist_message(
        database,
        &request.project_id,
        &request.run_id,
        &participant.kind,
        "agent",
        &visible_summary,
        &[],
        &[],
        None,
    )?;
    if let Some(intent) = ship_intent.as_ref() {
        persist_message(
            database,
            &request.project_id,
            &request.run_id,
            "system",
            "status",
            "Autonomous Ship intent accepted.",
            &[],
            &[],
            Some(&format!("{}\n{}", intent.objective, intent.reason)),
        )?;
    } else if let Some(diagnostic) = intent_diagnostic {
        persist_message(
            database,
            &request.project_id,
            &request.run_id,
            "system",
            "status",
            "Autonomous Ship intent was ignored.",
            &[],
            &[],
            Some(&diagnostic),
        )?;
    }
    emit_event(
        &app,
        &request.run_id,
        "complete",
        "chat",
        "complete",
        Some(&participant.kind),
        "Chat complete",
        "Project chat response saved to this room.",
        None,
    );
    Ok(ChatResult {
        run_id: request.run_id,
        participant: participant.kind.clone(),
        summary: visible_summary,
        session_id: result.session_id,
        actual_model: result.actual_model,
        stopped: false,
        ship_intent,
    })
}

async fn execute_room_run(
    app: AppHandle,
    database: &Database,
    runtime: &RuntimeState,
    request: StartRunRequest,
) -> Result<StartRunResult, String> {
    let base_repository = PathBuf::from(&request.repository_path);
    git_static(&base_repository, &["rev-parse", "--show-toplevel"])
        .map_err(|_| "The attached path is not a Git repository.".to_owned())?;

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
    let (cancel_sender, cancel_receiver) = watch::channel(false);
    runtime
        .cancellations
        .lock()
        .await
        .insert(request.run_id.clone(), cancel_sender);

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
                "INSERT OR IGNORE INTO projects (id, name, goal, repository_path)
                 VALUES (?1, 'Agent Room', 'Remove manual context transfer between coding agents.', ?2)",
                params![request.project_id, request.repository_path],
            )
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
            false,
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
        build_sections.insert(
            1,
            (
                "Recent room context",
                format!(
                    "The objective refers to the recent conversation. Resolve pronouns from this bounded handoff and implement the previously described change:\n{context}"
                ),
            ),
        );
    }
    let (build_packet, build_context_bytes) = assemble_packet(&build_sections);
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
        true,
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
    let review_started = Instant::now();
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
            diff_evidence(&worktree, &snapshot_head),
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
    let review_preflight_ms = review_started.elapsed().as_millis() as u64;
    let review_result = invoke_provider(
        &app,
        &request.run_id,
        reviewer,
        Phase::Review,
        false,
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
        let (revision_instructions, _) = repository_instructions(&worktree, &files);
        let (revision_packet, revision_context_bytes) = assemble_packet(&[
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
        let revision_result = invoke_provider(
            &app,
            &request.run_id,
            builder,
            Phase::Revise,
            true,
            &revision_packet,
            &worktree,
            build_result.session_id.as_deref(),
            builder_profile.model.as_deref(),
            builder_profile.effort.as_deref(),
            &revision_output_path,
            cancel_receiver.clone(),
        )
        .await?;
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
            revision_context_bytes,
            Some(&truncate_utf8(&revision_result.summary, 8 * 1024)),
        )?;
        persist_receipt(
            database,
            &request.run_id,
            Phase::Revise,
            builder,
            &builder_profile,
            ReceiptMetrics {
                context_bytes: revision_context_bytes,
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
        let final_review_started = Instant::now();
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
                diff_evidence(&worktree, &snapshot_head),
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
        let final_review_preflight_ms = final_review_started.elapsed().as_millis() as u64;
        let final_result = invoke_provider(
            &app,
            &request.run_id,
            reviewer,
            Phase::FinalReview,
            false,
            &final_packet,
            &worktree,
            review_result.session_id.as_deref(),
            reviewer_profile.model.as_deref(),
            reviewer_profile.effort.as_deref(),
            &final_output_path,
            cancel_receiver.clone(),
        )
        .await?;
        let final_review_total_ms = final_review_started.elapsed().as_millis() as u64;
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
        persist_receipt(
            database,
            &request.run_id,
            Phase::FinalReview,
            reviewer,
            &reviewer_profile,
            ReceiptMetrics {
                context_bytes: final_context_bytes,
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
    let has_changes = git_static(&worktree, &["rev-parse", "HEAD"])? != snapshot_head;
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

fn run_state_allows_side_chat(state: &str) -> bool {
    matches!(
        state,
        "selecting" | "working" | "verifying" | "reviewing" | "revising"
    )
}

fn mark_run_and_activation_failed(
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

fn finalize_unhandled_run_error(
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

#[tauri::command]
async fn start_room_run(
    app: AppHandle,
    database: State<'_, Database>,
    runtime: State<'_, RuntimeState>,
    request: StartRunRequest,
) -> Result<StartRunResult, String> {
    let run_id = request.run_id.clone();
    let project_id = request.project_id.clone();
    let result = execute_room_run(app.clone(), database.inner(), runtime.inner(), request).await;
    if let Err(error) = result.as_ref() {
        let _ = finalize_unhandled_run_error(&app, database.inner(), &project_id, &run_id, error);
        runtime.cancellations.lock().await.remove(&run_id);
    }
    result
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_directory = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_directory)?;
            let connection = Connection::open(data_directory.join("agent-room.db"))?;
            migrate(&connection)?;
            reconcile_interrupted_runs(&connection)?;
            app.manage(Database(Mutex::new(connection)));
            app.manage(RuntimeState::default());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_environment,
            project_pick,
            project_attach,
            project_list,
            project_select,
            project_active,
            load_project_settings,
            save_project_settings,
            load_provider_profiles,
            save_provider_profile,
            load_room,
            test_provider_connection,
            discover_provider_models,
            start_room_chat,
            start_room_run,
            stop_run
        ])
        .run(tauri::generate_context!())
        .expect("error while running Agent Room");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_repository() -> (PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!("agent-room-test-{}", Uuid::new_v4()));
        let repository = root.join("repository");
        std::fs::create_dir_all(&repository).expect("create test repository");
        git(&repository, &["init".to_owned()]).expect("initialize repository");
        git(
            &repository,
            &[
                "config".to_owned(),
                "user.name".to_owned(),
                "Agent Room Test".to_owned(),
            ],
        )
        .expect("configure test name");
        git(
            &repository,
            &[
                "config".to_owned(),
                "user.email".to_owned(),
                "agent-room-test@local".to_owned(),
            ],
        )
        .expect("configure test email");
        std::fs::write(repository.join("plan.md"), "committed\n").expect("write tracked file");
        git_static(&repository, &["add", "plan.md"]).expect("stage tracked file");
        git_static(&repository, &["commit", "-m", "Initial"]).expect("commit tracked file");
        (root, repository)
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

        let first_room = load_room_snapshot(&database, &first.id).expect("load first room");
        let second_room = load_room_snapshot(&database, &second.id).expect("load second room");
        assert_eq!(first_room.messages.len(), 1);
        assert_eq!(first_room.messages[0].body, "first room");
        assert_eq!(second_room.messages.len(), 1);
        assert_eq!(second_room.messages[0].body, "second room");

        std::fs::remove_dir_all(first_root).expect("remove first test repository");
        std::fs::remove_dir_all(second_root).expect("remove second test repository");
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

        assert_eq!(project_row_counts(&connection).expect("count migrated rows"), before);
        let project_id = project_id_for_root(&std::fs::canonicalize(&repository).expect("canonical repository"));
        let message_project_id: String = connection
            .query_row("SELECT project_id FROM messages WHERE id = 'legacy-message'", [], |row| row.get(0))
            .expect("load migrated message");
        assert_eq!(message_project_id, project_id);
        std::fs::remove_dir_all(root).expect("remove fixture repository");
    }

    fn remove_test_repository(root: &Path) {
        assert!(root.starts_with(std::env::temp_dir()));
        std::fs::remove_dir_all(root).expect("remove test repository");
    }

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
        assert!(
            provider_activity("codex", &serde_json::json!({"type": "thread.started"})).is_none()
        );
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
                 VALUES ('project-1', 'Agent Room', 'Test failure', 'C:\\repo')",
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
        let error = promote_worktree(&repository, &isolation, &managed_root)
            .expect_err("promotion must stop");

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
        std::fs::write(repository.join("binary.bin"), [0_u8, 255, 2, 3])
            .expect("modify binary file");
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
        promote_worktree(&repository, &isolation, &managed_root)
            .expect("apply complex verified delta");

        assert_eq!(
            git_static(&repository, &["diff", "--cached", "--binary"])
                .expect("read preserved index"),
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
                 VALUES ('project-1', 'Agent Room', 'Test restart', 'C:\\repo')",
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

        assert_eq!(
            reconcile_interrupted_runs(&connection).expect("reconcile runs"),
            1
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
    }

    #[test]
    fn chat_receipt_does_not_require_an_autonomous_run() {
        let connection = Connection::open_in_memory().expect("open test database");
        migrate(&connection).expect("migrate test database");
        connection
            .execute(
                "INSERT INTO projects (id, name, goal, repository_path)
                 VALUES (?1, ?2, ?3, ?4)",
                params!["project-1", "Agent Room", "Test chat", "C:\\repo"],
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

        let snapshot = load_room_snapshot(&database, "project-1").expect("load room");
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
    fn cross_provider_handoff_is_bounded_and_skips_resumed_owner() {
        let connection = Connection::open_in_memory().expect("open test database");
        migrate(&connection).expect("migrate test database");
        connection
            .execute(
                "INSERT INTO projects (id, name, goal, repository_path)
                 VALUES (?1, ?2, ?3, ?4)",
                params!["project-1", "Agent Room", "Test chat", "C:\\repo"],
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
            recent_chat_handoff(&database, "project-1", "codex", true, None)
                .expect("same-provider handoff")
                .is_none()
        );
        let switched = recent_chat_handoff(&database, "project-1", "claude", true, None)
            .expect("cross-provider handoff")
            .expect("handoff exists");
        assert!(switched.contains("Codex"));
        assert!(switched.contains("Explain the architecture."));
        assert!(switched.len() <= CHAT_HANDOFF_BUDGET_BYTES);
        assert!(
            recent_chat_handoff(&database, "project-1", "codex", false, None)
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
        let recovery_context =
            recent_chat_handoff(&database, "project-1", "codex", false, Some("ship-1"))
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
        assert_eq!(command.args, [
            "--print", "--output-format", "stream-json", "--stream-partial-output", "--trust",
            "--workspace", "C:/worktree", "--sandbox", "enabled", "--mode", "ask",
            "--model", "claude-opus-4-8[effort=high]", "--resume", "chat-1", "Read the packet."
        ]);
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
        let punctuated = ProviderRun { summary: "```READY.```".to_owned(), ..ready };
        assert!(connection_test_ready(&punctuated));
        assert!(
            authentication_attention("This repository uses OAuth authentication.", "").is_none()
        );
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

}
