//! Tag mutations: add and remove tags from todos.

use async_graphql;
use std::sync::Arc;

pub struct TagMutations;

#[seaography::CustomFields]
#[allow(non_snake_case)]
impl TagMutations {
    pub async fn addTag(
        ctx: &async_graphql::Context<'_>,
        todoId: i64,
        slug: String,
    ) -> async_graphql::Result<todo_domain::entity::todo::Model> {
        let svc = ctx.data::<Arc<dyn links::LinkService>>().unwrap().clone();
        let todo = svc
            .add_tag(todoId, &slug)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        Ok(todo)
    }

    pub async fn removeTag(
        ctx: &async_graphql::Context<'_>,
        todoId: i64,
        slug: String,
    ) -> async_graphql::Result<todo_domain::entity::todo::Model> {
        let svc = ctx.data::<Arc<dyn links::LinkService>>().unwrap().clone();
        let todo = svc
            .remove_tag(todoId, &slug)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        Ok(todo)
    }
}
