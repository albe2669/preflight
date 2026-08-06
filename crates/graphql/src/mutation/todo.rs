//! Todo mutations: create, update, set status.

use async_graphql;
use std::sync::Arc;
use todo_domain::entity::enums::TodoStatus;

pub struct TodoMutations;

#[seaography::CustomFields]
#[allow(non_snake_case)]
impl TodoMutations {
    pub async fn createTodo(
        ctx: &async_graphql::Context<'_>,
        title: String,
        description: Option<String>,
    ) -> async_graphql::Result<todo_domain::entity::todo::Model> {
        let svc = ctx
            .data::<Arc<dyn todo_domain::TodoService>>()
            .unwrap()
            .clone();
        let todo = svc
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
    ) -> async_graphql::Result<todo_domain::entity::todo::Model> {
        let svc = ctx
            .data::<Arc<dyn todo_domain::TodoService>>()
            .unwrap()
            .clone();
        let todo = svc
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
    ) -> async_graphql::Result<todo_domain::entity::todo::Model> {
        let svc = ctx
            .data::<Arc<dyn todo_domain::TodoService>>()
            .unwrap()
            .clone();
        let todo = svc
            .set_status(id, status, blockedReason)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        Ok(todo)
    }
}
