//! preflight TUI binary entry point.

use serde::Deserialize;

/// GraphQL endpoint resolution:
///   1. `PREFLIGHT_GRAPHQL_ENDPOINT` env (explicit override),
///   2. `[server]` host+port from `~/.config/preflight/config.toml`,
///   3. `http://127.0.0.1:8000/graphql`.
fn resolve_endpoint() -> String {
    if let Ok(ep) = std::env::var("PREFLIGHT_GRAPHQL_ENDPOINT") {
        return ep;
    }
    if let Some(path) = config_path() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(cfg) = toml::from_str::<EndpointConfig>(&content) {
                if let Some(s) = cfg.server {
                    return format!("http://{}:{}/graphql", s.host, s.port);
                }
            }
        }
    }
    "http://127.0.0.1:8000/graphql".to_string()
}

/// Only the `[server]` table is read; all other sections are ignored.
#[derive(Deserialize)]
struct EndpointConfig {
    server: Option<ServerEndpoint>,
}

#[derive(Deserialize)]
struct ServerEndpoint {
    host: String,
    port: u16,
}

/// `$XDG_CONFIG_HOME/preflight/config.toml`, falling back to
/// `$HOME/.config/preflight/config.toml` per the XDG Base Directory spec.
fn config_path() -> Option<std::path::PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return Some(
                std::path::PathBuf::from(xdg)
                    .join("preflight")
                    .join("config.toml"),
            );
        }
    }
    let home = std::env::var("HOME").ok()?;
    Some(
        std::path::PathBuf::from(home)
            .join(".config")
            .join("preflight")
            .join("config.toml"),
    )
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let endpoint = resolve_endpoint();
    tui::run(&endpoint).await
}
