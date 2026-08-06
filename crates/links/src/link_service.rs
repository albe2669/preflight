//! Link + convert service for PRs and Linear issues.
//!
//! PRs and Linear issues are *inbox rows*, not todos. They live in their own
//! tables and are linked or converted by an explicit action. `todo_from_pr`
//! is the "quickly convert" path: it defaults the title to
//! `Review {repo}#{number}`, applies the `review` tag, links the PR with
//! relation `Reviews`, and optionally drops it on today's plan — one call.

use crate::entity::enums::LinkRelation;
use crate::entity::todo_linear_issue::{self, Entity as TliEntity};
use crate::entity::todo_pull_request::{self, ActiveModel as TprActiveModel, Entity as TprEntity};
use github::entity::pull_request::{self, Entity as PullRequestEntity, Model as PullRequest};
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait,
    QueryFilter, Set, TransactionTrait,
};
use std::sync::Arc;
use todo_domain::entity::enums::{EventActor, EventKind, TodoStatus};
use todo_domain::entity::tag::{ActiveModel as TagActiveModel, Entity as TagEntity};
use todo_domain::entity::todo::{
    ActiveModel as TodoActiveModel, Entity as TodoEntity, Model as Todo,
};
use todo_domain::entity::todo_tag::{
    self, ActiveModel as TodoTagActiveModel, Entity as TodoTagEntity,
};

use crate::error::{LinkError, Result};
use todo_domain::clock::{Clock, now_tz};
use todo_domain::day_plan::DayPlanService;
use todo_domain::events::EventWriter;

use async_trait::async_trait;
const REVIEW_SLUG: &str = "review";

#[async_trait]
pub trait LinkService: Send + Sync {
    async fn todo_from_pr(&self, pr_id: i64, plan_today: bool) -> Result<Todo>;
    async fn link_pr(&self, todo_id: i64, pr_id: i64, relation: LinkRelation) -> Result<Todo>;
    async fn dismiss_pr(&self, pr_id: i64) -> Result<PullRequest>;
    async fn todo_from_linear(&self, linear_issue_id: i64, plan_today: bool) -> Result<Todo>;
    async fn link_linear(&self, todo_id: i64, linear_issue_id: i64) -> Result<Todo>;
    async fn add_tag(&self, todo_id: i64, slug: &str) -> Result<Todo>;
    async fn remove_tag(&self, todo_id: i64, slug: &str) -> Result<Todo>;
}

/// Concrete implementation. Private; only the constructor is pub.
pub(crate) struct LinkServiceImpl {
    db: DatabaseConnection,
    clock: Clock,
    day_plan: Arc<dyn DayPlanService>,
}

impl LinkServiceImpl {
    pub(crate) fn new(
        db: DatabaseConnection,
        clock: Clock,
        day_plan: Arc<dyn DayPlanService>,
    ) -> Self {
        Self {
            db,
            clock,
            day_plan,
        }
    }

    async fn ensure_review_tag<C: ConnectionTrait + Sync>(txn: &C) -> Result<i64> {
        let tag = Self::find_or_create_tag(txn, REVIEW_SLUG, Some("Review")).await?;
        Ok(tag.id)
    }

    async fn find_or_create_tag<C: ConnectionTrait + Sync>(
        txn: &C,
        slug: &str,
        name: Option<&str>,
    ) -> Result<todo_domain::entity::tag::Model> {
        if let Some(t) = TagEntity::find()
            .filter(todo_domain::entity::tag::Column::Slug.eq(slug))
            .one(txn)
            .await?
        {
            return Ok(t);
        }
        let a = TagActiveModel {
            slug: ActiveValue::set(slug.to_string()),
            name: ActiveValue::set(name.unwrap_or(slug).to_string()),
            color: ActiveValue::set(None),
            created_at: ActiveValue::set(now_tz()),
            ..Default::default()
        };
        Ok(a.insert(txn).await?)
    }
}

/// Construct a link service. Called from server/main only.
pub fn new(
    db: DatabaseConnection,
    clock: Clock,
    day_plan: Arc<dyn DayPlanService>,
) -> impl LinkService {
    LinkServiceImpl::new(db, clock, day_plan)
}

#[async_trait]
impl LinkService for LinkServiceImpl {
    /// Convert a PR into a review todo. Title = `Review {repo}#{number}`,
    /// applies the `review` tag, links with `Reviews`, optionally plans today.
    async fn todo_from_pr(&self, pr_id: i64, plan_today: bool) -> Result<Todo> {
        let clock = self.clock.clone();
        let todo = self
            .db
            .transaction(|txn| {
                Box::pin(async move {
                    let pr: PullRequest = PullRequestEntity::find_by_id(pr_id)
                        .one(txn)
                        .await?
                        .ok_or_else(|| LinkError::NotFound(format!("pull_request {pr_id}")))?;

                    let now = now_tz();
                    let title = format!("Review {}#{}", pr.repo, pr.number);
                    let todo = TodoActiveModel {
                        title: ActiveValue::set(title.clone()),
                        description: ActiveValue::set(None),
                        status: ActiveValue::set(TodoStatus::Todo),
                        blocked_reason: ActiveValue::set(None),
                        sort_key: ActiveValue::set(0),
                        created_at: ActiveValue::set(now_tz()),
                        updated_at: ActiveValue::set(now),
                        started_at: ActiveValue::set(None),
                        closed_at: ActiveValue::set(None),
                        ..Default::default()
                    }
                    .insert(txn)
                    .await?;

                    EventWriter::append(
                        txn,
                        &clock,
                        todo.id,
                        EventKind::Created,
                        EventActor::User,
                        None,
                        None,
                        Some(title),
                        None,
                    )
                    .await?;

                    // Ensure + attach the `review` tag.
                    let tag_id = Self::ensure_review_tag(txn).await?;
                    let _tt = TodoTagActiveModel {
                        todo_id: ActiveValue::set(todo.id),
                        tag_id: ActiveValue::set(tag_id),
                        created_at: ActiveValue::set(now_tz()),
                    }
                    .insert(txn)
                    .await?;
                    EventWriter::append(
                        txn,
                        &clock,
                        todo.id,
                        EventKind::TagAdded,
                        EventActor::User,
                        Some("tag".into()),
                        None,
                        Some(REVIEW_SLUG.into()),
                        None,
                    )
                    .await?;

                    // Link the PR.
                    let _tpr = TprActiveModel {
                        todo_id: ActiveValue::set(todo.id),
                        pull_request_id: ActiveValue::set(pr_id),
                        relation: ActiveValue::set(LinkRelation::Reviews),
                        created_at: ActiveValue::set(now_tz()),
                    }
                    .insert(txn)
                    .await?;
                    EventWriter::append(
                        txn,
                        &clock,
                        todo.id,
                        EventKind::LinkedPr,
                        EventActor::User,
                        Some("pull_request_id".into()),
                        None,
                        Some(pr_id.to_string()),
                        None,
                    )
                    .await?;
                    Ok(todo)
                })
            })
            .await?;

        // Plan for today outside the create transaction.
        if plan_today {
            self.day_plan.plan_today(todo.id).await?;
        }
        Ok(todo)
    }

    /// Link an existing todo to a PR with a relation. Idempotent: if the link
    /// exists it is updated to the new relation.
    async fn link_pr(&self, todo_id: i64, pr_id: i64, relation: LinkRelation) -> Result<Todo> {
        let clock = self.clock.clone();
        self.db
            .transaction(|txn| {
                Box::pin(async move {
                    let todo: Todo = TodoEntity::find_by_id(todo_id)
                        .one(txn)
                        .await?
                        .ok_or_else(|| LinkError::NotFound(format!("todo {todo_id}")))?;
                    let _pr: PullRequest = PullRequestEntity::find_by_id(pr_id)
                        .one(txn)
                        .await?
                        .ok_or_else(|| LinkError::NotFound(format!("pull_request {pr_id}")))?;

                    let existing = TprEntity::find()
                        .filter(todo_pull_request::Column::TodoId.eq(todo_id))
                        .filter(todo_pull_request::Column::PullRequestId.eq(pr_id))
                        .one(txn)
                        .await?;
                    if let Some(row) = existing {
                        let mut a: TprActiveModel = row.into();
                        a.relation = ActiveValue::set(relation);
                        a.update(txn).await?;
                    } else {
                        let _ = TprActiveModel {
                            todo_id: ActiveValue::set(todo_id),
                            pull_request_id: ActiveValue::set(pr_id),
                            relation: ActiveValue::set(relation),
                            created_at: ActiveValue::set(now_tz()),
                        }
                        .insert(txn)
                        .await?;
                    }
                    EventWriter::append(
                        txn,
                        &clock,
                        todo_id,
                        EventKind::LinkedPr,
                        EventActor::User,
                        Some("pull_request_id".into()),
                        None,
                        Some(pr_id.to_string()),
                        None,
                    )
                    .await?;
                    Ok(todo)
                })
            })
            .await
            .map_err(Into::into)
    }

    /// Dismiss a PR from the inbox without creating a todo.
    async fn dismiss_pr(&self, pr_id: i64) -> Result<PullRequest> {
        let pr: PullRequest = PullRequestEntity::find_by_id(pr_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| LinkError::NotFound(format!("pull_request {pr_id}")))?;
        let mut a: pull_request::ActiveModel = pr.into();
        a.dismissed_at = Set(Some(now_tz()));
        Ok(a.update(&self.db).await?)
    }

    /// Convert a Linear issue into a todo. Title = the issue's title. Does NOT
    /// plan by default (issues are implementation work, not review work).
    async fn todo_from_linear(&self, linear_issue_id: i64, plan_today: bool) -> Result<Todo> {
        let clock = self.clock.clone();
        let todo = self
            .db
            .transaction(|txn| {
                Box::pin(async move {
                    let issue: linear::entity::linear_issue::Model =
                        linear::entity::linear_issue::Entity::find_by_id(linear_issue_id)
                            .one(txn)
                            .await?
                            .ok_or_else(|| {
                                LinkError::NotFound(format!("linear_issue {linear_issue_id}"))
                            })?;

                    let now = now_tz();
                    let title = issue.title.clone();
                    let todo = TodoActiveModel {
                        title: ActiveValue::set(title.clone()),
                        description: ActiveValue::set(issue.description.clone()),
                        status: ActiveValue::set(TodoStatus::Todo),
                        blocked_reason: ActiveValue::set(None),
                        sort_key: ActiveValue::set(0),
                        created_at: ActiveValue::set(now_tz()),
                        updated_at: ActiveValue::set(now),
                        started_at: ActiveValue::set(None),
                        closed_at: ActiveValue::set(None),
                        ..Default::default()
                    }
                    .insert(txn)
                    .await?;

                    EventWriter::append(
                        txn,
                        &clock,
                        todo.id,
                        EventKind::Created,
                        EventActor::User,
                        None,
                        None,
                        Some(title),
                        None,
                    )
                    .await?;

                    let _tli = todo_linear_issue::ActiveModel {
                        todo_id: ActiveValue::set(todo.id),
                        linear_issue_id: ActiveValue::set(linear_issue_id),
                        created_at: ActiveValue::set(now_tz()),
                    }
                    .insert(txn)
                    .await?;
                    EventWriter::append(
                        txn,
                        &clock,
                        todo.id,
                        EventKind::LinkedLinear,
                        EventActor::User,
                        Some("linear_issue_id".into()),
                        None,
                        Some(linear_issue_id.to_string()),
                        None,
                    )
                    .await?;
                    Ok(todo)
                })
            })
            .await?;

        if plan_today {
            self.day_plan.plan_today(todo.id).await?;
        }
        Ok(todo)
    }

    /// Link an existing todo to a Linear issue. Idempotent on the issue (an
    /// issue converts to at most one todo — `ux_todo_linear_issue_issue`).
    async fn link_linear(&self, todo_id: i64, linear_issue_id: i64) -> Result<Todo> {
        let clock = self.clock.clone();
        self.db
            .transaction(|txn| {
                Box::pin(async move {
                    let todo: Todo = TodoEntity::find_by_id(todo_id)
                        .one(txn)
                        .await?
                        .ok_or_else(|| LinkError::NotFound(format!("todo {todo_id}")))?;
                    let _issue: linear::entity::linear_issue::Model =
                        linear::entity::linear_issue::Entity::find_by_id(linear_issue_id)
                            .one(txn)
                            .await?
                            .ok_or_else(|| {
                                LinkError::NotFound(format!("linear_issue {linear_issue_id}"))
                            })?;

                    let existing = TliEntity::find()
                        .filter(todo_linear_issue::Column::LinearIssueId.eq(linear_issue_id))
                        .one(txn)
                        .await?;
                    if let Some(row) = existing {
                        let mut a: todo_linear_issue::ActiveModel = row.into();
                        a.todo_id = ActiveValue::set(todo_id);
                        a.update(txn).await?;
                    } else {
                        let _ = todo_linear_issue::ActiveModel {
                            todo_id: ActiveValue::set(todo_id),
                            linear_issue_id: ActiveValue::set(linear_issue_id),
                            created_at: ActiveValue::set(now_tz()),
                        }
                        .insert(txn)
                        .await?;
                    }
                    EventWriter::append(
                        txn,
                        &clock,
                        todo_id,
                        EventKind::LinkedLinear,
                        EventActor::User,
                        Some("linear_issue_id".into()),
                        None,
                        Some(linear_issue_id.to_string()),
                        None,
                    )
                    .await?;
                    Ok(todo)
                })
            })
            .await
            .map_err(Into::into)
    }

    /// Add a tag by slug to a todo, creating the tag if it does not exist.
    async fn add_tag(&self, todo_id: i64, slug: &str) -> Result<Todo> {
        let clock = self.clock.clone();
        let slug = slug.to_string();
        self.db
            .transaction(|txn| {
                Box::pin(async move {
                    let todo: Todo = TodoEntity::find_by_id(todo_id)
                        .one(txn)
                        .await?
                        .ok_or_else(|| LinkError::NotFound(format!("todo {todo_id}")))?;

                    let tag = Self::find_or_create_tag(txn, &slug, None).await?;
                    let existing = TodoTagEntity::find()
                        .filter(todo_tag::Column::TodoId.eq(todo_id))
                        .filter(todo_tag::Column::TagId.eq(tag.id))
                        .one(txn)
                        .await?;
                    if existing.is_none() {
                        let _ = TodoTagActiveModel {
                            todo_id: ActiveValue::set(todo_id),
                            tag_id: ActiveValue::set(tag.id),
                            created_at: ActiveValue::set(now_tz()),
                        }
                        .insert(txn)
                        .await?;
                        EventWriter::append(
                            txn,
                            &clock,
                            todo_id,
                            EventKind::TagAdded,
                            EventActor::User,
                            Some("tag".into()),
                            None,
                            Some(slug),
                            None,
                        )
                        .await?;
                    }
                    Ok(todo)
                })
            })
            .await
            .map_err(Into::into)
    }

    /// Remove a tag by slug from a todo.
    async fn remove_tag(&self, todo_id: i64, slug: &str) -> Result<Todo> {
        let clock = self.clock.clone();
        let slug = slug.to_string();
        self.db
            .transaction(|txn| {
                Box::pin(async move {
                    let todo: Todo = TodoEntity::find_by_id(todo_id)
                        .one(txn)
                        .await?
                        .ok_or_else(|| LinkError::NotFound(format!("todo {todo_id}")))?;
                    let tag = TagEntity::find()
                        .filter(todo_domain::entity::tag::Column::Slug.eq(slug.clone()))
                        .one(txn)
                        .await?;
                    if let Some(tag) = tag {
                        let link = TodoTagEntity::find()
                            .filter(todo_tag::Column::TodoId.eq(todo_id))
                            .filter(todo_tag::Column::TagId.eq(tag.id))
                            .one(txn)
                            .await?;
                        if link.is_some() {
                            let _ = TodoTagEntity::delete_by_id((todo_id, tag.id))
                                .exec(txn)
                                .await?;
                            EventWriter::append(
                                txn,
                                &clock,
                                todo_id,
                                EventKind::TagRemoved,
                                EventActor::User,
                                Some("tag".into()),
                                Some(slug),
                                None,
                                None,
                            )
                            .await?;
                        }
                    }
                    Ok(todo)
                })
            })
            .await
            .map_err(Into::into)
    }
}
