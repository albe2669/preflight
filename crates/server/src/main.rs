mod config;
mod db;
mod routes;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cfg = config::Config::load()?;
    let clock = cfg.clock()?;
    let db = db::connect(&cfg.database.path).await?;
    db::migrate(&db).await?;

    let schema = graphql::schema_builder(
        db.clone(),
        clock.clone(),
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
