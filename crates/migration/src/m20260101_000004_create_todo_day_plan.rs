use sea_orm_migration::prelude::*;

use crate::idens::{Todo, TodoDayPlan};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(TodoDayPlan::Table)
                    .if_not_exists()
                    // Surrogate key purely for Seaography ergonomics (mutations
                    // addressed by a single id). The unique index below is the
                    // real key.
                    .col(
                        ColumnDef::new(TodoDayPlan::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(TodoDayPlan::PlanDate).date().not_null())
                    .col(ColumnDef::new(TodoDayPlan::TodoId).integer().not_null())
                    .col(
                        ColumnDef::new(TodoDayPlan::Position)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(TodoDayPlan::CarriedOver)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(TodoDayPlan::AddedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    // Soft unplan. Pulling something off today's list must not
                    // erase the fact that you had planned it -- otherwise
                    // "what did I intend to do yesterday" quietly lies.
                    .col(
                        ColumnDef::new(TodoDayPlan::RemovedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_todo_day_plan_todo")
                            .from(TodoDayPlan::Table, TodoDayPlan::TodoId)
                            .to(Todo::Table, Todo::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // A todo appears at most once per day. This is what makes
        // "plan for today" idempotent.
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("ux_todo_day_plan_date_todo")
                    .table(TodoDayPlan::Table)
                    .col(TodoDayPlan::PlanDate)
                    .col(TodoDayPlan::TodoId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Today's list, in order. There is no "reset" job: a new logical date
        // simply has no rows yet.
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_todo_day_plan_date_pos")
                    .table(TodoDayPlan::Table)
                    .col(TodoDayPlan::PlanDate)
                    .col(TodoDayPlan::Position)
                    .to_owned(),
            )
            .await?;

        // "Which days did this todo appear on?"
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_todo_day_plan_todo")
                    .table(TodoDayPlan::Table)
                    .col(TodoDayPlan::TodoId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(TodoDayPlan::Table).to_owned())
            .await
    }
}
