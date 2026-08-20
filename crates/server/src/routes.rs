use async_graphql::dynamic::Schema;
use async_graphql::http::{GraphQLPlaygroundConfig, playground_source};
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::http::{HeaderValue, Method};
use axum::{
    Router,
    extract::State,
    response::{self, IntoResponse},
    routing::get,
};
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::services::ServeDir;

async fn graphql_playground() -> impl IntoResponse {
    response::Html(playground_source(GraphQLPlaygroundConfig::new("/")))
}

async fn graphql_handler(State(schema): State<Schema>, req: GraphQLRequest) -> GraphQLResponse {
    schema.execute(req.into_inner()).await.into()
}

pub fn router(schema: Schema, cors_origins: &[String], frontend_dist: Option<&str>) -> Router {
    let cors = build_cors(cors_origins);
    let mut router = Router::new()
        .route("/", get(graphql_playground).post(graphql_handler))
        .with_state(schema)
        .layer(cors);
    if let Some(path) = frontend_dist {
        router = router.fallback_service(ServeDir::new(path));
    }
    router
}

/// Build a CORS layer from the configured origins. An empty list permits no
/// cross-origin callers; the GraphQL endpoint stays reachable from same-origin
/// tools (curl, the TUI, Raycast).
fn build_cors(origins: &[String]) -> CorsLayer {
    // "*" is the canonical "allow any origin" value; tower_http's
    // `AllowOrigin::list` panics on a wildcard, so route it to `any()`.
    let allow_origin = if origins.iter().any(|o| o == "*") {
        AllowOrigin::any()
    } else {
        let parsed: Vec<HeaderValue> = origins
            .iter()
            .filter_map(|o| HeaderValue::from_str(o).ok())
            .collect();
        AllowOrigin::list(parsed)
    };
    CorsLayer::new()
        .allow_origin(allow_origin)
        .allow_headers([
            axum::http::HeaderName::from_static("content-type"),
            axum::http::HeaderName::from_static("authorization"),
        ])
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
}
