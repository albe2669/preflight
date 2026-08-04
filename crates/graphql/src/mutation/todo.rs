//! Todo mutations: create, update, set status.

use async_graphql;
use entity::sea_orm_active_enums::TodoStatus;

pub struct TodoMutations;

#[seaography::CustomFields]
#[allow(non_snake_case)]
impl TodoMutations {
    pub async fn createTodo(
        ctx: &async_graphql::Context<'_>,
        title: String,
        description: Option<String>,
    ) -> async_graphql::Result<entity::todo::Model> {
        let db = ctx.data::<sea_orm::DatabaseConnection>().unwrap().clone();
        let clock = ctx.data::<preflight_core::Clock>().unwrap().clone();
        let todo = preflight_core::TodoService::new(&db, &clock)
            .create(title, description)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        Ok(todo)
    }

    pub async fn updateTodo(
        ctx: &async_graphql::Context<'_>,
        id: i64,
        title: Option<String>,
        description: Option<String>,
    ) -> async_graphql::Result<entity::todo::Model> {
        let db = ctx.data::<sea_orm::DatabaseConnection>().unwrap().clone();
        let clock = ctx.data::<preflight_core::Clock>().unwrap().clone();
        let todo = preflight_core::TodoService::new(&db, &clock)
            .update(id, title, description)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        Ok(todo)
    }

    pub async fn setTodoStatus(
        ctx: &async_graphql::Context<'_>,
        id: i64,
        status: TodoStatus,
        blockedReason: Option<String>,
    ) -> async_graphql::Result<entity::todo::Model> {
        let db = ctx.data::<sea_orm::DatabaseConnection>().unwrap().clone();
        let clock = ctx.data::<preflight_core::Clock>().unwrap().clone();
        let todo = preflight_core::TodoService::new(&db, &clock)
            .set_status(id, status, blockedReason)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        Ok(todo)
    }
}
