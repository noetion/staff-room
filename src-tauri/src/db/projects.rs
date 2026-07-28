use crate::*;

pub(crate) fn project_id_for_root(repository: &Path) -> String {
    let digest = Sha256::digest(repository.to_string_lossy().as_bytes());
    digest
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(crate) fn canonical_repository_root(path: &Path) -> Result<PathBuf, String> {
    let canonical_path = std::fs::canonicalize(path)
        .map_err(|error| format!("Cannot read repository path: {error}"))?;
    if !canonical_path.is_dir() {
        return Err("The attached path is not a directory.".to_owned());
    }
    let root = git_static(&canonical_path, &["rev-parse", "--show-toplevel"])
        .map_err(|_| "The attached path is not a Git repository.".to_owned())?;
    std::fs::canonicalize(root).map_err(|error| format!("Cannot read Git repository root: {error}"))
}

pub(crate) fn project_from_parts(
    id: String,
    name: String,
    goal: String,
    repository_path: String,
) -> Project {
    let branch =
        git_static(Path::new(&repository_path), &["branch", "--show-current"]).unwrap_or_default();
    Project {
        id,
        name,
        goal,
        repository_path,
        branch,
    }
}

pub(crate) fn active_project(connection: &Connection) -> Result<Option<Project>, String> {
    connection
        .query_row(
            "SELECT projects.id, projects.name, projects.goal, projects.repository_path
             FROM app_state JOIN projects ON projects.id = app_state.value
             WHERE app_state.key = 'active_project_id'",
            [],
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
        .map_err(|error| error.to_string())
}

pub(crate) fn attach_project(database: &Database, path: &str) -> Result<Project, String> {
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
    ensure_verification_config(&connection, &id, &root)?;
    connection
        .execute(
            "INSERT INTO app_state (key, value) VALUES ('active_project_id', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [&id],
        )
        .map_err(|error| error.to_string())?;
    active_project(&connection)?.ok_or_else(|| "Attached project could not be loaded.".to_owned())
}

pub(crate) fn detected_verification_config(repository: &Path) -> VerificationConfig {
    let mut commands = Vec::new();
    let mut prepare_steps = Vec::new();
    let package = repository.join("package.json");
    if package.is_file() {
        if let Ok(contents) = std::fs::read_to_string(&package) {
            if let Ok(value) = serde_json::from_str::<Value>(&contents) {
                if let Some(scripts) = value.get("scripts").and_then(Value::as_object) {
                    for script in ["test", "build", "lint"] {
                        if scripts.contains_key(script) {
                            commands.push(VerificationCommand {
                                label: format!("npm {script}"),
                                command: format!("npm run {script}"),
                                enabled: true,
                            });
                        }
                    }
                }
            }
        }
        prepare_steps.push(if repository.join("package-lock.json").is_file() {
            "npm ci --prefer-offline".to_owned()
        } else {
            "npm install".to_owned()
        });
    }
    if repository.join("src-tauri").join("Cargo.toml").is_file() {
        commands.push(VerificationCommand {
            label: "cargo check".to_owned(),
            command: "cargo check --manifest-path src-tauri/Cargo.toml".to_owned(),
            enabled: true,
        });
        prepare_steps.push("cargo fetch --locked --manifest-path src-tauri/Cargo.toml".to_owned());
    } else if repository.join("Cargo.toml").is_file() {
        commands.push(VerificationCommand {
            label: "cargo check".to_owned(),
            command: "cargo check".to_owned(),
            enabled: true,
        });
        prepare_steps.push("cargo fetch --locked".to_owned());
    }
    commands.truncate(4);
    VerificationConfig {
        enabled: true,
        commands,
        prepare: (!prepare_steps.is_empty()).then(|| prepare_steps.join(" && ")),
    }
}

pub(crate) fn ensure_verification_config(
    connection: &Connection,
    project_id: &str,
    repository: &Path,
) -> Result<(), String> {
    let config = detected_verification_config(repository);
    let commands = serde_json::to_string(&config.commands).map_err(|error| error.to_string())?;
    connection
        .execute(
            "INSERT OR IGNORE INTO verification_config (project_id, enabled, commands_json, prepare_command)
             VALUES (?1, ?2, ?3, ?4)",
            params![project_id, i64::from(config.enabled), commands, config.prepare],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) fn project_row_counts(
    connection: &Connection,
) -> Result<Vec<(&'static str, i64)>, rusqlite::Error> {
    PROJECT_SCOPED_TABLES
        .iter()
        .map(|table| {
            connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .map(|count| (*table, count))
        })
        .collect()
}

pub(crate) fn backup_v1_database(connection: &Connection) -> Result<(), String> {
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
        let temporary = path.with_extension(format!("v1-{}.backup.tmp", Uuid::new_v4()));
        connection
            .backup(rusqlite::DatabaseName::Main, &temporary, None)
            .map_err(|error| {
                format!("Failed to create a consistent v1 database backup: {error}")
            })?;
        let backup_connection = Connection::open(&temporary)
            .map_err(|error| format!("Failed to verify the v1 database backup: {error}"))?;
        let integrity: String = backup_connection
            .query_row("PRAGMA quick_check", [], |row| row.get(0))
            .map_err(|error| format!("Failed to verify the v1 database backup: {error}"))?;
        drop(backup_connection);
        if integrity != "ok" {
            let _ = std::fs::remove_file(&temporary);
            return Err(format!(
                "The v1 database backup failed its integrity check: {integrity}"
            ));
        }
        if let Err(error) = std::fs::rename(&temporary, &backup) {
            let _ = std::fs::remove_file(&temporary);
            if !backup.exists() {
                return Err(format!(
                    "Failed to finalize the v1 database backup: {error}"
                ));
            }
        }
    }
    Ok(())
}

pub(crate) fn backup_v1_database_before_migration(connection: &Connection) -> rusqlite::Result<()> {
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

pub(crate) fn migrate_v1_to_v2(connection: &Connection) -> rusqlite::Result<()> {
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
    let canonical_project_exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1)",
        [&project_id],
        |row| row.get(0),
    )?;
    if canonical_project_exists {
        return merge_legacy_project(connection, &project_id);
    }
    let row_counts = project_row_counts(connection)?;
    connection.execute_batch("PRAGMA foreign_keys = OFF;")?;
    let result = (|| {
        let transaction = connection.unchecked_transaction()?;
        for table in PROJECT_SCOPED_TABLES {
            let column = if *table == "projects" {
                "id"
            } else {
                "project_id"
            };
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

pub(crate) fn project_repository(database: &Database, project_id: &str) -> Result<PathBuf, String> {
    let stored_path = {
        let connection = database.0.lock().map_err(|error| error.to_string())?;
        connection
            .query_row(
                "SELECT repository_path FROM projects WHERE id = ?1",
                [project_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "The selected project is no longer attached.".to_owned())?
    };
    let repository = canonical_repository_root(Path::new(&stored_path))?;
    if project_id_for_root(&repository) != project_id {
        return Err(
            "The selected project's repository identity no longer matches its stored record. Reattach the repository before running an agent."
                .to_owned(),
        );
    }
    Ok(repository)
}

fn merge_legacy_project(connection: &Connection, project_id: &str) -> rusqlite::Result<()> {
    let transaction = connection.unchecked_transaction()?;

    for table in ["messages", "runs", "chat_receipts"] {
        transaction.execute(
            &format!("UPDATE {table} SET project_id = ?1 WHERE project_id = 'agent-room'"),
            [project_id],
        )?;
    }

    for (table, key) in [
        ("chat_sessions", "participant_kind"),
        ("provider_sessions", "participant_kind"),
        ("provider_profiles", "participant_kind"),
        ("provider_connections", "participant_kind"),
    ] {
        transaction.execute(
            &format!(
                "DELETE FROM {table}
                 WHERE project_id = 'agent-room'
                   AND {key} IN (
                       SELECT {key} FROM {table} WHERE project_id = ?1
                   )"
            ),
            [project_id],
        )?;
        transaction.execute(
            &format!("UPDATE {table} SET project_id = ?1 WHERE project_id = 'agent-room'"),
            [project_id],
        )?;
    }

    transaction.execute(
        "DELETE FROM provider_route_profiles
         WHERE project_id = 'agent-room'
           AND (participant_kind, route) IN (
               SELECT participant_kind, route
               FROM provider_route_profiles
               WHERE project_id = ?1
           )",
        [project_id],
    )?;
    transaction.execute(
        "UPDATE provider_route_profiles SET project_id = ?1 WHERE project_id = 'agent-room'",
        [project_id],
    )?;

    for table in ["project_settings", "verification_config"] {
        transaction.execute(
            &format!(
                "DELETE FROM {table}
                 WHERE project_id = 'agent-room'
                   AND EXISTS (
                       SELECT 1 FROM {table} WHERE project_id = ?1
                   )"
            ),
            [project_id],
        )?;
        transaction.execute(
            &format!("UPDATE {table} SET project_id = ?1 WHERE project_id = 'agent-room'"),
            [project_id],
        )?;
    }

    transaction.execute("DELETE FROM projects WHERE id = 'agent-room'", [])?;
    transaction.execute(
        "UPDATE app_state
         SET value = ?1
         WHERE key = 'active_project_id' AND value = 'agent-room'",
        [project_id],
    )?;
    transaction.commit()
}

pub(crate) fn reconcile_interrupted_runs(connection: &Connection) -> rusqlite::Result<usize> {
    connection.execute(
        "UPDATE activations
         SET state = 'failed', finished_at = CURRENT_TIMESTAMP
         WHERE state = 'running'",
        [],
    )?;
    let promoting = connection.execute(
        "UPDATE runs
         SET state = 'waiting',
             current_owner = NULL,
             stop_reason = 'Promotion state unknown — inspect the repository before continuing.',
             finished_at = CURRENT_TIMESTAMP
         WHERE state = 'promoting'",
        [],
    )?;
    let interrupted = connection.execute(
        "UPDATE runs
         SET state = 'stopped',
             current_owner = NULL,
             stop_reason = 'Agent Room closed or restarted while this run was active. The managed worktree was preserved for recovery.',
             finished_at = CURRENT_TIMESTAMP
         WHERE state IN ('selecting', 'working', 'verifying', 'reviewing', 'revising')",
        [],
    )?;
    Ok(promoting + interrupted)
}
