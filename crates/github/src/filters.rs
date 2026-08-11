//! GitHub query compilation and post-fetch filtering.

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GithubFilter {
    pub repo: Option<String>,
    pub author: Option<Author>,
    pub reviewer: Option<Author>,
    pub reviewing_team: Option<String>,
    pub exclude_others_drafts: bool,
    pub exclude_my_drafts: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Author {
    Me,
    Login(String),
}

/// A fetched PR row used by the post-fetch filters.
#[derive(Clone, Debug, PartialEq)]
pub struct FetchedPr {
    pub repo_owner: String,
    pub repo_name: String,
    pub number: i64,
    pub title: String,
    pub url: String,
    pub author_login: Option<String>,
    pub state: String,
    pub is_draft: bool,
    pub review_requested: bool,
    pub requested_reviewer_teams: Vec<String>,
    pub authored_by_me: bool,
    pub remote_created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub remote_updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl GithubFilter {
    pub fn is_empty(&self) -> bool {
        self.repo.is_none()
            && self.author.is_none()
            && self.reviewer.is_none()
            && self.reviewing_team.is_none()
            && !self.exclude_others_drafts
            && !self.exclude_my_drafts
    }
}

/// Compile a list of filters into a single GitHub search query string.
pub fn compile_github_query(filters: &[GithubFilter]) -> String {
    let filters: Vec<&GithubFilter> = filters.iter().filter(|f| !f.is_empty()).collect();
    if filters.is_empty() {
        return String::new();
    }

    if filters.len() == 1 {
        let clause = filter_to_clause(filters[0]);
        if clause.is_empty() {
            return String::new();
        }
        return format!("is:pr AND {clause}");
    }

    let clauses: Vec<String> = filters.iter().map(|f| filter_to_clause(f)).collect();
    let or_part = clauses
        .iter()
        .map(|c| {
            // wrap multi-part clauses in parens for OR grouping
            if c.contains(" AND ") {
                format!("({c})")
            } else {
                c.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" OR ");
    format!("is:pr AND is:open AND ({or_part})")
}

fn filter_to_clause(f: &GithubFilter) -> String {
    let mut parts = Vec::new();

    if let Some(repo) = &f.repo {
        parts.push(format!("repo:{repo}"));
    }

    if let Some(author) = &f.author {
        match author {
            Author::Me => parts.push("author:@me".into()),
            Author::Login(login) => parts.push(format!("author:{login}")),
        }
    }

    if let Some(reviewer) = &f.reviewer {
        match reviewer {
            Author::Me => parts.push("user-review-requested:@me".into()),
            Author::Login(login) => parts.push(format!("review-requested:{login}")),
        }
    }

    if let Some(team) = &f.reviewing_team {
        parts.push(format!("team-review-requested:{team}"));
    }

    if f.exclude_my_drafts {
        parts.push("draft:false".into());
    } else if f.exclude_others_drafts {
        parts.push("(draft:false OR author:@me)".into());
    }

    parts.join(" AND ")
}

/// Drop PRs whose requested_reviewer_teams include any excluded team.
pub fn apply_team_exclusion(prs: Vec<FetchedPr>, excluded_teams: &[String]) -> Vec<FetchedPr> {
    prs.into_iter()
        .filter(|pr| {
            !pr.requested_reviewer_teams
                .iter()
                .any(|t| excluded_teams.contains(t))
        })
        .collect()
}

fn rule_matches_pr(f: &GithubFilter, pr: &FetchedPr) -> bool {
    if let Some(repo) = &f.repo {
        let repo_full = format!("{}/{}", pr.repo_owner, pr.repo_name);
        if repo.contains('/') {
            if repo != &repo_full {
                return false;
            }
        } else {
            // repo as org-only — match owner
            if repo != &pr.repo_owner {
                return false;
            }
        }
    }

    if let Some(author) = &f.author {
        match author {
            Author::Me => {
                if !pr.authored_by_me {
                    return false;
                }
            }
            Author::Login(l) => {
                if pr.author_login.as_deref() != Some(l.as_str()) {
                    return false;
                }
            }
        }
    }

    if let Some(reviewer) = &f.reviewer {
        match reviewer {
            Author::Me => {
                if !pr.review_requested {
                    return false;
                }
            }
            Author::Login(_) => {
                // FetchedPr lacks requested reviewer logins — documented limitation
                return false;
            }
        }
    }

    if let Some(team) = &f.reviewing_team {
        if !pr.requested_reviewer_teams.iter().any(|t| t == team) {
            return false;
        }
    }

    true
}

/// When policy on, keep non-drafts and drafts authored by me.
pub fn apply_draft_policy(
    prs: Vec<FetchedPr>,
    filters: &[GithubFilter],
    exclude_drafts_unless_authored_by_me: bool,
) -> Vec<FetchedPr> {
    prs.into_iter()
        .filter(|pr| {
            if !pr.is_draft {
                return true;
            }
            // Draft — check if any filter rule with exclude_my_drafts matches
            let has_exclude_my = filters
                .iter()
                .any(|f| f.exclude_my_drafts && rule_matches_pr(f, pr));
            if has_exclude_my {
                return !pr.authored_by_me;
            }
            // Global policy
            !exclude_drafts_unless_authored_by_me || pr.authored_by_me
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fetched_pr(number: i64, is_draft: bool, authored_by_me: bool) -> FetchedPr {
        FetchedPr {
            repo_owner: "o".into(),
            repo_name: "r".into(),
            number,
            title: "".into(),
            url: "".into(),
            author_login: Some("u".into()),
            state: "open".into(),
            is_draft,
            review_requested: false,
            requested_reviewer_teams: vec![],
            authored_by_me,
            remote_created_at: None,
            remote_updated_at: None,
        }
    }

    // -----------------------------------------------------------------------
    // compile_github_query golden tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_compile_github_query_all_my_prs_and_team_reviewed_non_draft() {
        let filters = vec![
            GithubFilter {
                author: Some(Author::Me),
                ..Default::default()
            },
            GithubFilter {
                repo: Some("api-specs".into()),
                reviewing_team: Some("ai-agents".into()),
                exclude_others_drafts: true,
                ..Default::default()
            },
        ];
        let got = compile_github_query(&filters);
        let expected = "is:pr AND (author:@me OR (repo:api-specs AND team-review-requested:ai-agents AND (draft:false OR author:@me)))";
        assert_eq!(&got, expected);
    }

    #[test]
    fn test_compile_github_query_single_repo_non_draft() {
        let filters = vec![GithubFilter {
            repo: Some("agent-api".into()),
            exclude_others_drafts: true,
            ..Default::default()
        }];
        let got = compile_github_query(&filters);
        let expected = "is:pr AND repo:agent-api AND (draft:false OR author:@me)";
        assert_eq!(&got, expected);
    }

    #[test]
    fn test_compile_github_query_single_repo_no_draft_filter() {
        let filters = vec![GithubFilter {
            repo: Some("albe2669/".into()),
            ..Default::default()
        }];
        let got = compile_github_query(&filters);
        let expected = "is:pr AND repo:albe2669/";
        assert_eq!(&got, expected);
    }

    #[test]
    fn test_compile_github_query_multiple_repos_author_and_reviewer() {
        let filters = vec![
            GithubFilter {
                repo: Some("ml-reasoning".into()),
                author: Some(Author::Me),
                ..Default::default()
            },
            GithubFilter {
                repo: Some("ml-reasoning".into()),
                reviewer: Some(Author::Me),
                ..Default::default()
            },
            GithubFilter {
                repo: Some("gocomo".into()),
                author: Some(Author::Me),
                ..Default::default()
            },
            GithubFilter {
                repo: Some("gocomo".into()),
                reviewer: Some(Author::Me),
                ..Default::default()
            },
        ];
        let got = compile_github_query(&filters);
        let expected = "is:pr AND ((repo:ml-reasoning AND author:@me) OR (repo:ml-reasoning AND user-review-requested:@me) OR (repo:gocomo AND author:@me) OR (repo:gocomo AND user-review-requested:@me))";
        assert_eq!(&got, expected);
    }

    #[test]
    fn test_compile_github_query_empty_filters() {
        let filters: Vec<GithubFilter> = vec![];
        let got = compile_github_query(&filters);
        assert_eq!(got, "");
    }

    // -----------------------------------------------------------------------
    // post-fetch filter tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_apply_team_exclusion_drops_matching_team() {
        let prs = vec![
            FetchedPr {
                repo_owner: "org".into(),
                repo_name: "repo".into(),
                number: 1,
                title: "".into(),
                url: "".into(),
                author_login: Some("alice".into()),
                state: "open".into(),
                is_draft: false,
                review_requested: false,
                requested_reviewer_teams: vec!["ai-agents".into(), "core".into()],
                authored_by_me: false,
                remote_created_at: None,
                remote_updated_at: None,
            },
            FetchedPr {
                repo_owner: "org".into(),
                repo_name: "repo".into(),
                number: 2,
                title: "".into(),
                url: "".into(),
                author_login: Some("bob".into()),
                state: "open".into(),
                is_draft: false,
                review_requested: false,
                requested_reviewer_teams: vec!["core".into()],
                authored_by_me: false,
                remote_created_at: None,
                remote_updated_at: None,
            },
        ];
        let got = apply_team_exclusion(prs, &["ai-agents".into()]);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].number, 2);
    }

    #[test]
    fn test_apply_team_exclusion_keeps_all_when_no_match() {
        let prs = vec![FetchedPr {
            repo_owner: "org".into(),
            repo_name: "repo".into(),
            number: 1,
            title: "".into(),
            url: "".into(),
            author_login: None,
            state: "open".into(),
            is_draft: false,
            review_requested: false,
            requested_reviewer_teams: vec!["core".into()],
            authored_by_me: false,
            remote_created_at: None,
            remote_updated_at: None,
        }];
        let got = apply_team_exclusion(prs, &["ai-agents".into()]);
        assert_eq!(got.len(), 1);
    }

    #[test]
    fn test_apply_draft_policy_on_keeps_non_draft_and_my_drafts() {
        let prs = vec![
            // non-draft, keep
            fetched_pr(1, false, false),
            // draft by me, keep
            fetched_pr(2, true, true),
            // draft by someone else, drop
            fetched_pr(3, true, false),
        ];
        let got = apply_draft_policy(prs, &[], true);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].number, 1);
        assert_eq!(got[1].number, 2);
    }

    #[test]
    fn test_apply_draft_policy_off_keeps_all() {
        let prs = vec![fetched_pr(1, false, false), fetched_pr(2, true, false)];
        let got = apply_draft_policy(prs, &[], false);
        assert_eq!(got.len(), 2);
    }

    // -----------------------------------------------------------------------
    // is_empty tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_is_empty_true_for_default() {
        let f = GithubFilter::default();
        assert!(f.is_empty());
    }

    #[test]
    fn test_is_empty_false_when_has_repo() {
        let f = GithubFilter {
            repo: Some("x".into()),
            ..Default::default()
        };
        assert!(!f.is_empty());
    }

    // -----------------------------------------------------------------------
    // 2.1 exclude_others_drafts compiles to (draft:false OR author:@me)
    // -----------------------------------------------------------------------

    #[test]
    fn test_exclude_others_drafts_compiles_clause() {
        let f = GithubFilter {
            exclude_others_drafts: true,
            ..Default::default()
        };
        let got = compile_github_query(&[f]);
        assert_eq!(&got, "is:pr AND (draft:false OR author:@me)");
    }

    // -----------------------------------------------------------------------
    // 2.2 exclude_my_drafts compiles to draft:false
    // -----------------------------------------------------------------------

    #[test]
    fn test_exclude_my_drafts_compiles_clause() {
        let f = GithubFilter {
            exclude_my_drafts: true,
            ..Default::default()
        };
        let got = compile_github_query(&[f]);
        assert_eq!(&got, "is:pr AND draft:false");
    }

    // -----------------------------------------------------------------------
    // 2.3 both set compiles to draft:false (exclude_my takes priority)
    // -----------------------------------------------------------------------

    #[test]
    fn test_both_draft_flags_compiles_to_draft_false() {
        let f = GithubFilter {
            exclude_others_drafts: true,
            exclude_my_drafts: true,
            ..Default::default()
        };
        let got = compile_github_query(&[f]);
        assert_eq!(&got, "is:pr AND draft:false");
    }

    // -----------------------------------------------------------------------
    // 3.1 global on + draft by me matched by exclude_others rule => kept
    // -----------------------------------------------------------------------

    #[test]
    fn test_global_on_exclude_others_rule_keeps_my_draft() {
        let prs = vec![fetched_pr(1, true, true)]; // draft by me
        let filters = vec![GithubFilter {
            repo: Some("o".into()),
            exclude_others_drafts: true,
            ..Default::default()
        }];
        let got = apply_draft_policy(prs, &filters, true);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].number, 1);
    }

    // -----------------------------------------------------------------------
    // 3.2 global on + draft by me matched by exclude_my rule => dropped
    // -----------------------------------------------------------------------

    #[test]
    fn test_global_on_exclude_my_rule_drops_my_draft() {
        let prs = vec![fetched_pr(1, true, true)]; // draft by me
        let filters = vec![GithubFilter {
            repo: Some("o".into()),
            exclude_my_drafts: true,
            ..Default::default()
        }];
        let got = apply_draft_policy(prs, &filters, true);
        assert_eq!(got.len(), 0);
    }

    // -----------------------------------------------------------------------
    // 3.3 draft by me matching a keep-mine AND an exclude_my rule => dropped
    // -----------------------------------------------------------------------

    #[test]
    fn test_draft_matching_keep_mine_and_exclude_my_dropped() {
        let prs = vec![fetched_pr(1, true, true)]; // draft by me
        let filters = vec![
            // keep-mine rule (exclude_others only — doesn't trigger exclude_my path)
            GithubFilter {
                repo: Some("o".into()),
                exclude_others_drafts: true,
                ..Default::default()
            },
            // exclude_my rule
            GithubFilter {
                repo: Some("o".into()),
                author: Some(Author::Me),
                exclude_my_drafts: true,
                ..Default::default()
            },
        ];
        let got = apply_draft_policy(prs, &filters, true);
        assert_eq!(got.len(), 0);
    }

    // -----------------------------------------------------------------------
    // is_empty tests for new draft fields
    // -----------------------------------------------------------------------

    #[test]
    fn test_is_empty_false_when_exclude_others_drafts() {
        let f = GithubFilter {
            exclude_others_drafts: true,
            ..Default::default()
        };
        assert!(!f.is_empty());
    }

    #[test]
    fn test_is_empty_false_when_exclude_my_drafts() {
        let f = GithubFilter {
            exclude_my_drafts: true,
            ..Default::default()
        };
        assert!(!f.is_empty());
    }
}
