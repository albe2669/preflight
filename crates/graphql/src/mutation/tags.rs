//! Tag mutations: add and remove tags from todos.

use async_graphql;

pub struct TagMutations;

#[seaography::CustomFields]
#[allow(non_snake_case)]
impl TagMutations {
    pub async fn addTag(
        ctx: &async_graphql::Context<'_>,
        todoId: i64,
        slug: String,
    ) -> async_graphql::Result<entity::todo::Model> {
        let db = ctx.data::<sea_orm::DatabaseConnection>().unwrap().clone();
        let clock = ctx.data::<preflight_core::Clock>().unwrap().clone();
        let todo = preflight_core::LinkService::new(&db, &clock)
            .add_tag(todoId, &slug)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        Ok(todo)
    }

    pub async fn removeTag(
        ctx: &async_graphql::Context<'_>,
        todoId: i64,
        slug: String,
    ) -> async_graphql::Result<entity::todo::Model> {
        let db = ctx.data::<sea_orm::DatabaseConnection>().unwrap().clone();
        let clock = ctx.data::<preflight_core::Clock>().unwrap().clone();
        let todo = preflight_core::LinkService::new(&db, &clock)
            .remove_tag(todoId, &slug)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        Ok(todo)
    }
}
