//! GraphQL client: type-safe queries and mutations over the backend's `/`.
//!
//! Built with cynic against the shared `api/schema.graphql` (introspected from
//! the live seaography server). The schema is regenerated via the
//! `introspect-schema` binary; `build.rs` feeds it to cynic's codegen so every
//! query struct is compile-checked against the SDL.

/// The schema module — cynic generates all schema types inside this.
#[cynic::schema("preflight")]
pub mod schema {}

// Bring the cynic-generated schema types (input objects, enums, filters,
// order-by) into scope so the query/mutation structs below can name them
// without a `schema::` prefix.
use schema::*;

/// A typed GraphQL client. Constructed with a base URL; owns an async
/// `reqwest` client.
#[derive(Clone)]
pub struct Client {
    endpoint: String,
    http: reqwest::Client,
}

#[derive(thiserror::Error, Debug)]
pub enum GqlError {
    #[error("transport: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("graphql: {0}")]
    Graphql(String),
    #[error("decode: {0}")]
    Decode(String),
}

type GqlResult<T> = std::result::Result<T, GqlError>;

// ---- Schema-derived types (cynic QueryFragments) ----

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "Query", variables = "FetchAllVars")]
pub struct FetchAll {
    #[arguments(filters: $filters)]
    pub todo: TodoConnection,
    #[arguments(filters: $plan_filters, orderBy: $plan_order)]
    pub todo_day_plan: TodoDayPlanConnection,
    #[arguments(filters: $pr_filters)]
    pub pull_request: PullRequestConnection,
    #[arguments(filters: $linear_filters)]
    pub linear_issue: LinearIssueConnection,
    pub sync_state: SyncStateConnection,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct FetchAllVars {
    pub filters: Option<TodoFilterInput>,
    pub plan_filters: Option<TodoDayPlanFilterInput>,
    pub plan_order: Option<TodoDayPlanOrderInput>,
    pub pr_filters: Option<PullRequestFilterInput>,
    pub linear_filters: Option<LinearIssueFilterInput>,
}

impl Default for FetchAllVars {
    fn default() -> Self {
        Self {
            filters: None,
            plan_filters: None,
            plan_order: Some(TodoDayPlanOrderInput {
                position: Some(OrderByEnum::Asc),
            }),
            pr_filters: None,
            linear_filters: None,
        }
    }
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "Todo")]
pub struct Todo {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub blocked_reason: Option<String>,
    pub sort_key: i64,
    pub created_at: String,
    pub started_at: Option<String>,
    pub closed_at: Option<String>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "TodoDayPlan")]
pub struct PlanRow {
    pub id: i64,
    pub position: i64,
    pub carried_over: bool,
    pub removed_at: Option<String>,
    pub todo: Option<Todo>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "PullRequest")]
pub struct PullRequest {
    pub id: i64,
    pub owner: String,
    pub repo: String,
    pub number: i64,
    pub title: String,
    pub url: String,
    pub author: Option<String>,
    pub state: String,
    pub review_requested: bool,
    pub authored_by_me: bool,
    pub dismissed_at: Option<String>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "LinearIssue")]
pub struct LinearIssue {
    pub id: i64,
    pub identifier: String,
    pub title: String,
    pub url: String,
    pub state_name: String,
    pub state_type: String,
    pub priority: Option<i64>,
    pub team_key: Option<String>,
    pub assignee_name: Option<String>,
    pub assigned_to_me: bool,
    pub dismissed_at: Option<String>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "SyncState")]
pub struct SyncState {
    pub source: String,
    pub cursor: Option<String>,
    pub last_synced_at: Option<String>,
    pub last_status: String,
    pub last_error: Option<String>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "TodoEvent")]
pub struct TodoEvent {
    pub id: i64,
    pub kind: String,
    pub field: Option<String>,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub actor: String,
    pub occurred_at: String,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "Tag")]
pub struct Tag {
    pub slug: String,
    pub name: String,
}

// ---- Connection wrappers ----

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "TodoConnection")]
pub struct TodoConnection {
    pub nodes: Vec<Todo>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "TodoDayPlanConnection")]
pub struct TodoDayPlanConnection {
    pub nodes: Vec<PlanRow>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "PullRequestConnection")]
pub struct PullRequestConnection {
    pub nodes: Vec<PullRequest>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "LinearIssueConnection")]
pub struct LinearIssueConnection {
    pub nodes: Vec<LinearIssue>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "SyncStateConnection")]
pub struct SyncStateConnection {
    pub nodes: Vec<SyncState>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "TodoEventConnection")]
pub struct TodoEventConnection {
    pub nodes: Vec<TodoEvent>,
}

// ---- Daily review ----

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "Query", variables = "DailyReviewVars")]
pub struct DailyReviewQuery {
    #[arguments(date: $date)]
    pub daily_review: DailyReview,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct DailyReviewVars {
    pub date: String,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "DailyReview")]
pub struct DailyReview {
    pub date: String,
    pub planned: Vec<Todo>,
    pub touched: Vec<Todo>,
    pub completed: Vec<Todo>,
    pub carried_over: Vec<Todo>,
}

// ---- Mutations ----

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "Mutation", variables = "CreateTodoVars")]
pub struct CreateTodoMut {
    #[arguments(title: $title, description: $desc)]
    pub create_todo: Todo,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct CreateTodoVars {
    pub title: String,
    pub desc: Option<String>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "Mutation", variables = "UpdateTodoVars")]
pub struct UpdateTodoMut {
    #[arguments(id: $id, title: $title, description: $desc)]
    pub update_todo: Todo,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct UpdateTodoVars {
    pub id: i64,
    pub title: Option<String>,
    pub desc: Option<String>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "Mutation", variables = "SetStatusVars")]
pub struct SetStatusMut {
    #[arguments(id: $id, status: $status, blockedReason: $blocked)]
    pub set_todo_status: Todo,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct SetStatusVars {
    pub id: i64,
    pub status: TodoStatusEnum,
    pub blocked: Option<String>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "Mutation", variables = "PlanTodayVars")]
pub struct PlanTodayMut {
    #[arguments(todoId: $todo_id)]
    pub plan_for_today: TodoDayPlanModel,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "TodoDayPlan")]
pub struct TodoDayPlanModel {
    pub id: i64,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct PlanTodayVars {
    pub todo_id: i64,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "Mutation", variables = "UnplanVars")]
pub struct UnplanMut {
    #[arguments(todoId: $todo_id)]
    pub unplan_for_today: bool,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct UnplanVars {
    pub todo_id: i64,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "Mutation", variables = "ReorderVars")]
pub struct ReorderMut {
    #[arguments(date: $date, todoIds: $todo_ids)]
    pub reorder_day_plan: Vec<TodoDayPlanModel>,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct ReorderVars {
    pub date: String,
    pub todo_ids: Vec<i64>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "Mutation", variables = "CarryOverVars")]
pub struct CarryOverMut {
    #[arguments(from: $from, to: $to)]
    pub carry_over_unfinished: Vec<TodoDayPlanModel>,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct CarryOverVars {
    pub from: String,
    pub to: String,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "Mutation", variables = "TagVars")]
pub struct AddTagMut {
    #[arguments(todoId: $todo_id, slug: $slug)]
    pub add_tag: Todo,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct TagVars {
    pub todo_id: i64,
    pub slug: String,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "Mutation", variables = "TagVars")]
pub struct RemoveTagMut {
    #[arguments(todoId: $todo_id, slug: $slug)]
    pub remove_tag: Todo,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "Mutation", variables = "TodoFromPrVars")]
pub struct TodoFromPrMut {
    #[arguments(pullRequestId: $pr_id, planToday: $plan_today)]
    pub todo_from_pull_request: Todo,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct TodoFromPrVars {
    pub pr_id: i64,
    pub plan_today: bool,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "Mutation", variables = "DismissPrVars")]
pub struct DismissPrMut {
    #[arguments(id: $id)]
    pub dismiss_pull_request: PullRequest,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct DismissPrVars {
    pub id: i64,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "Mutation", variables = "TodoFromLinearVars")]
pub struct TodoFromLinearMut {
    #[arguments(linearIssueId: $issue_id, planToday: $plan_today)]
    pub todo_from_linear_issue: Todo,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct TodoFromLinearVars {
    pub issue_id: i64,
    pub plan_today: bool,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "Mutation")]
pub struct SyncGithubMut {
    pub sync_github: SyncState,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "Mutation")]
pub struct SyncLinearMut {
    pub sync_linear: SyncState,
}

// ---- Client impl ----

impl Client {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            http: reqwest::Client::builder().build().expect("reqwest client"),
        }
    }

    async fn run<Q, V>(&self, _query: Q, vars: V) -> GqlResult<Q>
    where
        Q: cynic::QueryFragment + cynic::QueryBuilder<V>,
        V: cynic::QueryVariables + serde::Serialize,
    {
        let operation = Q::build(vars);
        self.post_operation(operation).await
    }

    async fn run_mutation<Q, V>(&self, _query: Q, vars: V) -> GqlResult<Q>
    where
        Q: cynic::QueryFragment + cynic::MutationBuilder<V>,
        V: cynic::QueryVariables + serde::Serialize,
    {
        let operation = Q::build(vars);
        self.post_operation(operation).await
    }

    async fn post_operation<Q, V>(&self, operation: cynic::Operation<Q, V>) -> GqlResult<Q>
    where
        Q: cynic::QueryFragment,
        V: cynic::QueryVariables,
        cynic::Operation<Q, V>: serde::Serialize,
    {
        let body = serde_json::to_value(&operation).map_err(|e| GqlError::Decode(e.to_string()))?;
        let resp = self.http.post(&self.endpoint).json(&body).send().await?;
        let val: serde_json::Value = resp.json().await?;
        if let Some(errs) = val.get("errors") {
            if !errs.is_null() {
                return Err(GqlError::Graphql(errs.to_string()));
            }
        }
        let data = val
            .get("data")
            .ok_or_else(|| GqlError::Graphql("no data".into()))?;
        serde_json::from_value(data.clone()).map_err(|e| GqlError::Decode(e.to_string()))
    }

    pub async fn fetch_all(&self, date: &str) -> GqlResult<FetchAll> {
        let mut vars = FetchAllVars::default();
        vars.plan_filters = Some(TodoDayPlanFilterInput {
            planDate: Some(TextFilterInput {
                eq: Some(date.into()),
            }),
        });
        self.run(FetchAll {}, vars).await
    }

    pub async fn daily_review(&self, date: &str) -> GqlResult<DailyReview> {
        let vars = DailyReviewVars { date: date.into() };
        self.run(DailyReviewQuery {}, vars)
            .await
            .map(|q| q.daily_review)
    }

    pub async fn todo_events(&self, todo_id: i64) -> GqlResult<Vec<TodoEvent>> {
        let query = TodoEventsQuery {};
        let vars = TodoEventsVars { todo_id };
        let res: TodoEventsResult = self.run(query, vars).await?;
        Ok(res.todo_event.nodes)
    }

    pub async fn create_todo(&self, title: &str) -> GqlResult<Todo> {
        let vars = CreateTodoVars {
            title: title.into(),
            desc: None,
        };
        self.run_mutation(CreateTodoMut {}, vars)
            .await
            .map(|m| m.create_todo)
    }

    pub async fn update_todo(&self, id: i64, title: Option<&str>) -> GqlResult<Todo> {
        let vars = UpdateTodoVars {
            id,
            title: title.map(Into::into),
            desc: None,
        };
        self.run_mutation(UpdateTodoMut {}, vars)
            .await
            .map(|m| m.update_todo)
    }

    pub async fn set_status(
        &self,
        id: i64,
        status: &str,
        blocked: Option<&str>,
    ) -> GqlResult<Todo> {
        let vars = SetStatusVars {
            id,
            status: match status {
                "started" => TodoStatusEnum::Started,
                "blocked" => TodoStatusEnum::Blocked,
                "done" => TodoStatusEnum::Done,
                "cancelled" => TodoStatusEnum::Cancelled,
                _ => TodoStatusEnum::Todo,
            },
            blocked: blocked.map(Into::into),
        };
        self.run_mutation(SetStatusMut {}, vars)
            .await
            .map(|m| m.set_todo_status)
    }

    pub async fn plan_today(&self, todo_id: i64) -> GqlResult<()> {
        let vars = PlanTodayVars { todo_id };
        self.run_mutation(PlanTodayMut {}, vars).await?;
        Ok(())
    }

    pub async fn unplan_today(&self, todo_id: i64) -> GqlResult<()> {
        let vars = UnplanVars { todo_id };
        self.run_mutation(UnplanMut {}, vars).await?;
        Ok(())
    }

    pub async fn reorder(&self, date: &str, ids: &[i64]) -> GqlResult<()> {
        let vars = ReorderVars {
            date: date.into(),
            todo_ids: ids.to_vec(),
        };
        self.run_mutation(ReorderMut {}, vars).await?;
        Ok(())
    }

    pub async fn carry_over(&self, from: &str, to: &str) -> GqlResult<()> {
        let vars = CarryOverVars {
            from: from.into(),
            to: to.into(),
        };
        self.run_mutation(CarryOverMut {}, vars).await?;
        Ok(())
    }

    pub async fn add_tag(&self, todo_id: i64, slug: &str) -> GqlResult<()> {
        let vars = TagVars {
            todo_id,
            slug: slug.into(),
        };
        self.run_mutation(AddTagMut {}, vars).await?;
        Ok(())
    }

    pub async fn remove_tag(&self, todo_id: i64, slug: &str) -> GqlResult<()> {
        let vars = TagVars {
            todo_id,
            slug: slug.into(),
        };
        self.run_mutation(RemoveTagMut {}, vars).await?;
        Ok(())
    }

    pub async fn todo_from_pr(&self, pr_id: i64, plan_today: bool) -> GqlResult<Todo> {
        let vars = TodoFromPrVars { pr_id, plan_today };
        self.run_mutation(TodoFromPrMut {}, vars)
            .await
            .map(|m| m.todo_from_pull_request)
    }

    pub async fn dismiss_pr(&self, id: i64) -> GqlResult<()> {
        let vars = DismissPrVars { id };
        self.run_mutation(DismissPrMut {}, vars).await?;
        Ok(())
    }

    pub async fn todo_from_linear(&self, issue_id: i64, plan_today: bool) -> GqlResult<Todo> {
        let vars = TodoFromLinearVars {
            issue_id,
            plan_today,
        };
        self.run_mutation(TodoFromLinearMut {}, vars)
            .await
            .map(|m| m.todo_from_linear_issue)
    }

    pub async fn sync_github(&self) -> GqlResult<SyncState> {
        self.run_mutation(SyncGithubMut {}, ())
            .await
            .map(|m| m.sync_github)
    }

    pub async fn sync_linear(&self) -> GqlResult<SyncState> {
        self.run_mutation(SyncLinearMut {}, ())
            .await
            .map(|m| m.sync_linear)
    }
}

// ---- Inline query types for todo events ----

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "Query", variables = "TodoEventsVars")]
pub struct TodoEventsQuery {
    #[arguments(filters: $filters, orderBy: $order)]
    pub todo_event: TodoEventConnection,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct TodoEventsVars {
    pub filters: Option<TodoEventFilterInput>,
    pub order: Option<TodoEventOrderInput>,
}

type TodoEventsResult = TodoEventsQuery;

impl TodoEventsVars {
    fn new(todo_id: i64) -> Self {
        Self {
            filters: Some(TodoEventFilterInput {
                todo_id: Some(IntegerFilterInput { eq: Some(todo_id) }),
            }),
            order: Some(TodoEventOrderInput {
                occurred_at: Some(OrderByEnum::Asc),
            }),
        }
    }
}
