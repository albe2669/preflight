use migration::MigratorTrait;
use std::time::Duration;

use sea_orm::{DatabaseConnection, SqlxSqliteConnector};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};

/// Connect to SQLite with the pragmas the schema relies on: foreign keys ON
/// (per-connection, off by default), WAL journal mode, and a busy timeout.
/// `create_if_missing` lets a fresh `db.sqlite` come into being, but it does
/// not create the parent directory — a first-launch service (home-manager
/// `systemd`/`launchd`) pointing at e.g. `~/.local/state/preflight/db.sqlite`
/// would fail with "unable to open database file" before the file appears, so
/// we create any missing parent directories here.
pub async fn connect(path: &str) -> anyhow::Result<DatabaseConnection> {
    if let Some(parent) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| anyhow::anyhow!("failed to create database directory {parent:?}: {e}"))?;
    }

    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5));

    let pool = sqlx::SqlitePool::connect_with(options)
        .await
        .map_err(|e| anyhow::anyhow!("failed to connect to {path}: {e}"))?;

    Ok(SqlxSqliteConnector::from_sqlx_sqlite_pool(pool))
}

/// Apply all pending migrations.
pub async fn migrate(db: &DatabaseConnection) -> anyhow::Result<()> {
    migration::Migrator::up(db, None)
        .await
        .map_err(|e| anyhow::anyhow!("migration failed: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn connect_creates_missing_parent_directories() {
        // A nested path whose parent does not yet exist, e.g. a fresh
        // `~/.local/state/preflight/db.sqlite` on first launch.
        let dir = std::env::temp_dir().join(format!("preflight-db-noexist-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let db_path = dir.join("nested").join("dir").join("db.sqlite");

        let conn = connect(db_path.to_str().unwrap()).await.unwrap();

        // Parent directories must exist and the DB file must be created so a
        // first-time service launch does not fail with "unable to open
        // database file".
        assert!(db_path.parent().unwrap().is_dir());
        assert!(db_path.is_file());

        conn.close().await.unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
