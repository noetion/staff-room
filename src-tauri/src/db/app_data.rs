use rusqlite::{Connection, DatabaseName, OpenFlags};
use std::path::{Path, PathBuf};
use uuid::Uuid;

const DATABASE_FILE: &str = "staff-room.db";
const LEGACY_APP_DIRECTORY: &str = "com.agentroom.desktop";
const LEGACY_DATABASE_FILE: &str = "agent-room.db";

pub(crate) fn prepare_app_data(
    data_directory: &Path,
    local_data_directory: &Path,
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(data_directory)
        .map_err(|error| format!("Cannot create The Staff Room data directory: {error}"))?;

    let database = data_directory.join(DATABASE_FILE);
    if let Some(parent) = data_directory.parent() {
        let legacy_directory = parent.join(LEGACY_APP_DIRECTORY);
        if legacy_directory != data_directory && legacy_directory.is_dir() {
            migrate_legacy_database(&legacy_directory, &database)?;
        }
    }

    if let Some(parent) = local_data_directory.parent() {
        let legacy_local_data_directory = parent.join(LEGACY_APP_DIRECTORY);
        if legacy_local_data_directory != local_data_directory
            && legacy_local_data_directory.is_dir()
        {
            migrate_legacy_voice_assets(&legacy_local_data_directory, local_data_directory)?;
        }
    }

    Ok(database)
}

fn migrate_legacy_database(legacy_directory: &Path, database: &Path) -> Result<(), String> {
    if database.exists() {
        return Ok(());
    }
    let legacy_database = legacy_directory.join(LEGACY_DATABASE_FILE);
    if !legacy_database.is_file() {
        return Ok(());
    }

    let temporary = database.with_extension(format!("migrating-{}.tmp", Uuid::new_v4()));
    let result = (|| {
        let source =
            Connection::open_with_flags(&legacy_database, OpenFlags::SQLITE_OPEN_READ_ONLY)
                .map_err(|error| {
                    format!("Cannot open the pre-release database for migration: {error}")
                })?;
        source
            .backup(DatabaseName::Main, &temporary, None)
            .map_err(|error| format!("Cannot migrate the pre-release database: {error}"))?;
        drop(source);

        let migrated = Connection::open(&temporary)
            .map_err(|error| format!("Cannot verify the migrated database: {error}"))?;
        let integrity: String = migrated
            .query_row("PRAGMA quick_check", [], |row| row.get(0))
            .map_err(|error| format!("Cannot verify the migrated database: {error}"))?;
        drop(migrated);
        if integrity != "ok" {
            return Err(format!(
                "The migrated database failed its integrity check: {integrity}"
            ));
        }

        finalize_migration(&temporary, database, "database")
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

fn migrate_legacy_voice_assets(
    legacy_directory: &Path,
    data_directory: &Path,
) -> Result<(), String> {
    let legacy_voice = legacy_directory.join("voice");
    if !legacy_voice.is_dir() {
        return Ok(());
    }
    let voice = data_directory.join("voice");
    std::fs::create_dir_all(&voice)
        .map_err(|error| format!("Cannot create The Staff Room voice directory: {error}"))?;
    for name in ["ggml-model.bin", "whisper-cli.exe", "whisper-cli"] {
        copy_legacy_file(&legacy_voice.join(name), &voice.join(name))?;
    }
    Ok(())
}

fn copy_legacy_file(source: &Path, destination: &Path) -> Result<(), String> {
    if destination.exists() || !source.is_file() {
        return Ok(());
    }
    let temporary = destination.with_extension(format!("migrating-{}.tmp", Uuid::new_v4()));
    std::fs::copy(source, &temporary)
        .map_err(|error| format!("Cannot migrate {}: {error}", source.display()))?;
    let result = finalize_migration(&temporary, destination, "voice asset");
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

fn finalize_migration(temporary: &Path, destination: &Path, label: &str) -> Result<(), String> {
    if let Err(error) = std::fs::rename(temporary, destination) {
        if destination.exists() {
            let _ = std::fs::remove_file(temporary);
            return Ok(());
        }
        return Err(format!("Cannot finalize the migrated {label}: {error}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_database_and_voice_assets_without_removing_legacy_data() {
        let root = std::env::temp_dir().join(format!("staff-room-data-test-{}", Uuid::new_v4()));
        let legacy_data = root.join("roaming").join(LEGACY_APP_DIRECTORY);
        let legacy_local_data = root.join("local").join(LEGACY_APP_DIRECTORY);
        let current_data = root.join("roaming").join("com.staffroom.desktop");
        let current_local_data = root.join("local").join("com.staffroom.desktop");
        std::fs::create_dir_all(&legacy_data).unwrap();
        std::fs::create_dir_all(legacy_local_data.join("voice")).unwrap();

        let legacy_database = legacy_data.join(LEGACY_DATABASE_FILE);
        let connection = Connection::open(&legacy_database).unwrap();
        connection
            .execute("CREATE TABLE proof (value TEXT NOT NULL)", [])
            .unwrap();
        connection
            .execute("INSERT INTO proof (value) VALUES ('preserved')", [])
            .unwrap();
        drop(connection);
        std::fs::write(
            legacy_local_data.join("voice").join("ggml-model.bin"),
            b"model",
        )
        .unwrap();

        let migrated_path = prepare_app_data(&current_data, &current_local_data).unwrap();

        assert_eq!(migrated_path, current_data.join(DATABASE_FILE));
        let migrated = Connection::open(migrated_path).unwrap();
        let value: String = migrated
            .query_row("SELECT value FROM proof", [], |row| row.get(0))
            .unwrap();
        assert_eq!(value, "preserved");
        assert_eq!(
            std::fs::read(current_local_data.join("voice").join("ggml-model.bin")).unwrap(),
            b"model"
        );
        assert!(legacy_database.exists());

        drop(migrated);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn preserves_existing_current_data() {
        let root = std::env::temp_dir().join(format!("staff-room-data-test-{}", Uuid::new_v4()));
        let legacy_data = root.join("roaming").join(LEGACY_APP_DIRECTORY);
        let legacy_local_data = root.join("local").join(LEGACY_APP_DIRECTORY);
        let current_data = root.join("roaming").join("com.staffroom.desktop");
        let current_local_data = root.join("local").join("com.staffroom.desktop");
        std::fs::create_dir_all(&legacy_data).unwrap();
        std::fs::create_dir_all(legacy_local_data.join("voice")).unwrap();
        std::fs::create_dir_all(&current_data).unwrap();
        std::fs::create_dir_all(current_local_data.join("voice")).unwrap();
        std::fs::write(legacy_data.join(LEGACY_DATABASE_FILE), b"legacy").unwrap();
        std::fs::write(current_data.join(DATABASE_FILE), b"current").unwrap();
        std::fs::write(
            legacy_local_data.join("voice").join("ggml-model.bin"),
            b"legacy model",
        )
        .unwrap();
        std::fs::write(
            current_local_data.join("voice").join("ggml-model.bin"),
            b"current model",
        )
        .unwrap();

        let path = prepare_app_data(&current_data, &current_local_data).unwrap();

        assert_eq!(std::fs::read(path).unwrap(), b"current");
        assert_eq!(
            std::fs::read(current_local_data.join("voice").join("ggml-model.bin")).unwrap(),
            b"current model"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
