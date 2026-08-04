//! Convert mutations: PR/Linear issue conversion and linking.

use async_graphql;
use entity::sea_orm_active_enums::LinkRelation;

pub struct ConvertMutations;

#[seaography::CustomFields]
#[allow(non_snake_case)]
impl ConvertMutations {
    pub async fn todoFromPullRequest(
        ctx: &async_graphql::Context<'_>,
        pullRequestId: i64,
        planToday: bool,
    ) -> async_graphql::Result<entity::todo::Model> {
        let db = ctx.data::<sea_orm::DatabaseConnection>().unwrap().clone();
        let clock = ctx.data::<preflight_core::Clock>().unwrap().clone();
        let todo = preflight_core::LinkService::new(&db, &clock)
            .todo_from_pr(pullRequestId, planToday)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        Ok(todo)
    }

    pub async fn linkPullRequest(
        ctx: &async_graphql::Context<'_>,
        todoId: i64,
        pullRequestId: i64,
        relation: LinkRelation,
    ) -> async_graphql::Result<entity::todo::Model> {
        let db = ctx.data::<sea_orm::DatabaseConnection>().unwrap().clone();
        let clock = ctx.data::<preflight_core::Clock>().unwrap().clone();
        let todo = preflight_core::LinkService::new(&db, &clock)
            .link_pr(todoId, pullRequestId, relation)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        Ok(todo)
    }

    pub async fn dismissPullRequest(
        ctx: &async_graphql::Context<'_>,
        id: i64,
    ) -> async_graphql::Result<entity::pull_request::Model> {
        let db = ctx.data::<sea_orm::DatabaseConnection>().unwrap().clone();
        let clock = ctx.data::<preflight_core::Clock>().unwrap().clone();
        let pr = preflight_core::LinkService::new(&db, &clock)
            .dismiss_pr(id)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        Ok(pr)
    }

    pub async fn todoFromLinearIssue(
        ctx: &async_graphql::Context<'_>,
        linearIssueId: i64,
        planToday: bool,
    ) -> async_graphql::Result<entity::todo::Model> {
        let db = ctx.data::<sea_orm::DatabaseConnection>().unwrap().clone();
        let clock = ctx.data::<preflight_core::Clock>().unwrap().clone();
        let todo = preflight_core::LinkService::new(&db, &clock)
            .todo_from_linear(linearIssueId, planToday)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        Ok(todo)
    }

    pub async fn linkLinearIssue(
        ctx: &async_graphql::Context<'_>,
        todoId: i64,
        linearIssueId: i64,
    ) -> async_graphql::Result<entity::todo::Model> {
        let db = ctx.data::<sea_orm::DatabaseConnection>().unwrap().clone();
        let clock = ctx.data::<preflight_core::Clock>().unwrap().clone();
        let todo = preflight_core::LinkService::new(&db, &clock)
            .link_linear(todoId, linearIssueId)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        Ok(todo)
    }
}
