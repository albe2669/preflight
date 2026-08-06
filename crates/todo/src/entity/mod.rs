//! Todo domain entities.
//!
//! All tables owned by the todo domain — todo, todo_event, todo_day_plan,
//! tag, and todo_tag — with their SeaORM entity definitions and enum types.

pub mod tag;
pub mod todo;
pub mod todo_day_plan;
pub mod todo_event;
pub mod todo_tag;

pub mod enums;

seaography::register_entity_modules!([todo, todo_day_plan, todo_event, todo_tag, tag]);
