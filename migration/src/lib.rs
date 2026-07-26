pub use sea_orm_migration::prelude::*;

mod m20220101_000001_create_table;
mod m20260726_191522_create_todo_kind;
mod m20260726_192845_create_todo_status;
mod m20260726_194625_create_todo_event_type;
mod m20260726_195246_create_todo;
mod m20260726_195300_create_app_setting;
mod m20260726_195400_create_todo_status_transition;
mod m20260726_195500_create_tag;
mod m20260726_195600_create_todo_tag;
mod m20260726_195700_create_todo_day_plan;
mod m20260726_195800_create_pull_request;
mod m20260726_195900_create_todo_pull_request;
mod m20260726_200000_create_linear_issue;
mod m20260726_200100_create_linear_state_map;
mod m20260726_200200_create_sync_state;
mod m20260726_200300_create_todo_event;
mod m20260726_200400_create_todo_triggers;
mod m20260726_200500_create_todo_fts;
mod m20260726_200600_create_views;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_create_table::Migration),
            Box::new(m20260726_191522_create_todo_kind::Migration),
            Box::new(m20260726_192845_create_todo_status::Migration),
            Box::new(m20260726_194625_create_todo_event_type::Migration),
            Box::new(m20260726_195246_create_todo::Migration),
            Box::new(m20260726_195300_create_app_setting::Migration),
            Box::new(m20260726_195400_create_todo_status_transition::Migration),
            Box::new(m20260726_195500_create_tag::Migration),
            Box::new(m20260726_195600_create_todo_tag::Migration),
            Box::new(m20260726_195700_create_todo_day_plan::Migration),
            Box::new(m20260726_195800_create_pull_request::Migration),
            Box::new(m20260726_195900_create_todo_pull_request::Migration),
            Box::new(m20260726_200000_create_linear_issue::Migration),
            Box::new(m20260726_200100_create_linear_state_map::Migration),
            Box::new(m20260726_200200_create_sync_state::Migration),
            Box::new(m20260726_200300_create_todo_event::Migration),
            Box::new(m20260726_200400_create_todo_triggers::Migration),
            Box::new(m20260726_200500_create_todo_fts::Migration),
            Box::new(m20260726_200600_create_views::Migration),
        ]
    }
}
