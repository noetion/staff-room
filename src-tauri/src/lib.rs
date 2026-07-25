use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Command as StdCommand,
    sync::Mutex,
};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    sync::{oneshot, Mutex as AsyncMutex},
};
use uuid::Uuid;

struct Database(Mutex<Connection>);

#[derive(Default)]
struct RuntimeState {
    cancellations: AsyncMutex<HashMap<String, oneshot::Sender<()>>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderCapabilities {
    non_interactive_turn: bool,
    streaming: bool,
    structured_output: bool,
    exact_resume: bool,
    cancellation: bool,
    write_mode: bool,
}

#[derive(Debug, Serialize)]
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
struct ActivationEvent {
    run_id: String,
    stream: String,
    payload: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StartRunResult {
    run_id: String,
    summary: String,
    session_id: Option<String>,
    changed_files: Vec<String>,
    git_status: String,
    stopped: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct ProjectInput {
    id: String,
    name: String,
    goal: String,
    repository_path: String,
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
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY(project_id) REFERENCES projects(id)
        );

        CREATE TABLE IF NOT EXISTS runs (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            objective TEXT NOT NULL,
            state TEXT NOT NULL,
            current_owner TEXT,
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
        ",
    )
}

fn command_output(
    executable: &Path,
    args: &[&str],
    working_directory: Option<&Path>,
) -> Option<String> {
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

fn probe(
    kind: &str,
    name: &str,
    executable_names: &[&str],
    structured: bool,
    streaming: bool,
    exact_resume: bool,
) -> Participant {
    let path = find_executable(executable_names);
    let version = path
        .as_deref()
        .and_then(|executable| command_output(executable, &["--version"], None))
        .and_then(|value| value.lines().next().map(str::to_owned));
    Participant {
        kind: kind.to_owned(),
        name: name.to_owned(),
        installed: path.is_some(),
        version,
        executable_path: path
            .as_ref()
            .map(|value| value.to_string_lossy().into_owned()),
        state: if path.is_some() {
            "ready"
        } else {
            "unavailable"
        }
        .to_owned(),
        capabilities: ProviderCapabilities {
            non_interactive_turn: true,
            streaming,
            structured_output: structured,
            exact_resume,
            cancellation: true,
            write_mode: true,
        },
    }
}

fn git_output(repository: &Path, args: &[&str]) -> String {
    find_executable(&["git"])
        .and_then(|git| command_output(&git, args, Some(repository)))
        .unwrap_or_else(|| "Git evidence unavailable".to_owned())
}

fn changed_files(repository: &Path) -> Vec<String> {
    let output = git_output(repository, &["status", "--short"]);
    output
        .lines()
        .filter_map(|line| line.get(3..).map(str::trim))
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

fn find_session_id(value: &Value) -> Option<String> {
    match value {
        Value::Object(map) => {
            for key in ["thread_id", "threadId", "session_id", "sessionId"] {
                if let Some(Value::String(id)) = map.get(key) {
                    return Some(id.clone());
                }
            }
            map.values().find_map(find_session_id)
        }
        Value::Array(values) => values.iter().find_map(find_session_id),
        _ => None,
    }
}

#[tauri::command]
fn get_environment() -> Result<NativeEnvironment, String> {
    let current_directory = std::env::current_dir().map_err(|error| error.to_string())?;
    let repository = find_executable(&["git"])
        .and_then(|git| {
            command_output(
                &git,
                &["rev-parse", "--show-toplevel"],
                Some(&current_directory),
            )
        })
        .map(PathBuf::from)
        .filter(|path| path.is_dir())
        .unwrap_or(current_directory);
    let branch = git_output(&repository, &["branch", "--show-current"]);
    Ok(NativeEnvironment {
        native: true,
        repository_path: repository.to_string_lossy().into_owned(),
        branch,
        participants: vec![
            probe("codex", "Codex", &["codex"], true, true, true),
            probe("claude", "Claude", &["claude"], true, true, true),
            probe("cursor", "Cursor", &["cursor-agent"], true, true, true),
            probe("antigravity", "Antigravity", &["agy"], false, false, true),
        ],
    })
}

#[tauri::command]
fn save_project(database: State<'_, Database>, project: ProjectInput) -> Result<(), String> {
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
async fn stop_run(runtime: State<'_, RuntimeState>, run_id: String) -> Result<bool, String> {
    let sender = runtime.cancellations.lock().await.remove(&run_id);
    Ok(sender.map(|value| value.send(()).is_ok()).unwrap_or(false))
}

#[tauri::command]
async fn start_codex_run(
    app: AppHandle,
    database: State<'_, Database>,
    runtime: State<'_, RuntimeState>,
    project_id: String,
    objective: String,
    repository_path: String,
) -> Result<StartRunResult, String> {
    let repository = PathBuf::from(&repository_path);
    if !repository.is_dir() || !repository.join(".git").exists() {
        return Err("The attached path is not a Git repository.".to_owned());
    }
    let codex =
        find_executable(&["codex"]).ok_or_else(|| "Codex CLI is not installed.".to_owned())?;
    let run_id = Uuid::new_v4().to_string();
    let human_message_id = Uuid::new_v4().to_string();

    {
        let connection = database.0.lock().map_err(|error| error.to_string())?;
        connection
            .execute(
                "INSERT OR IGNORE INTO projects (id, name, goal, repository_path) VALUES (?1, ?2, ?3, ?4)",
                params![project_id, "Agent Room", "Local agent coordination", repository_path],
            )
            .map_err(|error| error.to_string())?;
        let transaction = connection
            .unchecked_transaction()
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "INSERT INTO runs (id, project_id, objective, state, current_owner) VALUES (?1, ?2, ?3, 'working', 'codex')",
                params![run_id, project_id, objective],
            )
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "INSERT INTO messages (id, project_id, run_id, sender_kind, message_kind, body)
                 VALUES (?1, ?2, ?3, 'human', 'human', ?4)",
                params![human_message_id, project_id, run_id, objective],
            )
            .map_err(|error| error.to_string())?;
        transaction.commit().map_err(|error| error.to_string())?;
    }

    let cache_directory = app
        .path()
        .app_cache_dir()
        .map_err(|error| error.to_string())?;
    std::fs::create_dir_all(&cache_directory).map_err(|error| error.to_string())?;
    let final_output_path = cache_directory.join(format!("{run_id}-final.txt"));

    let mut child = Command::new(codex)
        .args([
            "exec",
            "--json",
            "--sandbox",
            "workspace-write",
            "--cd",
            repository.to_string_lossy().as_ref(),
            "--output-last-message",
            final_output_path.to_string_lossy().as_ref(),
            &objective,
        ])
        .current_dir(&repository)
        .kill_on_drop(true)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|error| format!("Failed to start Codex: {error}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Codex stdout was unavailable.".to_owned())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Codex stderr was unavailable.".to_owned())?;
    let (cancel_sender, cancel_receiver) = oneshot::channel();
    runtime
        .cancellations
        .lock()
        .await
        .insert(run_id.clone(), cancel_sender);

    let stdout_app = app.clone();
    let stdout_run_id = run_id.clone();
    let stdout_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        let mut session_id = None;
        while let Ok(Some(line)) = lines.next_line().await {
            if let Ok(value) = serde_json::from_str::<Value>(&line) {
                session_id = session_id.or_else(|| find_session_id(&value));
            }
            let _ = stdout_app.emit(
                "activation-event",
                ActivationEvent {
                    run_id: stdout_run_id.clone(),
                    stream: "stdout".to_owned(),
                    payload: line,
                },
            );
        }
        session_id
    });

    let stderr_app = app.clone();
    let stderr_run_id = run_id.clone();
    let stderr_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = stderr_app.emit(
                "activation-event",
                ActivationEvent {
                    run_id: stderr_run_id.clone(),
                    stream: "stderr".to_owned(),
                    payload: line,
                },
            );
        }
    });

    let mut stopped = false;
    let status = tokio::select! {
        status = child.wait() => status.map_err(|error| error.to_string())?,
        _ = cancel_receiver => {
            stopped = true;
            child.kill().await.map_err(|error| format!("Failed to stop Codex: {error}"))?;
            child.wait().await.map_err(|error| error.to_string())?
        }
    };
    runtime.cancellations.lock().await.remove(&run_id);
    let session_id = stdout_task.await.map_err(|error| error.to_string())?;
    let _ = stderr_task.await;

    let summary = std::fs::read_to_string(&final_output_path).unwrap_or_default();
    let files = changed_files(&repository);
    let git_status = git_output(&repository, &["status", "--short", "--branch"]);
    let final_state = if stopped {
        "stopped"
    } else if status.success() {
        "complete"
    } else {
        "failed"
    };
    let result_message_id = Uuid::new_v4().to_string();

    {
        let connection = database.0.lock().map_err(|error| error.to_string())?;
        let transaction = connection
            .unchecked_transaction()
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "UPDATE runs SET state = ?1, stop_reason = ?2, finished_at = CURRENT_TIMESTAMP WHERE id = ?3",
                params![
                    final_state,
                    if stopped { Some("Stopped by user") } else if status.success() { None } else { Some("Provider process failed") },
                    run_id
                ],
            )
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "INSERT INTO messages (id, project_id, run_id, sender_kind, message_kind, body)
                 VALUES (?1, ?2, ?3, 'codex', ?4, ?5)",
                params![
                    result_message_id,
                    project_id,
                    run_id,
                    if status.success() { "agent" } else { "error" },
                    if summary.is_empty() {
                        "Codex did not return a final summary."
                    } else {
                        &summary
                    }
                ],
            )
            .map_err(|error| error.to_string())?;
        if let Some(id) = &session_id {
            transaction
                .execute(
                    "INSERT INTO provider_sessions (project_id, participant_kind, provider_session_id)
                     VALUES (?1, 'codex', ?2)
                     ON CONFLICT(project_id, participant_kind) DO UPDATE SET
                       provider_session_id = excluded.provider_session_id,
                       last_used_at = CURRENT_TIMESTAMP",
                    params![project_id, id],
                )
                .map_err(|error| error.to_string())?;
        }
        transaction.commit().map_err(|error| error.to_string())?;
    }

    if !stopped && !status.success() {
        return Err(format!(
            "Codex exited with status {}. Repository changes may be partial.\n{}",
            status, git_status
        ));
    }

    Ok(StartRunResult {
        run_id,
        summary,
        session_id,
        changed_files: files,
        git_status,
        stopped,
    })
}

pub fn run() {
    tauri::Builder::default()
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
            start_codex_run,
            stop_run
        ])
        .run(tauri::generate_context!())
        .expect("error while running Agent Room");
}
