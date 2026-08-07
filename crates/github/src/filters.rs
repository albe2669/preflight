//! GitHub query compilation and post-fetch filtering.

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GithubFilter {
    pub repo: Option<String>,
    pub author: Option<Author>,
    pub reviewer: Option<Author>,
    pub reviewing_team: Option<String>,
    pub exclude_draft: bool,
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
    pub author_login: Option<String>,
    pub is_draft: bool,
    pub requested_reviewer_teams: Vec<String>,
    pub authored_by_me: bool,
}

impl GithubFilter {
    pub fn is_empty(&self) -> bool {
        self.repo.is_none()
            && self.author.is_none()
            && self.reviewer.is_none()
            && self.reviewing_team.is_none()
            && !self.exclude_draft
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
    format!("is:pr AND ({or_part})")
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

    if f.exclude_draft {
        parts.push("draft:false".into());
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

/// When policy on, keep non-drafts and drafts authored by me.
pub fn apply_draft_policy(
    prs: Vec<FetchedPr>,
    exclude_drafts_unless_authored_by_me: bool,
) -> Vec<FetchedPr> {
    if !exclude_drafts_unless_authored_by_me {
        return prs;
    }
    prs.into_iter()
        .filter(|pr| !pr.is_draft || pr.authored_by_me)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

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
                exclude_draft: true,
                ..Default::default()
            },
        ];
        let got = compile_github_query(&filters);
        let expected = "is:pr AND (author:@me OR (repo:api-specs AND team-review-requested:ai-agents AND draft:false))";
        assert_eq!(&got, expected);
    }

    #[test]
    fn test_compile_github_query_single_repo_non_draft() {
        let filters = vec![GithubFilter {
            repo: Some("agent-api".into()),
            exclude_draft: true,
            ..Default::default()
        }];
        let got = compile_github_query(&filters);
        let expected = "is:pr AND repo:agent-api AND draft:false";
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
                author_login: Some("alice".into()),
                is_draft: false,
                requested_reviewer_teams: vec!["ai-agents".into(), "core".into()],
                authored_by_me: false,
            },
            FetchedPr {
                repo_owner: "org".into(),
                repo_name: "repo".into(),
                number: 2,
                author_login: Some("bob".into()),
                is_draft: false,
                requested_reviewer_teams: vec!["core".into()],
                authored_by_me: false,
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
            author_login: None,
            is_draft: false,
            requested_reviewer_teams: vec!["core".into()],
            authored_by_me: false,
        }];
        let got = apply_team_exclusion(prs, &["ai-agents".into()]);
        assert_eq!(got.len(), 1);
    }

    #[test]
    fn test_apply_draft_policy_on_keeps_non_draft_and_my_drafts() {
        let prs = vec![
            // non-draft, keep
            FetchedPr {
                repo_owner: "o".into(),
                repo_name: "r".into(),
                number: 1,
                author_login: None,
                is_draft: false,
                requested_reviewer_teams: vec![],
                authored_by_me: false,
            },
            // draft by me, keep
            FetchedPr {
                repo_owner: "o".into(),
                repo_name: "r".into(),
                number: 2,
                author_login: Some("me".into()),
                is_draft: true,
                requested_reviewer_teams: vec![],
                authored_by_me: true,
            },
            // draft by someone else, drop
            FetchedPr {
                repo_owner: "o".into(),
                repo_name: "r".into(),
                number: 3,
                author_login: Some("other".into()),
                is_draft: true,
                requested_reviewer_teams: vec![],
                authored_by_me: false,
            },
        ];
        let got = apply_draft_policy(prs, true);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].number, 1);
        assert_eq!(got[1].number, 2);
    }

    #[test]
    fn test_apply_draft_policy_off_keeps_all() {
        let prs = vec![
            FetchedPr {
                repo_owner: "o".into(),
                repo_name: "r".into(),
                number: 1,
                author_login: None,
                is_draft: false,
                requested_reviewer_teams: vec![],
                authored_by_me: false,
            },
            FetchedPr {
                repo_owner: "o".into(),
                repo_name: "r".into(),
                number: 2,
                author_login: Some("other".into()),
                is_draft: true,
                requested_reviewer_teams: vec![],
                authored_by_me: false,
            },
        ];
        let got = apply_draft_policy(prs, false);
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
}
