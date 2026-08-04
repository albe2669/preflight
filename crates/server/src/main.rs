mod config;
mod db;
mod routes;

use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cfg = config::Config::load()?;
    let clock = cfg.clock()?;
    let db = db::connect(&cfg.database.path).await?;
    db::migrate(&db).await?;

    let todo_svc = Arc::new(preflight_core::todo_service::new(db.clone(), clock.clone()));
    let day_plan_svc = Arc::new(preflight_core::day_plan::new(db.clone(), clock.clone()));
    let link_svc = Arc::new(preflight_core::links::new(db.clone(), clock.clone()));
    let review_svc = Arc::new(preflight_core::review::new(db.clone(), clock.clone()));
    let github_sync = Arc::new(sync::github::new(
        db.clone(),
        sync::github::GithubOptions {
            token: cfg.sync.github_token.clone(),
            query: cfg.sync.github_search_query.clone(),
        },
    ));
    let linear_sync = Arc::new(sync::linear::new(
        db.clone(),
        sync::linear::LinearOptions {
            token: cfg.sync.linear_token.clone(),
            team_keys: cfg.sync.linear_team_keys.clone(),
        },
    ));

    let schema = graphql::schema_builder(
        db.clone(),
        todo_svc,
        day_plan_svc,
        link_svc,
        review_svc,
        github_sync,
        linear_sync,
        cfg.server.depth_limit,
        cfg.server.complexity_limit,
    )
    .finish()
    .map_err(|e| anyhow::anyhow!("schema build failed: {e:?}"))?;

    let app = routes::router(schema);
    let addr = format!("{}:{}", cfg.server.host, cfg.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("GraphQL Playground at http://{addr}");
    axum::serve(listener, app).await?;
    Ok(())
}
