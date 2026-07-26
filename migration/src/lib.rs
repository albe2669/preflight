pub use sea_orm_migration::prelude::*;

mod m20220101_000001_create_table;
mod m20260726_191522_create_todo_kind;
mod m20260726_192845_create_todo_status;
mod m20260726_194625_create_todo_event_type;
mod m20260726_195246_create_todo;

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
        ]
    }
}
