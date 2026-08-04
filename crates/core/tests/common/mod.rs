#![cfg(feature = "integration")]

use migration::MigratorTrait;
use preflight_core::Clock;
use sea_orm::{ActiveModelTrait, ActiveValue, Database, DatabaseConnection};

pub async fn setup_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    migration::Migrator::up(&db, None).await.unwrap();
    db
}

pub fn test_clock() -> Clock {
    Clock::new(chrono_tz::America::Los_Angeles, 4)
}

/// Insert a pull_request row directly via ActiveModel, return its id.
pub async fn seed_pr(
    db: &DatabaseConnection,
    provider: &str,
    owner: &str,
    repo: &str,
    number: i64,
) -> i64 {
    use chrono::Utc;
    let now = Utc::now().into();
    let pr = entity::pull_request::ActiveModel {
        id: ActiveValue::NotSet,
        provider: ActiveValue::Set(provider.to_string()),
        owner: ActiveValue::Set(owner.to_string()),
        repo: ActiveValue::Set(repo.to_string()),
        number: ActiveValue::Set(number),
        title: ActiveValue::Set(format!("PR #{number}")),
        url: ActiveValue::Set(format!("https://github.com/{owner}/{repo}/pull/{number}")),
        author: ActiveValue::Set(Some("test-user".to_string())),
        state: ActiveValue::Set(entity::sea_orm_active_enums::PullRequestState::Open),
        review_requested: ActiveValue::Set(true),
        authored_by_me: ActiveValue::Set(false),
        remote_created_at: ActiveValue::Set(None),
        remote_updated_at: ActiveValue::Set(None),
        synced_at: ActiveValue::Set(now),
        dismissed_at: ActiveValue::Set(None),
    };
    pr.insert(db).await.unwrap().id
}
