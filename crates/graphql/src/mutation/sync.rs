//! Sync mutations: trigger remote syncs and return the updated sync state.

use async_graphql;
use std::sync::Arc;

pub struct SyncMutations;

#[seaography::CustomFields]
#[allow(non_snake_case)]
impl SyncMutations {
    pub async fn syncLinear(
        ctx: &async_graphql::Context<'_>,
    ) -> async_graphql::Result<entity::sync_state::Model> {
        let svc = ctx
            .data::<Arc<dyn sync::linear::LinearSync>>()
            .unwrap()
            .clone();
        let state = svc
            .pull()
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        Ok(state)
    }

    pub async fn syncGithub(
        ctx: &async_graphql::Context<'_>,
    ) -> async_graphql::Result<entity::sync_state::Model> {
        let svc = ctx
            .data::<Arc<dyn sync::github::GithubSync>>()
            .unwrap()
            .clone();
        let state = svc
            .pull()
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        Ok(state)
    }
}
