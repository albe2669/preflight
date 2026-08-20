//! Query root and schema builder.
//!
//! Registers all entity modules (queries only, no generated CRUD), enums,
//! custom queries, custom mutations, and the DailyReview output type.

use crate::entities::*;
use async_graphql::dynamic::*;
use sea_orm::DatabaseConnection;
use seaography::{Builder, BuilderContext, async_graphql};
use std::sync::{Arc, LazyLock};

static CONTEXT: LazyLock<BuilderContext> = LazyLock::new(BuilderContext::default);

/// Build a ready-to-finish `SchemaBuilder` with entities, enums, custom
/// queries, and custom mutations wired in.
#[allow(clippy::too_many_arguments)]
pub fn schema_builder(
    database: DatabaseConnection,
    todo: Arc<dyn todo_domain::TodoService>,
    day_plan: Arc<dyn todo_domain::DayPlanService>,
    link: Arc<dyn links::LinkService>,
    review: Arc<dyn todo_domain::ReviewService>,
    github: Arc<dyn github::GithubSync>,
    linear: Arc<dyn linear::LinearSync>,
    clock: todo_domain::Clock,
    depth: Option<usize>,
    complexity: Option<usize>,
) -> SchemaBuilder {
    let mut builder = Builder::new(&CONTEXT, database.clone());

    // Register entities — queries only, no generated CRUD mutations.
    seaography::register_entity!(builder, todo, mutation: false);
    seaography::register_entity!(builder, tag, mutation: false);
    seaography::register_entity!(builder, todo_tag, mutation: false);
    seaography::register_entity!(builder, todo_day_plan, mutation: false);
    seaography::register_entity!(builder, todo_event, mutation: false);
    seaography::register_entity!(builder, pull_request, mutation: false);
    seaography::register_entity!(builder, todo_pull_request, mutation: false);
    seaography::register_entity!(builder, linear_issue, mutation: false);
    seaography::register_entity!(builder, todo_linear_issue, mutation: false);
    seaography::register_entity!(builder, sync_state, mutation: false);

    // Register enums so they appear as GraphQL enums and can be used as args.
    builder.register_enumeration::<todo_domain::entity::enums::TodoStatus>();
    builder.register_enumeration::<todo_domain::entity::enums::EventKind>();
    builder.register_enumeration::<todo_domain::entity::enums::EventActor>();
    builder.register_enumeration::<github::entity::enums::PullRequestState>();
    builder.register_enumeration::<links::entity::enums::LinkRelation>();

    // Register custom queries.
    builder.register_custom_query::<crate::query::Queries>();

    // Register custom mutations.
    builder.register_custom_mutation::<crate::mutation::TodoMutations>();
    builder.register_custom_mutation::<crate::mutation::DayPlanMutations>();
    builder.register_custom_mutation::<crate::mutation::TagMutations>();
    builder.register_custom_mutation::<crate::mutation::ConvertMutations>();
    builder.register_custom_mutation::<crate::mutation::SyncMutations>();

    builder.register_custom_output::<crate::types::Clock>();
    builder.register_custom_output::<crate::types::DailyReview>();

    builder
        .set_depth_limit(depth)
        .set_complexity_limit(complexity)
        .schema_builder()
        .data(database)
        .data(todo)
        .data(day_plan)
        .data(link)
        .data(review)
        .data(github)
        .data(clock.clone())
        .data(linear)
}
