mod config;
mod db;
mod logging;
mod routes;

use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = config::Config::load()?;

    // Resolve log level: PREFLIGHT_LOG_LEVEL > config level
    let (log_level, level_source) = match std::env::var("PREFLIGHT_LOG_LEVEL") {
        Ok(v) => (v, "PREFLIGHT_LOG_LEVEL"),
        Err(_) => (cfg.logging.level.clone(), "config"),
    };
    let level_filter =
        logging::parse_level(&log_level).map_err(|e| anyhow::anyhow!("invalid log level: {e}"))?;

    // Resolve log directory: PREFLIGHT_LOG_DIR > config directory
    let (log_dir, dir_source) = match std::env::var("PREFLIGHT_LOG_DIR") {
        Ok(v) => (v, "PREFLIGHT_LOG_DIR"),
        Err(_) => (cfg.logging.directory.clone(), "config"),
    };

    let _logging_guard = logging::init(level_filter, log_dir.as_ref())?;
    tracing::info!(
        level = %log_level,
        level_source = level_source,
        directory = %log_dir,
        directory_source = dir_source,
        "logging initialized"
    );
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
            exclude_others_drafts: r.exclude_others_drafts,
            exclude_my_drafts: r.exclude_my_drafts,
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
        clock.clone(),
        cfg.server.depth_limit,
        cfg.server.complexity_limit,
    )
    .finish()
    .map_err(|e| anyhow::anyhow!("schema build failed: {e:?}"))?;

    let addr = format!("{}:{}", cfg.server.host, cfg.server.port);
    let app = routes::router(
        schema,
        &cfg.server.allowed_origins(),
        cfg.server.frontend_dist.as_deref(),
    );
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("GraphQL Playground at http://{addr}");
    axum::serve(listener, app).await?;
    Ok(())
}
/// Map a config string ("@me" or a login) to the GitHub domain `Author` enum.
fn parse_github_actor(s: &str) -> github::Author {
    if s == "@me" {
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
