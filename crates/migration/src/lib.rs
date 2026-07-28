pub use sea_orm_migration::prelude::*;

mod idens;

mod m20260101_000001_create_todo;
mod m20260101_000002_create_tag;
mod m20260101_000003_create_todo_event;
mod m20260101_000004_create_todo_day_plan;
mod m20260101_000005_create_pull_request;
mod m20260101_000006_create_linear_issue;
mod m20260101_000007_create_sync_state;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260101_000001_create_todo::Migration),
            Box::new(m20260101_000002_create_tag::Migration),
            Box::new(m20260101_000003_create_todo_event::Migration),
            Box::new(m20260101_000004_create_todo_day_plan::Migration),
            Box::new(m20260101_000005_create_pull_request::Migration),
            Box::new(m20260101_000006_create_linear_issue::Migration),
            Box::new(m20260101_000007_create_sync_state::Migration),
        ]
    }
}
