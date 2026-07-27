use crate::*;

#[tauri::command]
pub(crate) async fn get_environment(
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
            context_budget_bytes: BUILD_CONTEXT_BUDGET_BYTES,
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
        context_budget_bytes: BUILD_CONTEXT_BUDGET_BYTES,
    })
}

#[tauri::command]
pub(crate) async fn project_pick(app: AppHandle) -> Result<Option<String>, String> {
    Ok(app
        .dialog()
        .file()
        .blocking_pick_folder()
        .map(|path| path.to_string()))
}

#[tauri::command]
pub(crate) fn project_attach(
    database: State<'_, Database>,
    path: String,
) -> Result<Project, String> {
    attach_project(database.inner(), &path)
}

#[tauri::command]
pub(crate) fn project_list(database: State<'_, Database>) -> Result<Vec<Project>, String> {
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    let mut statement = connection
        .prepare(
            "SELECT id, name, goal, repository_path FROM projects ORDER BY updated_at DESC, name",
        )
        .map_err(|error| error.to_string())?;
    let projects = statement
        .query_map([], |row| {
            Ok(project_from_parts(
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
            ))
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    Ok(projects)
}

#[tauri::command]
pub(crate) fn project_select(database: State<'_, Database>, id: String) -> Result<Project, String> {
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    let project = connection
        .query_row(
            "SELECT id, name, goal, repository_path FROM projects WHERE id = ?1",
            [&id],
            |row| {
                Ok(project_from_parts(
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                ))
            },
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
pub(crate) fn project_active(database: State<'_, Database>) -> Result<Option<Project>, String> {
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    active_project(&connection)
}

#[tauri::command]
pub(crate) fn load_project_settings(
    database: State<'_, Database>,
    project_id: String,
) -> Result<ProjectSettings, String> {
    project_settings(database.inner(), &project_id)
}

pub(crate) fn project_settings(
    database: &Database,
    project_id: &str,
) -> Result<ProjectSettings, String> {
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
pub(crate) fn save_project_settings(
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
pub(crate) fn load_verification_config(
    database: State<'_, Database>,
    project_id: String,
) -> Result<VerificationConfig, String> {
    verification_config(database.inner(), &project_id)
}

pub(crate) fn verification_config(
    database: &Database,
    project_id: &str,
) -> Result<VerificationConfig, String> {
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    connection
        .query_row(
            "SELECT enabled, commands_json, prepare_command FROM verification_config WHERE project_id = ?1",
            [project_id],
            |row| {
                let commands_json: String = row.get(1)?;
                let commands = serde_json::from_str(&commands_json).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        commands_json.len(),
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
                Ok(VerificationConfig {
                    enabled: row.get::<_, i64>(0)? != 0,
                    commands,
                    prepare: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(|error| error.to_string())
        .map(|config| config.unwrap_or_default())
}

#[tauri::command]
pub(crate) fn save_verification_config(
    database: State<'_, Database>,
    config: VerificationConfigInput,
) -> Result<(), String> {
    let commands = config
        .commands
        .into_iter()
        .filter_map(|command| {
            let label = command.label.trim().to_owned();
            let command_text = command.command.trim().to_owned();
            (!label.is_empty() && !command_text.is_empty()).then_some(VerificationCommand {
                label,
                command: command_text,
                enabled: command.enabled,
            })
        })
        .take(4)
        .collect::<Vec<_>>();
    let prepare = config.prepare.and_then(|command| {
        let command = command.trim().to_owned();
        (!command.is_empty()).then_some(command)
    });
    let commands = serde_json::to_string(&commands).map_err(|error| error.to_string())?;
    let connection = database.0.lock().map_err(|error| error.to_string())?;
    connection
        .execute(
            "INSERT INTO verification_config (project_id, enabled, commands_json, prepare_command)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(project_id) DO UPDATE SET
               enabled = excluded.enabled,
               commands_json = excluded.commands_json,
               prepare_command = excluded.prepare_command,
               updated_at = CURRENT_TIMESTAMP",
            params![
                config.project_id,
                i64::from(config.enabled),
                commands,
                prepare
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}
