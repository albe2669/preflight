//! Sync mutations: trigger remote syncs and return the updated sync state.

use async_graphql;
use sea_orm::EntityTrait;

pub struct SyncMutations;

#[seaography::CustomFields]
#[allow(non_snake_case)]
impl SyncMutations {
    pub async fn syncLinear(
        ctx: &async_graphql::Context<'_>,
    ) -> async_graphql::Result<entity::sync_state::Model> {
        let db = ctx.data::<sea_orm::DatabaseConnection>().unwrap().clone();

        // Read sync config from environment.
        let token = std::env::var("LINEAR_TOKEN").unwrap_or_default();
        let team_keys: Vec<String> = std::env::var("LINEAR_TEAM_KEYS")
            .unwrap_or_default()
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();

        sync::linear::pull_linear(&db, &token, &team_keys)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        // Read back the sync_state row for "linear".
        let state = entity::sync_state::Entity::find_by_id("linear")
            .one(&db)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?
            .ok_or_else(|| async_graphql::Error::new("sync state not found for linear"))?;
        Ok(state)
    }

    pub async fn syncGithub(
        ctx: &async_graphql::Context<'_>,
    ) -> async_graphql::Result<entity::sync_state::Model> {
        let db = ctx.data::<sea_orm::DatabaseConnection>().unwrap().clone();

        // Read sync config from environment.
        let token = std::env::var("GITHUB_TOKEN").unwrap_or_default();
        let query = std::env::var("GITHUB_SEARCH_QUERY").unwrap_or_default();

        sync::github::pull_github(&db, &token, &query)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        // Read back the sync_state row for "github".
        let state = entity::sync_state::Entity::find_by_id("github")
            .one(&db)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?
            .ok_or_else(|| async_graphql::Error::new("sync state not found for github"))?;
        Ok(state)
    }
}
