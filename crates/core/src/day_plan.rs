//! Day-plan service: plan, unplan, reorder, carry-over.
//!
//! "Doing today" is a row in `todo_day_plan`, not a flag on a todo. A new
//! logical date simply has no rows yet — that is the morning reset, with no
//! cron job. `removed_at` is a soft unplan so "what did I *intend* to do
//! yesterday" stays honest even if an item was pulled off the list at 10am.

use chrono::NaiveDate;
use entity::sea_orm_active_enums::{EventActor, EventKind, TodoStatus};
use entity::todo;
use entity::todo_day_plan::{ActiveModel, Column, Entity, Model};
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait,
    Order, QueryFilter, QueryOrder, TransactionTrait,
};

use crate::clock::{Clock, now_tz};
use crate::error::{Error, Result};
use crate::events::EventWriter;
use async_trait::async_trait;

#[async_trait]
pub trait DayPlanService: Send + Sync {
    async fn plan(&self, todo_id: i64, date: NaiveDate) -> Result<Model>;
    async fn plan_today(&self, todo_id: i64) -> Result<Model>;
    async fn unplan_today(&self, todo_id: i64) -> Result<()>;
    async fn reorder(&self, date: NaiveDate, todo_ids: &[i64]) -> Result<Vec<Model>>;
    async fn carry_over(&self, from: NaiveDate, to: NaiveDate) -> Result<Vec<Model>>;
    async fn list_for_date(&self, date: NaiveDate) -> Result<Vec<Model>>;
}

/// Concrete implementation. Private; only the constructor is pub.
pub(crate) struct DayPlanServiceImpl {
    db: DatabaseConnection,
    clock: Clock,
}

impl DayPlanServiceImpl {
    pub(crate) fn new(db: DatabaseConnection, clock: Clock) -> Self {
        Self { db, clock }
    }

    /// The next position at the tail of a date's plan (max+1, or 0 if empty).
    async fn tail_position<C: ConnectionTrait + Sync>(txn: &C, date: NaiveDate) -> Result<i64> {
        let max = Entity::find()
            .filter(Column::PlanDate.eq(date))
            .order_by_desc(Column::Position)
            .one(txn)
            .await?
            .map(|m| m.position);
        Ok(max.map(|p| p + 1).unwrap_or(0))
    }

    #[allow(dead_code)]
    fn _ensure_order_used(&self) {
        let _ = (Order::Asc, Order::Desc);
    }
}

/// Construct a day plan service. Called from server/main only.
pub fn new(db: DatabaseConnection, clock: Clock) -> impl DayPlanService {
    DayPlanServiceImpl::new(db, clock)
}

#[async_trait]
impl DayPlanService for DayPlanServiceImpl {
    /// Plan a todo for a date. Idempotent on `(plan_date, todo_id)`: if a row
    /// exists (even soft-removed), it is reused — `removed_at` is cleared and
    /// `position` set to the tail. Emits a `Planned` event.
    async fn plan(&self, todo_id: i64, date: NaiveDate) -> Result<Model> {
        let clock = self.clock.clone();
        self.db
            .transaction(|txn| {
                Box::pin(async move {
                    let _todo: todo::Model = todo::Entity::find_by_id(todo_id)
                        .one(txn)
                        .await?
                        .ok_or_else(|| Error::NotFound(format!("todo {todo_id}")))?;

                    let existing = Entity::find()
                        .filter(Column::PlanDate.eq(date))
                        .filter(Column::TodoId.eq(todo_id))
                        .one(txn)
                        .await?;

                    let tail = Self::tail_position(txn, date).await?;

                    let model = if let Some(row) = existing {
                        let mut a: ActiveModel = row.into();
                        a.removed_at = ActiveValue::set(None);
                        a.carried_over = ActiveValue::set(false);
                        a.position = ActiveValue::set(tail);
                        a.update(txn).await?
                    } else {
                        let a = ActiveModel {
                            plan_date: ActiveValue::set(date),
                            todo_id: ActiveValue::set(todo_id),
                            position: ActiveValue::set(tail),
                            carried_over: ActiveValue::set(false),
                            added_at: ActiveValue::set(now_tz()),
                            removed_at: ActiveValue::set(None),
                            ..Default::default()
                        };
                        a.insert(txn).await?
                    };

                    EventWriter::append(
                        txn,
                        &clock,
                        todo_id,
                        EventKind::Planned,
                        EventActor::User,
                        Some("plan_date".into()),
                        None,
                        Some(date.to_string()),
                        None,
                    )
                    .await?;
                    Ok(model)
                })
            })
            .await
            .map_err(Into::into)
    }

    async fn plan_today(&self, todo_id: i64) -> Result<Model> {
        self.plan(todo_id, self.clock.now_logical()).await
    }

    /// Soft-unplan a todo for today: set `removed_at`. Returns `Ok(())` whether
    /// or not a row existed. Emits an `Unplanned` event if a live row was found.
    async fn unplan_today(&self, todo_id: i64) -> Result<()> {
        let clock = self.clock.clone();
        let today = self.clock.now_logical();
        self.db
            .transaction(|txn| {
                Box::pin(async move {
                    let row = Entity::find()
                        .filter(Column::PlanDate.eq(today))
                        .filter(Column::TodoId.eq(todo_id))
                        .filter(Column::RemovedAt.is_null())
                        .one(txn)
                        .await?;
                    if let Some(row) = row {
                        let mut a: ActiveModel = row.into();
                        a.removed_at = ActiveValue::set(Some(now_tz()));
                        a.update(txn).await?;
                        EventWriter::append(
                            txn,
                            &clock,
                            todo_id,
                            EventKind::Unplanned,
                            EventActor::User,
                            Some("plan_date".into()),
                            Some(today.to_string()),
                            None,
                            None,
                        )
                        .await?;
                    }
                    Ok(())
                })
            })
            .await
            .map_err(Into::into)
    }

    /// Rewrite `position` 0..N for the given todo ids on that date, in order.
    /// Ids not currently planned for that date are skipped.
    async fn reorder(&self, date: NaiveDate, todo_ids: &[i64]) -> Result<Vec<Model>> {
        let todo_ids: Vec<i64> = todo_ids.to_vec();
        self.db
            .transaction(|txn| {
                Box::pin(async move {
                    let mut updated = Vec::with_capacity(todo_ids.len());
                    for (pos, &todo_id) in todo_ids.iter().enumerate() {
                        let row = Entity::find()
                            .filter(Column::PlanDate.eq(date))
                            .filter(Column::TodoId.eq(todo_id))
                            .filter(Column::RemovedAt.is_null())
                            .one(txn)
                            .await?;
                        if let Some(row) = row {
                            let mut a: ActiveModel = row.into();
                            a.position = ActiveValue::set(pos as i64);
                            let m = a.update(txn).await?;
                            updated.push(m);
                        }
                    }
                    Ok(updated)
                })
            })
            .await
            .map_err(Into::into)
    }

    /// Carry unfinished (not done/cancelled) planned todos from one date to
    /// another, marking `carried_over = true`. Idempotent on `(to, todo_id)`.
    /// Emits a `CarriedOver` event per copied todo.
    async fn carry_over(&self, from: NaiveDate, to: NaiveDate) -> Result<Vec<Model>> {
        let clock = self.clock.clone();
        self.db
            .transaction(|txn| {
                Box::pin(async move {
                    let planned = Entity::find()
                        .filter(Column::PlanDate.eq(from))
                        .filter(Column::RemovedAt.is_null())
                        .order_by_asc(Column::Position)
                        .all(txn)
                        .await?;

                    let todo_ids: Vec<i64> = planned.iter().map(|p| p.todo_id).collect();
                    let todos = todo::Entity::find()
                        .filter(todo::Column::Id.is_in(todo_ids))
                        .all(txn)
                        .await?;
                    let unfinished: std::collections::HashMap<i64, todo::Model> = todos
                        .into_iter()
                        .filter(|t| !matches!(t.status, TodoStatus::Done | TodoStatus::Cancelled))
                        .map(|t| (t.id, t))
                        .collect();

                    let mut out = Vec::new();
                    let mut pos = Self::tail_position(txn, to).await?;
                    for p in &planned {
                        if !unfinished.contains_key(&p.todo_id) {
                            continue;
                        }
                        let exists = Entity::find()
                            .filter(Column::PlanDate.eq(to))
                            .filter(Column::TodoId.eq(p.todo_id))
                            .one(txn)
                            .await?;
                        let model = if let Some(row) = exists {
                            let mut a: ActiveModel = row.into();
                            a.removed_at = ActiveValue::set(None);
                            a.carried_over = ActiveValue::set(true);
                            a.position = ActiveValue::set(pos);
                            a.update(txn).await?
                        } else {
                            let a = ActiveModel {
                                plan_date: ActiveValue::set(to),
                                todo_id: ActiveValue::set(p.todo_id),
                                position: ActiveValue::set(pos),
                                carried_over: ActiveValue::set(true),
                                added_at: ActiveValue::set(now_tz()),
                                removed_at: ActiveValue::set(None),
                                ..Default::default()
                            };
                            a.insert(txn).await?
                        };
                        pos += 1;
                        EventWriter::append(
                            txn,
                            &clock,
                            p.todo_id,
                            EventKind::CarriedOver,
                            EventActor::User,
                            None,
                            Some(from.to_string()),
                            Some(to.to_string()),
                            None,
                        )
                        .await?;
                        out.push(model);
                    }
                    Ok(out)
                })
            })
            .await
            .map_err(Into::into)
    }

    /// Today's live plan, in order (removed_at IS NULL, ORDER BY position).
    async fn list_for_date(&self, date: NaiveDate) -> Result<Vec<Model>> {
        Ok(Entity::find()
            .filter(Column::PlanDate.eq(date))
            .filter(Column::RemovedAt.is_null())
            .order_by_asc(Column::Position)
            .all(&self.db)
            .await?)
    }
}
