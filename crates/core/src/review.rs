//! Daily review queries.
//!
//! Three questions the requirements ask for collapse into one review object:
//! what did I *plan* for a day, what did I *touch*, what did I *complete*, and
//! what was *carried over*. "Planned" scans `todo_day_plan`; "touched" scans
//! `todo_event` for user events on that logical date; both are index lookups,
//! never timezone math. "Completed" is the subset of touched todos whose
//! `closed_at` falls on the same logical date.

use chrono::NaiveDate;
use entity::sea_orm_active_enums::EventActor;
use entity::todo::{Entity as TodoEntity, Model as Todo};
use entity::todo_day_plan::{Column as DayPlanColumn, Entity as DayPlanEntity};
use entity::todo_event::{Column as EventColumn, Entity as EventEntity};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::{Deserialize, Serialize};

use crate::clock::Clock;
use crate::error::Result;
use async_trait::async_trait;

/// A day's review: planned, touched, completed, and carried-over todos.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyReview {
    pub date: NaiveDate,
    pub planned: Vec<Todo>,
    pub touched: Vec<Todo>,
    pub completed: Vec<Todo>,
    pub carried_over: Vec<Todo>,
}

#[async_trait]
pub trait ReviewService: Send + Sync {
    async fn daily(&self, date: NaiveDate) -> Result<DailyReview>;
}

/// Concrete implementation. Private; only the constructor is pub.
pub(crate) struct ReviewServiceImpl {
    db: DatabaseConnection,
    clock: Clock,
}

impl ReviewServiceImpl {
    pub(crate) fn new(db: DatabaseConnection, clock: Clock) -> Self {
        Self { db, clock }
    }

    /// Fetch todos by id, preserving the input order.
    async fn todos_by_ids(&self, ids: &[i64]) -> Result<Vec<Todo>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let rows = TodoEntity::find()
            .filter(entity::todo::Column::Id.is_in(ids.to_vec()))
            .all(&self.db)
            .await?;
        // Reorder to match `ids`.
        let by_id: std::collections::HashMap<i64, Todo> =
            rows.into_iter().map(|t| (t.id, t)).collect();
        let ordered = ids.iter().filter_map(|id| by_id.get(id).cloned()).collect();
        Ok(ordered)
    }
}

/// Construct a review service. Called from server/main only.
pub fn new(db: DatabaseConnection, clock: Clock) -> impl ReviewService {
    ReviewServiceImpl::new(db, clock)
}

#[async_trait]
impl ReviewService for ReviewServiceImpl {
    async fn daily(&self, date: NaiveDate) -> Result<DailyReview> {
        // Planned that day (live rows only), by position.
        let planned_rows = DayPlanEntity::find()
            .filter(DayPlanColumn::PlanDate.eq(date))
            .filter(DayPlanColumn::RemovedAt.is_null())
            .order_by_asc(DayPlanColumn::Position)
            .all(&self.db)
            .await?;
        let planned_ids: Vec<i64> = planned_rows.iter().map(|p| p.todo_id).collect();
        let carried_ids: Vec<i64> = planned_rows
            .iter()
            .filter(|p| p.carried_over)
            .map(|p| p.todo_id)
            .collect();
        let planned = self.todos_by_ids(&planned_ids).await?;
        let carried_over = self.todos_by_ids(&carried_ids).await?;

        // Touched: distinct todos with a user event on this logical date.
        let event_todo_ids: Vec<i64> = EventEntity::find()
            .filter(EventColumn::LogicalDate.eq(date))
            .filter(EventColumn::Actor.eq(EventActor::User))
            .select_only()
            .column(EventColumn::TodoId)
            .distinct()
            .into_tuple()
            .all(&self.db)
            .await?;
        let touched = self.todos_by_ids(&event_todo_ids).await?;

        // Completed: touched todos whose closed_at logical date == this date.
        // Using `clock` would recompute, but closed_at is a timestamp and the
        // set is already small (subset of touched), so filter in Rust.
        let clock = self.clock.clone();
        let completed: Vec<Todo> = touched
            .iter()
            .filter(|t| {
                t.closed_at
                    .map(|c| clock.logical_date(c.with_timezone(&chrono::Utc)) == date)
                    .unwrap_or(false)
            })
            .cloned()
            .collect();

        Ok(DailyReview {
            date,
            planned,
            touched,
            completed,
            carried_over,
        })
    }
}
