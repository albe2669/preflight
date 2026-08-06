#![cfg(feature = "integration")]

use migration::MigratorTrait;
use sea_orm::{Database, DatabaseConnection};
use todo_domain::Clock;

pub async fn setup_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    migration::Migrator::up(&db, None).await.unwrap();
    db
}

pub fn test_clock() -> Clock {
    Clock::new(chrono_tz::America::Los_Angeles, 4)
}
