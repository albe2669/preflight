//! The append-only event writer.
//!
//! Every core write calls [`EventWriter::append`] in the same transaction as
//! the row it describes. The writer never mutates a todo — it only inserts a
//! `todo_event` row. `logical_date` is computed from the supplied [`Clock`]
//! and `Utc::now()`, so "what did I work on yesterday" is an index scan over
//! `logical_date`, never a timezone calculation in SQL.

use crate::entity::enums::{EventActor, EventKind};
use crate::entity::todo_event;
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ConnectionTrait, Set};
use serde_json::Value;

use crate::clock::Clock;
use crate::error::Result;

pub struct EventWriter;

impl EventWriter {
    /// Append one event row on the caller's transaction.
    ///
    /// `txn` is any `ConnectionTrait` — a `DatabaseConnection` or a
    /// `DatabaseTransaction` — so the event commits atomically with the
    /// mutation that produced it.
    #[allow(clippy::too_many_arguments)]
    pub async fn append<C>(
        txn: &C,
        clock: &Clock,
        todo_id: i64,
        kind: EventKind,
        actor: EventActor,
        field: Option<String>,
        old_value: Option<String>,
        new_value: Option<String>,
        payload: Option<Value>,
    ) -> Result<()>
    where
        C: ConnectionTrait + Sync,
    {
        let now = Utc::now();
        let event = todo_event::ActiveModel {
            todo_id: Set(todo_id),
            kind: Set(kind),
            field: Set(field),
            old_value: Set(old_value),
            new_value: Set(new_value),
            payload: Set(payload),
            actor: Set(actor),
            occurred_at: Set(now.into()),
            logical_date: Set(clock.logical_date(now)),
            ..Default::default()
        };
        event.insert(txn).await?;
        Ok(())
    }
}
