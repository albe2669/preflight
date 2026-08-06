//! preflight TUI binary entry point.

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let endpoint = std::env::var("PREFLIGHT_GRAPHQL_ENDPOINT")
        .unwrap_or_else(|_| "http://127.0.0.1:8000/".to_string());
    tui::run(&endpoint).await
}
