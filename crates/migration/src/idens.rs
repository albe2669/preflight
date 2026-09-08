//! Table and column identifiers shared across migrations.
//!
//! The SeaORM docs suggest re-declaring these inside each migration file so a
//! migration is frozen against later renames. For a greenfield schema where
//! four migrations all need `Todo::Id` for foreign keys, one shared module is
//! considerably less noisy. The rule to follow if you ever rename something:
//! copy the *old* Iden into the migration that used it, then rename here.

use sea_orm_migration::prelude::*;

#[derive(DeriveIden)]
pub enum Todo {
    Table,
    Id,
    Title,
    Description,
    Status,
    BlockedReason,
    SortKey,
    CreatedAt,
    UpdatedAt,
    StartedAt,
    ClosedAt,
}

#[derive(DeriveIden)]
pub enum Tag {
    Table,
    Id,
    Slug,
    Name,
    Color,
    CreatedAt,
}

#[derive(DeriveIden)]
pub enum TodoTag {
    Table,
    TodoId,
    TagId,
    CreatedAt,
}

#[derive(DeriveIden)]
pub enum TodoEvent {
    Table,
    Id,
    TodoId,
    Kind,
    Field,
    OldValue,
    NewValue,
    Payload,
    Actor,
    OccurredAt,
    LogicalDate,
}

#[derive(DeriveIden)]
pub enum TodoDayPlan {
    Table,
    Id,
    PlanDate,
    TodoId,
    Position,
    CarriedOver,
    AddedAt,
    RemovedAt,
}

#[derive(DeriveIden)]
pub enum PullRequest {
    Table,
    Id,
    Provider,
    Owner,
    Repo,
    Number,
    Title,
    Url,
    Author,
    State,
    ReviewRequested,
    AuthoredByMe,
    RemoteCreatedAt,
    RemoteUpdatedAt,
    SyncedAt,
    DismissedAt,
    ChangesRequested,
    CopilotComments,
    MergeConflicts,
    Approved,
    ActionsFailing,
}

#[derive(DeriveIden)]
pub enum TodoPullRequest {
    Table,
    TodoId,
    PullRequestId,
    Relation,
    CreatedAt,
}

#[derive(DeriveIden)]
pub enum LinearIssue {
    Table,
    Id,
    LinearId,
    Identifier,
    Title,
    Description,
    Url,
    StateName,
    StateType,
    Priority,
    TeamKey,
    AssigneeName,
    AssignedToMe,
    RemoteCreatedAt,
    RemoteUpdatedAt,
    SyncedAt,
    DismissedAt,
}

#[derive(DeriveIden)]
pub enum TodoLinearIssue {
    Table,
    TodoId,
    LinearIssueId,
    CreatedAt,
}

#[derive(DeriveIden)]
pub enum SyncState {
    Table,
    Source,
    Cursor,
    LastSyncedAt,
    LastStatus,
    LastError,
}
