use sea_orm_migration::{prelude::*, schema::*};

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260726_195246_create_todo"
    }
}

/*
CREATE TABLE todo (
    id                     INTEGER PRIMARY KEY,
    public_id              TEXT    NOT NULL UNIQUE,      -- ULID; stable for export/backup
    kind                   TEXT    NOT NULL REFERENCES todo_kind(code)   ON UPDATE CASCADE,
    status                 TEXT    NOT NULL DEFAULT 'todo'
                                            REFERENCES todo_status(code) ON UPDATE CASCADE,

    title                  TEXT    NOT NULL CHECK (length(trim(title)) > 0),
    description            TEXT    NOT NULL DEFAULT '',  -- markdown; '' not NULL
    priority               INTEGER NOT NULL DEFAULT 0 CHECK (priority BETWEEN 0 AND 4),
    due_on                 TEXT    CHECK (due_on IS NULL OR due_on IS date(due_on)),

    -- provenance: where this todo came from ('local' | 'linear')
    source                 TEXT    NOT NULL DEFAULT 'local'
                                            CHECK (source IN ('local','linear')),
    -- one-way sync guards: once the user edits a mirrored field locally,
    -- the Linear sync must stop overwriting it.
    title_overridden       INTEGER NOT NULL DEFAULT 0 CHECK (title_overridden IN (0,1)),
    description_overridden INTEGER NOT NULL DEFAULT 0 CHECK (description_overridden IN (0,1)),

    created_at             TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at             TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    -- Derived caches, maintained by trigger. Everything here is also
    -- reconstructable from todo_event; they exist for cheap sorting/filtering.
    status_changed_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    started_at             TEXT,                          -- first entry into 'started'
    closed_at              TEXT,                          -- entry into a terminal status
    archived_at            TEXT                           -- soft delete
) STRICT;
 */

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table("todo")
                    .if_not_exists()
                    .col(pk_auto("id").not_null())
                    .col(string("kind").not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_todo_kind")
                            .from("todo", "kind")
                            .to("todo_kind", "code")
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .col(string("status").not_null().default("todo"))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_todo_status")
                            .from("todo", "status")
                            .to("todo_status", "code")
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .col(string("title").not_null())
                    .check(Expr::cust("length(trim(title)) > 0"))
                    .col(string("description").not_null().default(""))
                    .col(integer("priority").not_null().default(0))
                    .col(date("due_on"))
                    .col(
                        enumeration_null("source", "source", vec!["local", "linear"])
                            .not_null()
                            .default("local"),
                    )
                    .col(boolean("title_overridden").not_null().default(false))
                    .col(boolean("description_overridden").not_null().default(false))
                    .col(
                        timestamp("created_at")
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        timestamp("updated_at")
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        timestamp("status_changed_at")
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(timestamp("started_at"))
                    .col(timestamp("closed_at"))
                    .col(timestamp("archived_at"))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_todo_status")
                    .table("todo")
                    .col("status")
                    .col(("updated_at", IndexOrder::Desc))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_todo_kind")
                    .table("todo")
                    .col("kind")
                    .col("status")
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_todo_open")
                    .table("todo")
                    .col(("updated_at", IndexOrder::Desc))
                    .and_where(Expr::col("archived_at").is_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_todo_due")
                    .table("todo")
                    .col("due_on")
                    .and_where(Expr::col("due_on").is_not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table("todo").to_owned())
            .await
    }
}
