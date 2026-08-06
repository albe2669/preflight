#![cfg(feature = "integration")]

mod common;
use common::*;

use pretty_assertions::assert_eq;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use links::LinkService;
use todo_domain::todo_service::TodoService;

#[tokio::test]
async fn test_add_tag_creates_tag_and_link() {
    let db = setup_db().await;
    let clock = test_clock();
    let todo_svc = todo_domain::todo_service::new(db.clone(), clock.clone());
    let day_plan_svc = std::sync::Arc::new(todo_domain::day_plan::new(db.clone(), clock.clone()));
    let link_svc = links::new(db.clone(), clock, day_plan_svc);

    let todo = todo_svc.create("Task".to_string(), None).await.unwrap();

    link_svc.add_tag(todo.id, "urgent").await.unwrap();

    let tag = todo_domain::entity::tag::Entity::find()
        .filter(todo_domain::entity::tag::Column::Slug.eq("urgent"))
        .one(&db)
        .await
        .unwrap();
    assert!(tag.is_some());

    let link = todo_domain::entity::todo_tag::Entity::find()
        .filter(todo_domain::entity::todo_tag::Column::TodoId.eq(todo.id))
        .one(&db)
        .await
        .unwrap();
    assert!(link.is_some());
}

#[tokio::test]
async fn test_add_tag_idempotent() {
    let db = setup_db().await;
    let clock = test_clock();
    let todo_svc = todo_domain::todo_service::new(db.clone(), clock.clone());
    let day_plan_svc = std::sync::Arc::new(todo_domain::day_plan::new(db.clone(), clock.clone()));
    let link_svc = links::new(db.clone(), clock, day_plan_svc);

    let todo = todo_svc.create("Task".to_string(), None).await.unwrap();

    link_svc.add_tag(todo.id, "urgent").await.unwrap();
    link_svc.add_tag(todo.id, "urgent").await.unwrap();

    let links = todo_domain::entity::todo_tag::Entity::find()
        .filter(todo_domain::entity::todo_tag::Column::TodoId.eq(todo.id))
        .all(&db)
        .await
        .unwrap();
    assert_eq!(links.len(), 1, "should be only one link row");
}

#[tokio::test]
async fn test_remove_tag_deletes_link() {
    let db = setup_db().await;
    let clock = test_clock();
    let todo_svc = todo_domain::todo_service::new(db.clone(), clock.clone());
    let day_plan_svc = std::sync::Arc::new(todo_domain::day_plan::new(db.clone(), clock.clone()));
    let link_svc = links::new(db.clone(), clock, day_plan_svc);

    let todo = todo_svc.create("Task".to_string(), None).await.unwrap();

    link_svc.add_tag(todo.id, "urgent").await.unwrap();
    link_svc.remove_tag(todo.id, "urgent").await.unwrap();

    let link = todo_domain::entity::todo_tag::Entity::find()
        .filter(todo_domain::entity::todo_tag::Column::TodoId.eq(todo.id))
        .one(&db)
        .await
        .unwrap();
    assert!(link.is_none(), "link should be deleted");
}

#[tokio::test]
async fn test_todo_from_pr_creates_review_todo_with_tag() {
    let db = setup_db().await;
    let clock = test_clock();
    let day_plan_svc = std::sync::Arc::new(todo_domain::day_plan::new(db.clone(), clock.clone()));
    let link_svc = links::new(db.clone(), clock, day_plan_svc);

    let pr_id = seed_pr(&db, "github", "owner", "repo", 42).await;

    let todo = link_svc.todo_from_pr(pr_id, false).await.unwrap();

    // Title should be "Review repo#42"
    assert_eq!(todo.title, "Review repo#42");

    // Should have the "review" tag attached
    let links = todo_domain::entity::todo_tag::Entity::find()
        .filter(todo_domain::entity::todo_tag::Column::TodoId.eq(todo.id))
        .all(&db)
        .await
        .unwrap();
    assert!(!links.is_empty(), "should have the review tag attached");
}

// ─── link_pr ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_link_pr_creates_link_row() {
    let db = setup_db().await;
    let clock = test_clock();
    let todo_svc = todo_domain::todo_service::new(db.clone(), clock.clone());
    let day_plan_svc = std::sync::Arc::new(todo_domain::day_plan::new(db.clone(), clock.clone()));
    let link_svc = links::new(db.clone(), clock, day_plan_svc);

    let todo = todo_svc.create("Task".to_string(), None).await.unwrap();
    let pr_id = seed_pr(&db, "github", "owner", "repo", 99).await;

    link_svc
        .link_pr(todo.id, pr_id, links::entity::enums::LinkRelation::Reviews)
        .await
        .unwrap();

    let link = links::entity::todo_pull_request::Entity::find()
        .filter(links::entity::todo_pull_request::Column::TodoId.eq(todo.id))
        .filter(links::entity::todo_pull_request::Column::PullRequestId.eq(pr_id))
        .one(&db)
        .await
        .unwrap();
    assert!(link.is_some(), "link row should exist");
    assert_eq!(
        link.unwrap().relation,
        links::entity::enums::LinkRelation::Reviews,
        "relation should be Reviews"
    );
}

#[tokio::test]
async fn test_link_pr_idempotent_updates_relation() {
    let db = setup_db().await;
    let clock = test_clock();
    let todo_svc = todo_domain::todo_service::new(db.clone(), clock.clone());
    let day_plan_svc = std::sync::Arc::new(todo_domain::day_plan::new(db.clone(), clock.clone()));
    let link_svc = links::new(db.clone(), clock, day_plan_svc);

    let todo = todo_svc.create("Task".to_string(), None).await.unwrap();
    let pr_id = seed_pr(&db, "github", "owner", "repo", 99).await;

    link_svc
        .link_pr(todo.id, pr_id, links::entity::enums::LinkRelation::Reviews)
        .await
        .unwrap();
    link_svc
        .link_pr(
            todo.id,
            pr_id,
            links::entity::enums::LinkRelation::Implements,
        )
        .await
        .unwrap();

    let links = links::entity::todo_pull_request::Entity::find()
        .filter(links::entity::todo_pull_request::Column::TodoId.eq(todo.id))
        .all(&db)
        .await
        .unwrap();
    assert_eq!(links.len(), 1, "should be only one link row");
    assert_eq!(
        links[0].relation,
        links::entity::enums::LinkRelation::Implements,
        "relation should be updated to Implements"
    );
}

#[tokio::test]
async fn test_link_pr_unknown_todo_returns_not_found() {
    let db = setup_db().await;
    let clock = test_clock();
    let day_plan_svc = std::sync::Arc::new(todo_domain::day_plan::new(db.clone(), clock.clone()));
    let link_svc = links::new(db.clone(), clock, day_plan_svc);

    let pr_id = seed_pr(&db, "github", "owner", "repo", 99).await;

    let result = link_svc
        .link_pr(999_999, pr_id, links::entity::enums::LinkRelation::Reviews)
        .await;
    assert!(matches!(result, Err(links::error::LinkError::NotFound(_))));
}

#[tokio::test]
async fn test_link_pr_unknown_pr_returns_not_found() {
    let db = setup_db().await;
    let clock = test_clock();
    let todo_svc = todo_domain::todo_service::new(db.clone(), clock.clone());
    let day_plan_svc = std::sync::Arc::new(todo_domain::day_plan::new(db.clone(), clock.clone()));
    let link_svc = links::new(db.clone(), clock, day_plan_svc);

    let todo = todo_svc.create("Task".to_string(), None).await.unwrap();

    let result = link_svc
        .link_pr(
            todo.id,
            999_999,
            links::entity::enums::LinkRelation::Reviews,
        )
        .await;
    assert!(matches!(result, Err(links::error::LinkError::NotFound(_))));
}

// ─── dismiss_pr ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_dismiss_pr_sets_dismissed_at() {
    let db = setup_db().await;
    let clock = test_clock();
    let day_plan_svc = std::sync::Arc::new(todo_domain::day_plan::new(db.clone(), clock.clone()));
    let link_svc = links::new(db.clone(), clock, day_plan_svc);

    let pr_id = seed_pr(&db, "github", "owner", "repo", 42).await;

    let pr = link_svc.dismiss_pr(pr_id).await.unwrap();
    assert!(pr.dismissed_at.is_some(), "dismissed_at should be set");
}

#[tokio::test]
async fn test_dismiss_pr_unknown_returns_not_found() {
    let db = setup_db().await;
    let clock = test_clock();
    let day_plan_svc = std::sync::Arc::new(todo_domain::day_plan::new(db.clone(), clock.clone()));
    let link_svc = links::new(db.clone(), clock, day_plan_svc);

    let result = link_svc.dismiss_pr(999_999).await;
    assert!(matches!(result, Err(links::error::LinkError::NotFound(_))));
}

// ─── todo_from_linear ──────────────────────────────────────────────────────────

#[tokio::test]
async fn test_todo_from_linear_creates_todo_with_title() {
    let db = setup_db().await;
    let clock = test_clock();
    let day_plan_svc = std::sync::Arc::new(todo_domain::day_plan::new(db.clone(), clock.clone()));
    let link_svc = links::new(db.clone(), clock, day_plan_svc);

    let issue_id = seed_linear_issue(&db, "linear-1", "ENG-1", "Fix bug").await;

    let todo = link_svc.todo_from_linear(issue_id, false).await.unwrap();
    assert_eq!(todo.title, "Fix bug");
}

#[tokio::test]
async fn test_todo_from_linear_links_issue() {
    let db = setup_db().await;
    let clock = test_clock();
    let day_plan_svc = std::sync::Arc::new(todo_domain::day_plan::new(db.clone(), clock.clone()));
    let link_svc = links::new(db.clone(), clock, day_plan_svc);

    let issue_id = seed_linear_issue(&db, "linear-1", "ENG-1", "Fix bug").await;

    let todo = link_svc.todo_from_linear(issue_id, false).await.unwrap();

    let link = links::entity::todo_linear_issue::Entity::find()
        .filter(links::entity::todo_linear_issue::Column::TodoId.eq(todo.id))
        .filter(links::entity::todo_linear_issue::Column::LinearIssueId.eq(issue_id))
        .one(&db)
        .await
        .unwrap();
    assert!(link.is_some(), "todo_linear_issue link row should exist");
}

#[tokio::test]
async fn test_todo_from_linear_plan_today() {
    let db = setup_db().await;
    let clock = test_clock();
    let day_plan_svc = std::sync::Arc::new(todo_domain::day_plan::new(db.clone(), clock.clone()));
    let link_svc = links::new(db.clone(), clock.clone(), day_plan_svc.clone());

    let issue_id = seed_linear_issue(&db, "linear-1", "ENG-1", "Fix bug").await;

    let todo = link_svc.todo_from_linear(issue_id, true).await.unwrap();

    let today = clock.now_logical();
    let plan = todo_domain::entity::todo_day_plan::Entity::find()
        .filter(todo_domain::entity::todo_day_plan::Column::PlanDate.eq(today))
        .filter(todo_domain::entity::todo_day_plan::Column::TodoId.eq(todo.id))
        .one(&db)
        .await
        .unwrap();
    assert!(plan.is_some(), "todo_day_plan row should exist for today");
}

#[tokio::test]
async fn test_todo_from_linear_unknown_returns_not_found() {
    let db = setup_db().await;
    let clock = test_clock();
    let day_plan_svc = std::sync::Arc::new(todo_domain::day_plan::new(db.clone(), clock.clone()));
    let link_svc = links::new(db.clone(), clock, day_plan_svc);

    let result = link_svc.todo_from_linear(999_999, false).await;
    assert!(matches!(result, Err(links::error::LinkError::NotFound(_))));
}

// ─── link_linear ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_link_linear_creates_link_row() {
    let db = setup_db().await;
    let clock = test_clock();
    let todo_svc = todo_domain::todo_service::new(db.clone(), clock.clone());
    let day_plan_svc = std::sync::Arc::new(todo_domain::day_plan::new(db.clone(), clock.clone()));
    let link_svc = links::new(db.clone(), clock, day_plan_svc);

    let todo = todo_svc.create("Task".to_string(), None).await.unwrap();
    let issue_id = seed_linear_issue(&db, "linear-1", "ENG-1", "Fix bug").await;

    link_svc.link_linear(todo.id, issue_id).await.unwrap();

    let link = links::entity::todo_linear_issue::Entity::find()
        .filter(links::entity::todo_linear_issue::Column::TodoId.eq(todo.id))
        .filter(links::entity::todo_linear_issue::Column::LinearIssueId.eq(issue_id))
        .one(&db)
        .await
        .unwrap();
    assert!(link.is_some(), "todo_linear_issue link row should exist");
}

#[tokio::test]
async fn test_link_linear_idempotent_reassigns() {
    let db = setup_db().await;
    let clock = test_clock();
    let todo_svc = todo_domain::todo_service::new(db.clone(), clock.clone());
    let day_plan_svc = std::sync::Arc::new(todo_domain::day_plan::new(db.clone(), clock.clone()));
    let link_svc = links::new(db.clone(), clock, day_plan_svc);

    let todo1 = todo_svc.create("Task 1".to_string(), None).await.unwrap();
    let todo2 = todo_svc.create("Task 2".to_string(), None).await.unwrap();
    let issue_id = seed_linear_issue(&db, "linear-1", "ENG-1", "Fix bug").await;

    link_svc.link_linear(todo1.id, issue_id).await.unwrap();
    link_svc.link_linear(todo2.id, issue_id).await.unwrap();

    let link = links::entity::todo_linear_issue::Entity::find()
        .filter(links::entity::todo_linear_issue::Column::LinearIssueId.eq(issue_id))
        .one(&db)
        .await
        .unwrap();
    assert!(link.is_some(), "link row should exist");
    assert_eq!(
        link.unwrap().todo_id,
        todo2.id,
        "link should be reassigned to todo2"
    );

    // Confirm only one row for this issue
    let all_links = links::entity::todo_linear_issue::Entity::find()
        .filter(links::entity::todo_linear_issue::Column::LinearIssueId.eq(issue_id))
        .all(&db)
        .await
        .unwrap();
    assert_eq!(all_links.len(), 1, "should be exactly one link row");
}

#[tokio::test]
async fn test_link_linear_unknown_todo_returns_not_found() {
    let db = setup_db().await;
    let clock = test_clock();
    let day_plan_svc = std::sync::Arc::new(todo_domain::day_plan::new(db.clone(), clock.clone()));
    let link_svc = links::new(db.clone(), clock, day_plan_svc);

    let issue_id = seed_linear_issue(&db, "linear-1", "ENG-1", "Fix bug").await;

    let result = link_svc.link_linear(999_999, issue_id).await;
    assert!(matches!(result, Err(links::error::LinkError::NotFound(_))));
}

#[tokio::test]
async fn test_link_linear_unknown_issue_returns_not_found() {
    let db = setup_db().await;
    let clock = test_clock();
    let todo_svc = todo_domain::todo_service::new(db.clone(), clock.clone());
    let day_plan_svc = std::sync::Arc::new(todo_domain::day_plan::new(db.clone(), clock.clone()));
    let link_svc = links::new(db.clone(), clock, day_plan_svc);

    let todo = todo_svc.create("Task".to_string(), None).await.unwrap();

    let result = link_svc.link_linear(todo.id, 999_999).await;
    assert!(matches!(result, Err(links::error::LinkError::NotFound(_))));
}

// ─── remove_tag edge cases ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_remove_tag_unknown_slug_is_noop() {
    let db = setup_db().await;
    let clock = test_clock();
    let todo_svc = todo_domain::todo_service::new(db.clone(), clock.clone());
    let day_plan_svc = std::sync::Arc::new(todo_domain::day_plan::new(db.clone(), clock.clone()));
    let link_svc = links::new(db.clone(), clock, day_plan_svc);

    let todo = todo_svc.create("Task".to_string(), None).await.unwrap();
    link_svc.add_tag(todo.id, "urgent").await.unwrap();

    // Removing a tag that doesn't exist should be a no-op, not an error
    let result = link_svc.remove_tag(todo.id, "nonexistent").await;
    assert!(result.is_ok(), "removing nonexistent slug should be no-op");

    // The original tag should still be attached
    let links = todo_domain::entity::todo_tag::Entity::find()
        .filter(todo_domain::entity::todo_tag::Column::TodoId.eq(todo.id))
        .all(&db)
        .await
        .unwrap();
    assert_eq!(links.len(), 1, "original tag should still be attached");
}

#[tokio::test]
async fn test_remove_tag_when_not_attached_is_noop() {
    let db = setup_db().await;
    let clock = test_clock();
    let todo_svc = todo_domain::todo_service::new(db.clone(), clock.clone());
    let day_plan_svc = std::sync::Arc::new(todo_domain::day_plan::new(db.clone(), clock.clone()));
    let link_svc = links::new(db.clone(), clock, day_plan_svc);

    let todo = todo_svc.create("Task".to_string(), None).await.unwrap();
    link_svc.add_tag(todo.id, "urgent").await.unwrap();

    // "backlog" tag exists (created by add_tag) but is not attached to this todo
    link_svc.add_tag(todo.id, "backlog").await.unwrap();

    let todo2 = todo_svc.create("Task 2".to_string(), None).await.unwrap();

    // "backlog" tag exists as a tag row but is NOT attached to todo2
    let result = link_svc.remove_tag(todo2.id, "backlog").await;
    assert!(result.is_ok(), "removing unattached tag should be no-op");
}

// ─── add_tag edge cases ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_add_tag_unknown_todo_returns_not_found() {
    let db = setup_db().await;
    let clock = test_clock();
    let day_plan_svc = std::sync::Arc::new(todo_domain::day_plan::new(db.clone(), clock.clone()));
    let link_svc = links::new(db.clone(), clock, day_plan_svc);

    let result = link_svc.add_tag(999_999, "urgent").await;
    assert!(matches!(result, Err(links::error::LinkError::NotFound(_))));
}
