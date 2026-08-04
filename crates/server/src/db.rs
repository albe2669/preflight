use migration::MigratorTrait;
use std::time::Duration;

use sea_orm::{DatabaseConnection, SqlxSqliteConnector};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};

/// Connect to SQLite with the pragmas the schema relies on: foreign keys ON
/// (per-connection, off by default), WAL journal mode, and a busy timeout.
/// `create_if_missing` lets a fresh `db.sqlite` come into being.
pub async fn connect(path: &str) -> anyhow::Result<DatabaseConnection> {
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
