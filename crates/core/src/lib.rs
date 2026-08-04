#![allow(async_fn_in_trait)]

//! The only crate that writes to the database.
//!
//! Every mutation — whether from a GraphQL resolver or a sync puller — goes
//! through a service here, and every service write appends to `todo_event` in
//! the same transaction. That invariant is what keeps the event log from
//! drifting: there is no code path that changes a todo without recording it.

pub mod clock;
pub mod day_plan;
pub mod error;
pub mod events;
pub mod links;
pub mod review;
pub mod todo_service;

pub use clock::Clock;
pub use day_plan::DayPlanService;
pub use error::{Error, Result};
pub use links::LinkService;
pub use review::{DailyReview, ReviewService};
pub use todo_service::TodoService;
