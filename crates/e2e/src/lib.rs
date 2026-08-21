#![cfg(feature = "e2e")]

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use async_graphql::dynamic::Schema;
use axum::Router;
use migration::MigratorTrait;
use sea_orm::{DatabaseConnection, SqlxSqliteConnector};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};

/// A running e2e server: the bound address, a shutdown signal, and the temp
/// db path kept alive until drop.
pub struct TestServer {
    pub addr: SocketAddr,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    _db: tempfile::TempPath,
}

impl TestServer {
    /// GraphQL endpoint URL.
    pub fn graphql_url(&self) -> String {
        format!("http://{}/graphql", self.addr)
    }

    /// Send the shutdown signal. Drop also sends it as a fallback.
    pub async fn shutdown(mut self) {
        let _ = self.shutdown.take().map(|s| s.send(()));
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        let _ = self.shutdown.take().map(|s| s.send(()));
    }
}

/// Spawn a full server stack on a random port backed by a temp SQLite db.
pub async fn spawn() -> anyhow::Result<TestServer> {
    let tmp = tempfile::NamedTempFile::new()?;
    let db_path = tmp.into_temp_path();
    let db = connect_db(db_path.to_str().unwrap()).await?;
    migration::Migrator::up(&db, None).await?;

    let clock = todo_domain::Clock::new(chrono_tz::America::Los_Angeles, 4);

    let todo_svc = Arc::new(todo_domain::todo_service::new(db.clone(), clock.clone()));
    let day_plan_svc = Arc::new(todo_domain::day_plan::new(db.clone(), clock.clone()));
    let link_svc = Arc::new(links::new(db.clone(), clock.clone(), day_plan_svc.clone()));
    let review_svc = Arc::new(todo_domain::review::new(db.clone(), clock.clone()));

    let github_client: Arc<dyn github::GithubApiClient> = Arc::new(github::new_client(
        String::new(),
        "https://api.github.com/graphql".to_string(),
    ));
    let github_sync = Arc::new(github::sync::new(
        db.clone(),
        github_client,
        github::sync::GithubOptions::default(),
    ));
    let linear_client =
        linear::new_client(String::new(), "https://api.linear.app/graphql".to_string());
    let linear_sync = Arc::new(linear::sync::new(
        db.clone(),
        linear_client,
        linear::sync::LinearOptions::default(),
    ));

    let schema = graphql::schema_builder(
        db,
        todo_svc,
        day_plan_svc,
        link_svc,
        review_svc,
        github_sync,
        linear_sync,
        clock,
        None,
        None,
    )
    .finish()
    .map_err(|e| anyhow::anyhow!("schema build failed: {e:?}"))?;

    let app = graphql_router(schema);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await
            .ok();
    });

    Ok(TestServer {
        addr,
        shutdown: Some(shutdown_tx),
        _db: db_path,
    })
}

async fn connect_db(path: &str) -> anyhow::Result<DatabaseConnection> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5));
    let pool = sqlx::SqlitePool::connect_with(options).await?;
    Ok(SqlxSqliteConnector::from_sqlx_sqlite_pool(pool))
}

/// Minimal router: only the `/graphql` POST handler. No CORS layer — the
/// reqwest test client posts to 127.0.0.1, same-host, so no preflight is due.
fn graphql_router(schema: Schema) -> Router {
    use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
    use axum::extract::State;
    use axum::routing::post;
    Router::new()
        .route(
            "/graphql",
            post(
                |State(schema): State<Schema>, req: GraphQLRequest| async move {
                    GraphQLResponse(schema.execute(req.into_inner()).await.into())
                },
            ),
        )
        .with_state(schema)
}

/// POST a GraphQL query and return the parsed JSON body.
pub async fn gql_post(url: &str, query: &str) -> anyhow::Result<serde_json::Value> {
    let client = reqwest::Client::new();
    let resp = client
        .post(url)
        .header("content-type", "application/json")
        .json(&serde_json::json!({ "query": query }))
        .send()
        .await?;
    Ok(resp.json().await?)
}
