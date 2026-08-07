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
pub struct GithubFilterRule {
    pub repo: Option<String>,
    pub author: Option<String>,
    pub reviewer: Option<String>,
    pub reviewing_team: Option<String>,
    #[serde(default)]
    pub exclude_draft: bool,
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
}

impl Config {
    /// Load config from `config/default.toml`, or a path named by the CONFIG env var.
    pub fn load() -> anyhow::Result<Config> {
        let path = std::env::var("CONFIG").unwrap_or_else(|_| "config/default.toml".to_string());
        let content = std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("failed to read config {path}: {e}"))?;
        let cfg: Config = toml::from_str(&content).map_err(|e| {
            let msg = e.to_string();
            let hint = if msg.contains("github_search_query") {
                " `github_search_query` was removed; use `[[sync.github.filters]]` instead."
            } else if msg.contains("linear_team_keys") {
                " `linear_team_keys` was removed; use `[[sync.linear.filters]]` instead."
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
            && !self.exclude_draft
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
  { repo = "api-specs", reviewing_team = "ai-agents", exclude_draft = true },
  { author = "me" },
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
                    exclude_draft: false,
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
}
