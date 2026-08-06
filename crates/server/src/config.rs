use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub database: DatabaseConfig,
    pub clock: ClockConfig,
    pub sync: SyncConfig,
    pub server: ServerConfig,
}

#[derive(Debug, Deserialize)]
pub struct DatabaseConfig {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct ClockConfig {
    pub timezone: String,
    pub day_start_hour: i64,
}

#[derive(Debug, Deserialize)]
pub struct SyncConfig {
    pub github_token: String,
    pub github_search_query: String,
    pub linear_token: String,
    pub linear_team_keys: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub depth_limit: Option<usize>,
    pub complexity_limit: Option<usize>,
}

impl Config {
    /// Load config from `config/default.toml`, or a path named by the CONFIG env var.
    pub fn load() -> anyhow::Result<Config> {
        let path = std::env::var("CONFIG").unwrap_or_else(|_| "config/default.toml".to_string());
        let content = std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("failed to read config {path}: {e}"))?;
        let cfg: Config =
            toml::from_str(&content).map_err(|e| anyhow::anyhow!("failed to parse config: {e}"))?;
        Ok(cfg)
    }

    /// Build a `todo_domain::Clock` from the clock config.
    pub fn clock(&self) -> anyhow::Result<todo_domain::Clock> {
        let tz: chrono_tz::Tz = self
            .clock
            .timezone
            .parse()
            .map_err(|e| anyhow::anyhow!("invalid timezone {}: {e}", self.clock.timezone))?;
        Ok(todo_domain::Clock::new(tz, self.clock.day_start_hour))
    }
}
