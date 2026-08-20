#![cfg(feature = "integration")]

use chrono::Utc;
use github::entity::enums::PullRequestState;
use migration::MigratorTrait;
use sea_orm::{ActiveModelTrait, ActiveValue, Database, DatabaseConnection};
use todo_domain::Clock;

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
    let now = Utc::now().into();
    let pr = github::entity::pull_request::ActiveModel {
        id: ActiveValue::NotSet,
        provider: ActiveValue::Set(provider.to_string()),
        owner: ActiveValue::Set(owner.to_string()),
        repo: ActiveValue::Set(repo.to_string()),
        number: ActiveValue::Set(number),
        title: ActiveValue::Set(format!("PR #{number}")),
        url: ActiveValue::Set(format!("https://github.com/{owner}/{repo}/pull/{number}")),
        author: ActiveValue::Set(Some("test-user".to_string())),
        state: ActiveValue::Set(PullRequestState::Open),
        review_requested: ActiveValue::Set(true),
        authored_by_me: ActiveValue::Set(false),
        changes_requested: ActiveValue::Set(false),
        copilot_comments: ActiveValue::Set(false),
        merge_conflicts: ActiveValue::Set(false),
        remote_created_at: ActiveValue::Set(None),
        remote_updated_at: ActiveValue::Set(None),
        synced_at: ActiveValue::Set(now),
        dismissed_at: ActiveValue::Set(None),
    };
    pr.insert(db).await.unwrap().id
}

/// Insert a linear_issue row directly via ActiveModel, return its id.
pub async fn seed_linear_issue(
    db: &DatabaseConnection,
    linear_id: &str,
    identifier: &str,
    title: &str,
) -> i64 {
    let now = Utc::now().into();
    let issue = linear::entity::linear_issue::ActiveModel {
        id: ActiveValue::NotSet,
        linear_id: ActiveValue::Set(linear_id.to_string()),
        identifier: ActiveValue::Set(identifier.to_string()),
        title: ActiveValue::Set(title.to_string()),
        description: ActiveValue::Set(None),
        url: ActiveValue::Set(format!("https://linear.app/test/issue/{identifier}")),
        state_name: ActiveValue::Set("Todo".to_string()),
        state_type: ActiveValue::Set("active".to_string()),
        priority: ActiveValue::Set(None),
        team_key: ActiveValue::Set(Some("ENG".to_string())),
        assignee_name: ActiveValue::Set(None),
        assigned_to_me: ActiveValue::Set(false),
        remote_created_at: ActiveValue::Set(None),
        remote_updated_at: ActiveValue::Set(None),
        synced_at: ActiveValue::Set(now),
        dismissed_at: ActiveValue::Set(None),
    };
    issue.insert(db).await.unwrap().id
}
