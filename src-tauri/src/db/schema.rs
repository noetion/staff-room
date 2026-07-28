use crate::*;

pub(crate) fn migrate(connection: &Connection) -> rusqlite::Result<()> {
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

        CREATE INDEX IF NOT EXISTS messages_project_created_at_desc
            ON messages(project_id, created_at DESC);

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
            reviewed_fingerprint TEXT,
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

        CREATE TABLE IF NOT EXISTS run_events (
            id TEXT PRIMARY KEY,
            run_id TEXT NOT NULL,
            event_type TEXT NOT NULL,
            phase TEXT NOT NULL,
            state TEXT NOT NULL,
            current_owner TEXT,
            detail TEXT NOT NULL DEFAULT '',
            context_bytes INTEGER,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY(run_id) REFERENCES runs(id)
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
            last_seen_message_rowid INTEGER NOT NULL DEFAULT 0,
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

        CREATE TABLE IF NOT EXISTS verification_config (
            project_id TEXT PRIMARY KEY,
            enabled INTEGER NOT NULL DEFAULT 1,
            commands_json TEXT NOT NULL,
            prepare_command TEXT,
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
        "ALTER TABLE runs ADD COLUMN reviewed_fingerprint TEXT",
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
        "ALTER TABLE chat_sessions ADD COLUMN last_seen_message_rowid INTEGER NOT NULL DEFAULT 0",
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

pub(crate) const PROJECT_SCOPED_TABLES: &[&str] = &[
    "projects",
    "messages",
    "runs",
    "chat_sessions",
    "provider_sessions",
    "provider_profiles",
    "provider_route_profiles",
    "project_settings",
    "verification_config",
    "provider_connections",
    "chat_receipts",
];
