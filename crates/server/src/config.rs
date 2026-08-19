use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub database: DatabaseConfig,
    pub clock: ClockConfig,
    pub sync: SyncConfig,
    pub server: ServerConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
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
#[serde(deny_unknown_fields)]
pub struct SyncConfig {
    pub github_token: String,
    pub linear_token: String,
    /// Optional path to a secrets file holding the GitHub token (sops-nix
    /// style). When set, its trimmed contents win over `github_token`.
    pub github_token_path: Option<String>,
    /// Optional path to a secrets file holding the Linear token. When set,
    /// its trimmed contents win over `linear_token`.
    pub linear_token_path: Option<String>,
    pub github: GithubSyncConfig,
    pub linear: LinearSyncConfig,
}

/// Resolve a token from inline config or a secrets file (sops-nix style).
/// When `path` is set (non-blank) it wins: the file's contents, trimmed, are
/// the token. Otherwise the inline value is used unchanged.
pub fn resolve_token(inline: &str, path: Option<&str>) -> anyhow::Result<String> {
    match path {
        Some(p) if !p.trim().is_empty() => {
            let content = std::fs::read_to_string(p)
                .map_err(|e| anyhow::anyhow!("read token path {p}: {e}"))?;
            Ok(content.trim().to_string())
        }
        _ => Ok(inline.to_string()),
    }
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct GithubSyncConfig {
    pub exclude_drafts_unless_authored_by_me: bool,
    pub filters: Vec<GithubFilterRule>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct LinearSyncConfig {
    pub filters: Vec<LinearFilterRule>,
}

#[derive(Debug, Deserialize, Default, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GithubFilterRule {
    pub repo: Option<String>,
    pub author: Option<String>,
    pub reviewer: Option<String>,
    pub reviewing_team: Option<String>,
    #[serde(default)]
    pub exclude_others_drafts: bool,
    #[serde(default)]
    pub exclude_my_drafts: bool,
}

#[derive(Debug, Deserialize, Default, Clone, PartialEq)]
pub struct LinearFilterRule {
    pub team: Option<String>,
    pub assignee: Option<String>,
    pub creator: Option<String>,
    pub project_lead: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub depth_limit: Option<usize>,
    pub complexity_limit: Option<usize>,
    #[serde(default)]
    pub cors_origins: Vec<String>,
}

impl ServerConfig {
    /// Origins the web frontend may call from. Defaults to the common Vite
    /// dev ports so a source checkout works without extra config.
    pub fn allowed_origins(&self) -> Vec<String> {
        if self.cors_origins.is_empty() {
            vec![
                "http://localhost:5173".into(),
                "http://127.0.0.1:5173".into(),
                "http://localhost:4173".into(),
                "http://127.0.0.1:4173".into(),
            ]
        } else {
            self.cors_origins.clone()
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct LoggingConfig {
    pub level: String,
    pub directory: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".into(),
            directory: "logs".into(),
        }
    }
}

impl Config {
    /// Load config from `config/default.toml`, or a path named by the CONFIG env var.
    pub fn load() -> anyhow::Result<Config> {
        let path = std::env::var("CONFIG").unwrap_or_else(|_| "config/default.toml".to_string());
        let content = std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("failed to read config {path}: {e}"))?;
        Self::from_content(&content)
    }

    /// Parse a TOML string into a `Config` and validate it.
    pub fn from_content(content: &str) -> anyhow::Result<Config> {
        let cfg: Config = toml::from_str(content).map_err(|e| {
            let msg = e.to_string();
            let hint = if msg.contains("github_search_query") {
                " `github_search_query` was removed; use `[[sync.github.filters]]` instead."
            } else if msg.contains("linear_team_keys") {
                " `linear_team_keys` was removed; use `[[sync.linear.filters]]` instead."
            } else if msg.contains("exclude_draft") {
                " `exclude_draft` was renamed to `exclude_others_drafts`."
            } else {
                ""
            };
            anyhow::anyhow!("failed to parse config:{hint} {e}")
        })?;
        cfg.sync
            .github
            .validate()
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        cfg.sync
            .linear
            .validate()
            .map_err(|e| anyhow::anyhow!("{e}"))?;
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

impl GithubSyncConfig {
    fn validate(&self) -> Result<(), String> {
        for (i, rule) in self.filters.iter().enumerate() {
            if rule.is_empty() {
                return Err(format!(
                    "sync.github.filters[{i}] has no conditions; set at least one field"
                ));
            }
            if let Some(author) = &rule.author {
                if author == "me" {
                    return Err(format!(
                        "sync.github.filters[{i}].author = \"me\" is not allowed; use \"@me\" instead"
                    ));
                }
            }
            if let Some(reviewer) = &rule.reviewer {
                if reviewer == "me" {
                    return Err(format!(
                        "sync.github.filters[{i}].reviewer = \"me\" is not allowed; use \"@me\" instead"
                    ));
                }
            }
        }
        Ok(())
    }
}

impl LinearSyncConfig {
    fn validate(&self) -> Result<(), String> {
        for (i, rule) in self.filters.iter().enumerate() {
            if rule.is_empty() {
                return Err(format!(
                    "sync.linear.filters[{i}] has no conditions; set at least one field"
                ));
            }
        }
        Ok(())
    }
}

impl GithubFilterRule {
    fn is_empty(&self) -> bool {
        self.repo.is_none()
            && self.author.is_none()
            && self.reviewer.is_none()
            && self.reviewing_team.is_none()
            && !self.exclude_others_drafts
            && !self.exclude_my_drafts
    }
}

impl LinearFilterRule {
    fn is_empty(&self) -> bool {
        self.team.is_none()
            && self.assignee.is_none()
            && self.creator.is_none()
            && self.project_lead.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn github_two_rule_toml() -> &'static str {
        r#"
github_token = "tok"
linear_token = "tok"

[github]
exclude_drafts_unless_authored_by_me = true
filters = [
  { repo = "api-specs", reviewing_team = "ai-agents", exclude_others_drafts = true },
  { author = "@me" },
]

[linear]
filters = [
  { creator = "me" },
  { assignee = "me" },
]
"#
    }

    #[test]
    fn test_parse_github_two_rules_load() {
        let cfg: SyncConfig = toml::from_str(github_two_rule_toml()).unwrap();
        assert_eq!(cfg.github.filters.len(), 2);
        assert_eq!(cfg.github.filters[0].repo.as_deref(), Some("api-specs"));
        assert!(cfg.github.filters[0].exclude_others_drafts);
        assert!(cfg.github.exclude_drafts_unless_authored_by_me);
        assert_eq!(cfg.linear.filters.len(), 2);
    }

    #[test]
    fn test_parse_old_github_search_query_rejected() {
        let toml = r#"
github_token = "tok"
linear_token = "tok"
github_search_query = "repo:foo/bar"
[github]
filters = []
[linear]
filters = []
"#;
        let res: Result<SyncConfig, _> = toml::from_str(toml);
        assert!(res.is_err());
        let err = res.unwrap_err().to_string();
        assert!(
            err.contains("github_search_query"),
            "error should mention the removed key: {err}"
        );
    }

    #[test]
    fn test_parse_old_linear_team_keys_rejected() {
        let toml = r#"
github_token = "tok"
linear_token = "tok"
linear_team_keys = ["ENG"]
[github]
filters = []
[linear]
filters = []
"#;
        let res: Result<SyncConfig, _> = toml::from_str(toml);
        assert!(res.is_err());
        let err = res.unwrap_err().to_string();
        assert!(
            err.contains("linear_team_keys"),
            "error should mention the removed key: {err}"
        );
    }

    #[test]
    fn test_validate_rejects_empty_github_rule() {
        let cfg = SyncConfig {
            github_token: "tok".into(),
            linear_token: "tok".into(),
            github: GithubSyncConfig {
                exclude_drafts_unless_authored_by_me: false,
                filters: vec![GithubFilterRule {
                    repo: None,
                    author: None,
                    reviewer: None,
                    reviewing_team: None,
                    exclude_others_drafts: false,
                    exclude_my_drafts: false,
                }],
            },
            github_token_path: None,
            linear_token_path: None,
            linear: LinearSyncConfig::default(),
        };
        let err = cfg.github.validate().unwrap_err();
        assert!(err.contains("sync.github.filters[0]"));
        assert!(err.contains("no conditions"));
    }

    #[test]
    fn test_validate_rejects_empty_linear_rule() {
        let cfg = SyncConfig {
            github_token: "tok".into(),
            linear_token: "tok".into(),
            github: GithubSyncConfig::default(),
            linear: LinearSyncConfig {
                filters: vec![LinearFilterRule {
                    team: None,
                    assignee: None,
                    creator: None,
                    project_lead: None,
                }],
            },
            github_token_path: None,
            linear_token_path: None,
        };
        let err = cfg.linear.validate().unwrap_err();
        assert!(err.contains("sync.linear.filters[0]"));
        assert!(err.contains("no conditions"));
    }

    #[test]
    fn test_validate_empty_filter_list_is_ok() {
        let cfg = SyncConfig {
            github_token: "tok".into(),
            linear_token: "tok".into(),
            github_token_path: None,
            linear_token_path: None,
            github: GithubSyncConfig::default(),
            linear: LinearSyncConfig::default(),
        };
        assert!(cfg.github.validate().is_ok());
        assert!(cfg.linear.validate().is_ok());
    }

    #[test]
    fn test_load_default_toml_parses_and_validates() {
        // Tests run with cwd = crates/server, so resolve the workspace-root
        // config via CARGO_MANIFEST_DIR and parse it directly (Config::load
        // reads CONFIG env var / cwd-relative path; mutating env is unsafe in
        // edition 2024).
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let path = format!("{manifest_dir}/../../config/default.toml");
        let content = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
        let cfg: Config = toml::from_str(&content).expect("default.toml should parse");
        cfg.sync.github.validate().unwrap();
        cfg.sync.linear.validate().unwrap();
        assert!(cfg.sync.github.filters.is_empty());
        assert!(cfg.sync.linear.filters.is_empty());
        assert!(cfg.sync.github.exclude_drafts_unless_authored_by_me);
    }

    #[test]
    fn test_resolve_token_inline_when_no_path() {
        assert_eq!(resolve_token("tok", None).unwrap(), "tok");
        assert_eq!(resolve_token("tok", Some("")).unwrap(), "tok");
        assert_eq!(resolve_token("tok", Some("  ")).unwrap(), "tok");
    }

    #[test]
    fn test_resolve_token_path_wins_and_trims() {
        let dir = std::env::temp_dir().join(format!("preflight-tok-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("github-token");
        std::fs::write(&path, "  secret-token\n").unwrap();

        // Path wins over an inline value, and the contents are trimmed.
        let resolved = resolve_token("inline-tok", Some(path.to_str().unwrap())).unwrap();
        assert_eq!(resolved, "secret-token");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_resolve_token_missing_path_errors() {
        let err = resolve_token("tok", Some("/nonexistent/preflight-token-file")).unwrap_err();
        assert!(err.to_string().contains("read token path"));
    }
    #[test]
    fn test_exclude_others_drafts_parses() {
        let toml = r#"
github_token = "tok"
linear_token = "tok"
[github]
filters = [
  { repo = "x", exclude_others_drafts = true },
]
[linear]
filters = []
"#;
        let cfg: SyncConfig = toml::from_str(toml).unwrap();
        assert!(cfg.github.filters[0].exclude_others_drafts);
        assert!(!cfg.github.filters[0].exclude_my_drafts);
    }

    #[test]
    fn test_exclude_my_drafts_parses() {
        let toml = r#"
github_token = "tok"
linear_token = "tok"
[github]
filters = [
  { repo = "x", exclude_my_drafts = true },
]
[linear]
filters = []
"#;
        let cfg: SyncConfig = toml::from_str(toml).unwrap();
        assert!(!cfg.github.filters[0].exclude_others_drafts);
        assert!(cfg.github.filters[0].exclude_my_drafts);
    }

    #[test]
    fn test_old_exclude_draft_key_rejected() {
        let toml = r#"
[database]
path = "/tmp/preflight.db"
[clock]
timezone = "UTC"
day_start_hour = 8
[sync]
github_token = "tok"
linear_token = "tok"
[sync.github]
filters = [
  { repo = "x", exclude_draft = true },
]
[sync.linear]
filters = []
[server]
host = "127.0.0.1"
port = 0
"#;
        let res = Config::from_content(toml);
        assert!(res.is_err());
        let err = res.unwrap_err().to_string();
        assert!(
            err.contains("exclude_others_drafts"),
            "error should mention the renamed key: {err}"
        );
    }

    #[test]
    fn test_at_me_author_parses_and_validates() {
        let toml = r#"
[database]
path = "/tmp/preflight.db"
[clock]
timezone = "UTC"
day_start_hour = 8
[sync]
github_token = "tok"
linear_token = "tok"
[sync.github]
filters = [
  { author = "@me" },
]
[sync.linear]
filters = []
[server]
host = "127.0.0.1"
port = 0
"#;
        let cfg = Config::from_content(toml).unwrap();
        assert_eq!(cfg.sync.github.filters[0].author.as_deref(), Some("@me"));
    }

    #[test]
    fn test_bare_me_author_rejected() {
        let toml = r#"
[database]
path = "/tmp/preflight.db"
[clock]
timezone = "UTC"
day_start_hour = 8
[sync]
github_token = "tok"
linear_token = "tok"
[sync.github]
filters = [
  { author = "me" },
]
[sync.linear]
filters = []
[server]
host = "127.0.0.1"
port = 0
"#;
        let err = Config::from_content(toml).unwrap_err().to_string();
        assert!(err.contains("@me"), "error should direct to use @me: {err}");
    }
    #[test]
    fn test_logging_config_default_values() {
        let cfg = LoggingConfig::default();
        assert_eq!(cfg.level, "info");
        assert_eq!(cfg.directory, "logs");
    }

    #[test]
    fn test_config_without_logging_section_uses_defaults() {
        let toml = r#"
[database]
path = "/tmp/preflight.db"
[clock]
timezone = "UTC"
day_start_hour = 8
[sync]
github_token = "tok"
linear_token = "tok"
[sync.github]
filters = []
[sync.linear]
filters = []
[server]
host = "127.0.0.1"
port = 0
"#;
        let cfg = Config::from_content(toml).unwrap();
        assert_eq!(cfg.logging.level, "info");
        assert_eq!(cfg.logging.directory, "logs");
    }

    #[test]
    fn test_config_with_logging_section_parses() {
        let toml = r#"
[database]
path = "/tmp/preflight.db"
[clock]
timezone = "UTC"
day_start_hour = 8
[sync]
github_token = "tok"
linear_token = "tok"
[sync.github]
filters = []
[sync.linear]
filters = []
[server]
host = "127.0.0.1"
port = 0
[logging]
level = "debug"
directory = "/tmp/logs"
"#;
        let cfg = Config::from_content(toml).unwrap();
        assert_eq!(cfg.logging.level, "debug");
        assert_eq!(cfg.logging.directory, "/tmp/logs");
    }

    #[test]
    fn test_cors_origins_default_when_unset() {
        let toml = r#"
[database]
path = "/tmp/preflight.db"
[clock]
timezone = "UTC"
day_start_hour = 8
[sync]
github_token = "tok"
linear_token = "tok"
[sync.github]
filters = []
[sync.linear]
filters = []
[server]
host = "127.0.0.1"
port = 0
"#;
        let cfg = Config::from_content(toml).unwrap();
        let origins = cfg.server.allowed_origins();
        assert!(
            origins.contains(&"http://localhost:5173".to_string()),
            "default should allow the Vite dev origin: {origins:?}"
        );
    }

    #[test]
    fn test_cors_origins_explicit_override() {
        let toml = r#"
[database]
path = "/tmp/preflight.db"
[clock]
timezone = "UTC"
day_start_hour = 8
[sync]
github_token = "tok"
linear_token = "tok"
[sync.github]
filters = []
[sync.linear]
filters = []
[server]
host = "127.0.0.1"
port = 0
cors_origins = ["https://preflight.example.com"]
"#;
        let cfg = Config::from_content(toml).unwrap();
        assert_eq!(
            cfg.server.allowed_origins(),
            vec!["https://preflight.example.com".to_string()]
        );
    }

    #[test]
    fn test_cors_origins_wildcard_parses() {
        let toml = r#"
[database]
path = "/tmp/preflight.db"
[clock]
timezone = "UTC"
day_start_hour = 8
[sync]
github_token = "tok"
linear_token = "tok"
[sync.github]
filters = []
[sync.linear]
filters = []
[server]
host = "127.0.0.1"
port = 0
cors_origins = ["*"]
"#;
        let cfg = Config::from_content(toml).unwrap();
        assert_eq!(cfg.server.allowed_origins(), vec!["*".to_string()]);
    }
}
