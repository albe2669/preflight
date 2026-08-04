use async_graphql::dynamic::Schema;
use async_graphql::http::{GraphQLPlaygroundConfig, playground_source};
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::{
    Router,
    extract::State,
    response::{self, IntoResponse},
    routing::get,
};

async fn graphql_playground() -> impl IntoResponse {
    response::Html(playground_source(GraphQLPlaygroundConfig::new("/")))
}

async fn graphql_handler(State(schema): State<Schema>, req: GraphQLRequest) -> GraphQLResponse {
    schema.execute(req.into_inner()).await.into()
}

pub fn router(schema: Schema) -> Router {
    Router::new()
        .route("/", get(graphql_playground).post(graphql_handler))
        .with_state(schema)
}
