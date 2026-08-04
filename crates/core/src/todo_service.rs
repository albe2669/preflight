//! Todo create/edit/status service.
//!
//! Every method opens a transaction, performs the write, appends a
//! `todo_event` row via [`EventWriter`] on the same transaction, and commits.
//! `started_at`/`closed_at` are denormalized conveniences kept in sync here so
//! sorting is cheap; they are always derivable from the event log.

use entity::sea_orm_active_enums::{EventActor, EventKind, TodoStatus};
use entity::todo::{ActiveModel, Entity, Model};
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, TransactionTrait,
};

use crate::clock::{Clock, now_tz};
use crate::error::{Error, Result};
use crate::events::EventWriter;
use async_trait::async_trait;

#[async_trait]
pub trait TodoService: Send + Sync {
    async fn get(&self, id: i64) -> Result<Model>;
    async fn create(&self, title: String, description: Option<String>) -> Result<Model>;
    async fn update(
        &self,
        id: i64,
        title: Option<String>,
        description: Option<String>,
    ) -> Result<Model>;
    async fn set_status(
        &self,
        id: i64,
        status: TodoStatus,
        blocked_reason: Option<String>,
    ) -> Result<Model>;
    async fn all(&self) -> Result<Vec<Model>>;
    async fn find_by_title(&self, title: &str) -> Result<Option<Model>>;
}

/// Concrete implementation. Private; only the constructor is pub.
pub(crate) struct TodoServiceImpl {
    db: DatabaseConnection,
    clock: Clock,
}

impl TodoServiceImpl {
    pub(crate) fn new(db: DatabaseConnection, clock: Clock) -> Self {
        Self { db, clock }
    }
}

/// Construct a todo service. Called from server/main only.
pub fn new(db: DatabaseConnection, clock: Clock) -> impl TodoService {
    TodoServiceImpl::new(db, clock)
}

#[async_trait]
impl TodoService for TodoServiceImpl {
    async fn get(&self, id: i64) -> Result<Model> {
        Entity::find_by_id(id)
            .one(&self.db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("todo {id}")))
    }

    async fn create(&self, title: String, description: Option<String>) -> Result<Model> {
        let now = now_tz();
        let clock = self.clock.clone();
        let title_for_event = title.clone();
        self.db
            .transaction(|txn| {
                Box::pin(async move {
                    let model = ActiveModel {
                        title: ActiveValue::set(title),
                        description: ActiveValue::set(description),
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
                        model.id,
                        EventKind::Created,
                        EventActor::User,
                        None,
                        None,
                        Some(title_for_event),
                        None,
                    )
                    .await?;
                    Ok(model)
                })
            })
            .await
            .map_err(Into::into)
    }

    /// Update title and/or description. Emits one event per changed field.
    async fn update(
        &self,
        id: i64,
        title: Option<String>,
        description: Option<String>,
    ) -> Result<Model> {
        if title.is_none() && description.is_none() {
            return self.get(id).await;
        }
        let clock = self.clock.clone();
        self.db
            .transaction(|txn| {
                Box::pin(async move {
                    let model: Model = Entity::find_by_id(id)
                        .one(txn)
                        .await?
                        .ok_or_else(|| Error::NotFound(format!("todo {id}")))?;
                    let mut active: ActiveModel = model.clone().into();

                    if let Some(new_title) = title {
                        if new_title != model.title {
                            let old_title = model.title.clone();
                            active.title = ActiveValue::set(new_title.clone());
                            EventWriter::append(
                                txn,
                                &clock,
                                id,
                                EventKind::TitleChanged,
                                EventActor::User,
                                Some("title".into()),
                                Some(old_title),
                                Some(new_title),
                                None,
                            )
                            .await?;
                        }
                    }
                    if let Some(new_desc) = description {
                        if Some(new_desc.clone()) != model.description {
                            let old_desc = model.description.clone();
                            active.description = ActiveValue::set(Some(new_desc.clone()));
                            EventWriter::append(
                                txn,
                                &clock,
                                id,
                                EventKind::DescriptionChanged,
                                EventActor::User,
                                Some("description".into()),
                                old_desc,
                                Some(new_desc),
                                None,
                            )
                            .await?;
                        }
                    }
                    active.updated_at = ActiveValue::set(now_tz());
                    let saved = active.update(txn).await?;
                    Ok(saved)
                })
            })
            .await
            .map_err(Into::into)
    }

    /// Set status. Emits `StatusChanged` on any transition, plus `Blocked`/
    /// `Unblocked` when entering/leaving `Blocked`. Maintains `started_at`
    /// (on first `Started`) and `closed_at` (terminal states).
    async fn set_status(
        &self,
        id: i64,
        status: TodoStatus,
        blocked_reason: Option<String>,
    ) -> Result<Model> {
        let clock = self.clock.clone();
        self.db
            .transaction(|txn| {
                Box::pin(async move {
                    let model: Model = Entity::find_by_id(id)
                        .one(txn)
                        .await?
                        .ok_or_else(|| Error::NotFound(format!("todo {id}")))?;
                    let old_status = model.status.clone();
                    let was_blocked = matches!(old_status, TodoStatus::Blocked);
                    let now = now_tz();

                    let mut active: ActiveModel = model.into();
                    active.status = ActiveValue::set(status.clone());
                    active.updated_at = ActiveValue::set(now);

                    if status == TodoStatus::Started {
                        active.started_at = ActiveValue::set(Some(now));
                    }
                    if matches!(status, TodoStatus::Done | TodoStatus::Cancelled) {
                        active.closed_at = ActiveValue::set(Some(now));
                    } else {
                        active.closed_at = ActiveValue::set(None);
                    }
                    if status == TodoStatus::Blocked {
                        active.blocked_reason = ActiveValue::set(blocked_reason.clone());
                    } else {
                        active.blocked_reason = ActiveValue::set(None);
                    }

                    let saved = active.update(txn).await?;

                    if old_status != status {
                        EventWriter::append(
                            txn,
                            &clock,
                            id,
                            EventKind::StatusChanged,
                            EventActor::User,
                            Some("status".into()),
                            Some(format!("{old_status:?}").to_lowercase()),
                            Some(format!("{status:?}").to_lowercase()),
                            None,
                        )
                        .await?;
                    }
                    if status == TodoStatus::Blocked && !was_blocked {
                        EventWriter::append(
                            txn,
                            &clock,
                            id,
                            EventKind::Blocked,
                            EventActor::User,
                            Some("blocked_reason".into()),
                            None,
                            blocked_reason,
                            None,
                        )
                        .await?;
                    }
                    if was_blocked && status != TodoStatus::Blocked {
                        EventWriter::append(
                            txn,
                            &clock,
                            id,
                            EventKind::Unblocked,
                            EventActor::User,
                            None,
                            None,
                            None,
                            None,
                        )
                        .await?;
                    }
                    Ok(saved)
                })
            })
            .await
            .map_err(Into::into)
    }

    /// All todos, newest first. Convenience for internal callers; GraphQL
    /// reads come through the generated Seaography query root.
    async fn all(&self) -> Result<Vec<Model>> {
        Ok(Entity::find()
            .order_by_desc(entity::todo::Column::CreatedAt)
            .all(&self.db)
            .await?)
    }

    /// Find a todo by exact title match.
    async fn find_by_title(&self, title: &str) -> Result<Option<Model>> {
        Ok(Entity::find()
            .filter(entity::todo::Column::Title.eq(title))
            .one(&self.db)
            .await?)
    }
}
