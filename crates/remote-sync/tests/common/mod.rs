#![cfg(feature = "integration")]

use migration::MigratorTrait;
use sea_orm::{Database, DatabaseConnection};

/// Open an in-memory SQLite database with the full schema applied.
pub async fn setup_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    migration::Migrator::up(&db, None).await.unwrap();
    db
}
