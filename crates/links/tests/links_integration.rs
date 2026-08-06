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
