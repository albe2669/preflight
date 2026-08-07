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

    let todo_svc = Arc::new(todo_domain::todo_service::new(db.clone(), clock.clone()));
    let day_plan_svc = Arc::new(todo_domain::day_plan::new(db.clone(), clock.clone()));
    let link_svc = Arc::new(links::new(db.clone(), clock.clone(), day_plan_svc.clone()));
    let review_svc = Arc::new(todo_domain::review::new(db.clone(), clock.clone()));
    let github_filters: Vec<github::sync::GithubFilter> = cfg
        .sync
        .github
        .filters
        .iter()
        .map(|r| github::sync::GithubFilter {
            repo: r.repo.clone(),
            author: r.author.as_deref().map(parse_github_actor),
            reviewer: r.reviewer.as_deref().map(parse_github_actor),
            reviewing_team: r.reviewing_team.clone(),
            exclude_draft: r.exclude_draft,
        })
        .collect();
    let linear_filters: Vec<linear::sync::LinearFilter> = cfg
        .sync
        .linear
        .filters
        .iter()
        .map(|r| linear::sync::LinearFilter {
            team: r.team.clone(),
            assignee: r.assignee.as_deref().map(parse_linear_actor),
            creator: r.creator.as_deref().map(parse_linear_actor),
            project_lead: r.project_lead.as_deref().map(parse_linear_actor),
        })
        .collect();
    let github_token = config::resolve_token(
        &cfg.sync.github_token,
        cfg.sync.github_token_path.as_deref(),
    )?;
    let linear_token = config::resolve_token(
        &cfg.sync.linear_token,
        cfg.sync.linear_token_path.as_deref(),
    )?;
    let github_client: Arc<dyn github::GithubApiClient> = Arc::new(github::new_client(
        github_token.clone(),
        "https://api.github.com/graphql".to_string(),
    ));
    let github_sync = Arc::new(github::sync::new(
        db.clone(),
        github_client,
        github::sync::GithubOptions {
            token: github_token,
            filters: github_filters,
            exclude_drafts_unless_authored_by_me: cfg
                .sync
                .github
                .exclude_drafts_unless_authored_by_me,
        },
    ));
    let linear_client = linear::new_client(
        linear_token.clone(),
        "https://api.linear.app/graphql".to_string(),
    );
    let linear_sync = Arc::new(linear::sync::new(
        db.clone(),
        linear_client,
        linear::sync::LinearOptions {
            token: linear_token,
            filters: linear_filters,
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

/// Map a config string ("me" or a login) to the GitHub domain `Author` enum.
fn parse_github_actor(s: &str) -> github::Author {
    if s == "me" {
        github::Author::Me
    } else {
        github::Author::Login(s.to_string())
    }
}

/// Map a config string ("me" or a name) to the Linear domain `Actor` enum.
fn parse_linear_actor(s: &str) -> linear::Actor {
    if s == "me" {
        linear::Actor::Me
    } else {
        linear::Actor::Name(s.to_string())
    }
}
