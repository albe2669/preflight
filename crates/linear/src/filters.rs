//! Filter compilation for Linear sync queries.

use serde_json::Value;

/// Identity of an actor in a filter clause.
#[derive(Clone, Debug, PartialEq)]
pub enum Actor {
    Me,
    Name(String),
}

/// A single filter rule for Linear sync.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LinearFilter {
    pub team: Option<String>,
    pub assignee: Option<Actor>,
    pub creator: Option<Actor>,
    pub project_lead: Option<Actor>,
}

impl LinearFilter {
    fn is_empty(&self) -> bool {
        self.team.is_none()
            && self.assignee.is_none()
            && self.creator.is_none()
            && self.project_lead.is_none()
    }
}

/// Compile a slice of [`LinearFilter`] into a Linear GraphQL filter JSON value.
///
/// Returns [`Value::Null`] when no rules are present.  A single non-empty rule
/// is returned as a bare clause; multiple rules are wrapped in an `"or"` array.
/// Within a rule, multiple conditions are AND-ed via an `"and"` array.
pub fn compile_linear_filter(filters: &[LinearFilter]) -> Value {
    let mut clauses: Vec<Value> = filters
        .iter()
        .filter(|f| !f.is_empty())
        .map(rule_to_clause)
        .collect();

    match clauses.len() {
        0 => Value::Null,
        1 => clauses.pop().unwrap(),
        _ => serde_json::json!({ "or": clauses }),
    }
}

fn rule_to_clause(f: &LinearFilter) -> Value {
    let mut conditions: Vec<Value> = Vec::new();

    if let Some(team) = &f.team {
        conditions.push(serde_json::json!({
            "team": { "key": team }
        }));
    }

    if let Some(assignee) = &f.assignee {
        conditions.push(match assignee {
            Actor::Me => serde_json::json!({ "assignee": { "me": true } }),
            Actor::Name(name) => serde_json::json!({ "assignee": { "name": name } }),
        });
    }

    if let Some(creator) = &f.creator {
        conditions.push(match creator {
            Actor::Me => serde_json::json!({ "creator": { "me": true } }),
            Actor::Name(name) => serde_json::json!({ "creator": { "name": name } }),
        });
    }

    if let Some(project_lead) = &f.project_lead {
        // VERIFY: Linear may need a project UUID; asserting intended shape.
        conditions.push(match project_lead {
            Actor::Me => serde_json::json!({ "project": { "lead": { "me": true } } }),
            Actor::Name(name) => serde_json::json!({ "project": { "lead": { "name": name } } }),
        });
    }

    match conditions.len() {
        0 => Value::Null, // should not happen for non-empty rules
        1 => conditions.into_iter().next().unwrap(),
        _ => serde_json::json!({ "and": conditions }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_linear_filter_empty_filters_returns_null() {
        let filters: Vec<LinearFilter> = vec![];
        let result = compile_linear_filter(&filters);
        assert_eq!(result, Value::Null);
    }

    #[test]
    fn test_compile_linear_filter_single_creator_me_no_or_wrapper() {
        let filters = vec![LinearFilter {
            creator: Some(Actor::Me),
            ..Default::default()
        }];
        let result = compile_linear_filter(&filters);
        let expected = serde_json::json!({
            "creator": { "me": true }
        });
        pretty_assertions::assert_eq!(result, expected);
    }

    #[test]
    fn test_compile_linear_filter_creator_me_or_assignee_me() {
        let filters = vec![
            LinearFilter {
                creator: Some(Actor::Me),
                ..Default::default()
            },
            LinearFilter {
                assignee: Some(Actor::Me),
                ..Default::default()
            },
        ];
        let result = compile_linear_filter(&filters);
        let expected = serde_json::json!({
            "or": [
                { "creator": { "me": true } },
                { "assignee": { "me": true } }
            ]
        });
        pretty_assertions::assert_eq!(result, expected);
    }

    #[test]
    fn test_compile_linear_filter_team_and_assignee_me_single_rule() {
        let filters = vec![LinearFilter {
            team: Some("ENG".into()),
            assignee: Some(Actor::Me),
            ..Default::default()
        }];
        let result = compile_linear_filter(&filters);
        let expected = serde_json::json!({
            "and": [
                { "team": { "key": "ENG" } },
                { "assignee": { "me": true } }
            ]
        });
        pretty_assertions::assert_eq!(result, expected);
    }

    #[test]
    fn test_compile_linear_filter_project_lead_me() {
        // VERIFY: Linear may require a project UUID; asserting intended shape.
        let filters = vec![LinearFilter {
            project_lead: Some(Actor::Me),
            ..Default::default()
        }];
        let result = compile_linear_filter(&filters);
        let expected = serde_json::json!({
            "project": { "lead": { "me": true } }
        });
        pretty_assertions::assert_eq!(result, expected);
    }

    #[test]
    fn test_compile_linear_filter_project_lead_name() {
        let filters = vec![LinearFilter {
            project_lead: Some(Actor::Name("alice".into())),
            ..Default::default()
        }];
        let result = compile_linear_filter(&filters);
        let expected = serde_json::json!({
            "project": { "lead": { "name": "alice" } }
        });
        pretty_assertions::assert_eq!(result, expected);
    }

    #[test]
    fn test_compile_linear_filter_assignee_name() {
        let filters = vec![LinearFilter {
            assignee: Some(Actor::Name("bob".into())),
            ..Default::default()
        }];
        let result = compile_linear_filter(&filters);
        let expected = serde_json::json!({
            "assignee": { "name": "bob" }
        });
        pretty_assertions::assert_eq!(result, expected);
    }

    #[test]
    fn test_compile_linear_filter_empty_rules_skipped() {
        let filters = vec![
            LinearFilter::default(), // all-None
            LinearFilter::default(), // all-None
        ];
        let result = compile_linear_filter(&filters);
        assert_eq!(result, Value::Null);
    }
}
