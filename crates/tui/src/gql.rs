//! GraphQL client: type-safe queries and mutations over the backend's `/`.
//!
//! Built with cynic 3 against the shared `api/schema.graphql` (introspected from
//! the live seaography server). The schema is regenerated via the
//! `introspect-schema` binary; `build.rs` feeds it to cynic's codegen so every
//! query struct is compile-checked against the SDL.
//!
//! Cynic's schema module generates zero-field marker structs for input types
//! and enums. We define our own `InputObject`/`Enum` structs that derive cynic
//! traits, so we can construct them with real field values.

/// The schema module — cynic generates all schema types inside this.
#[cynic::schema("preflight")]
pub mod schema {}

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

// ---- Response types (cynic QueryFragments — compile-checked against SDL) ----

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "Todo")]
pub struct Todo {
    pub id: i32,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub blocked_reason: Option<String>,
    pub sort_key: i32,
    pub created_at: String,
    pub started_at: Option<String>,
    pub closed_at: Option<String>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "TodoDayPlan")]
pub struct PlanRow {
    pub id: i32,
    pub position: i32,
    pub carried_over: bool,
    pub removed_at: Option<String>,
    pub todo: Option<Todo>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "PullRequest")]
pub struct PullRequest {
    pub id: i32,
    pub owner: String,
    pub repo: String,
    pub number: i32,
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
    pub id: i32,
    pub identifier: String,
    pub title: String,
    pub url: String,
    pub state_name: String,
    pub state_type: String,
    pub priority: Option<i32>,
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
    pub id: i32,
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

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "TodoConnection")]
pub struct TodoConnection {
    pub nodes: Vec<Todo>,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
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
#[cynic(graphql_type = "DailyReview")]
pub struct DailyReview {
    pub date: String,
    pub planned: Vec<Todo>,
    pub touched: Vec<Todo>,
    pub completed: Vec<Todo>,
    pub carried_over: Vec<Todo>,
}

// ---- Query root fragments ----

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

// ---- Custom input objects (cynic schema markers are zero-field) ----

#[derive(cynic::InputObject, Clone, Debug)]
#[cynic(graphql_type = "TodoFilterInput")]
pub struct TodoFilterInput {
    pub id: Option<IntegerFilterInput>,
}

#[derive(cynic::InputObject, Clone, Debug)]
#[cynic(graphql_type = "TodoDayPlanFilterInput")]
pub struct TodoDayPlanFilterInput {
    pub plan_date: Option<TextFilterInput>,
}

#[derive(cynic::InputObject, Clone, Debug)]
#[cynic(graphql_type = "TodoDayPlanOrderInput")]
pub struct TodoDayPlanOrderInput {
    pub position: Option<OrderBy>,
}

#[derive(cynic::InputObject, Clone, Debug)]
#[cynic(graphql_type = "PullRequestFilterInput")]
pub struct PullRequestFilterInput {
    pub id: Option<IntegerFilterInput>,
}

#[derive(cynic::InputObject, Clone, Debug)]
#[cynic(graphql_type = "LinearIssueFilterInput")]
pub struct LinearIssueFilterInput {
    pub id: Option<IntegerFilterInput>,
}

#[derive(cynic::InputObject, Clone, Debug)]
#[cynic(graphql_type = "TodoEventFilterInput")]
pub struct TodoEventFilterInput {
    pub todo_id: Option<IntegerFilterInput>,
}

#[derive(cynic::InputObject, Clone, Debug)]
#[cynic(graphql_type = "TodoEventOrderInput")]
pub struct TodoEventOrderInput {
    pub occurred_at: Option<OrderBy>,
}

#[derive(cynic::InputObject, Clone, Debug)]
#[cynic(graphql_type = "TextFilterInput")]
pub struct TextFilterInput {
    pub eq: Option<String>,
}

#[derive(cynic::InputObject, Clone, Debug)]
#[cynic(graphql_type = "IntegerFilterInput")]
pub struct IntegerFilterInput {
    pub eq: Option<i32>,
}

#[derive(cynic::Enum, Clone, Debug)]
#[cynic(graphql_type = "OrderByEnum")]
pub enum OrderBy {
    #[cynic(rename = "ASC")]
    Asc,
    #[cynic(rename = "DESC")]
    Desc,
}

// ---- Mutation root fragments ----

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
    pub id: i32,
    pub title: Option<String>,
    pub desc: Option<String>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "Mutation", variables = "SetStatusVars")]
pub struct SetStatusMut {
    #[arguments(id: $id, status: $status, blockedReason: $blocked)]
    pub set_todo_status: Todo,
}

#[derive(cynic::Enum, Clone, Debug)]
#[cynic(graphql_type = "TodoStatusEnum")]
pub enum TodoStatus {
    #[cynic(rename = "todo")]
    Todo,
    #[cynic(rename = "started")]
    Started,
    #[cynic(rename = "blocked")]
    Blocked,
    #[cynic(rename = "done")]
    Done,
    #[cynic(rename = "cancelled")]
    Cancelled,
}
#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct SetStatusVars {
    pub id: i32,
    pub status: TodoStatus,
    pub blocked: Option<String>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "Mutation", variables = "PlanTodayVars")]
pub struct PlanTodayMut {
    #[arguments(todoId: $todo_id)]
    pub plan_for_today: TodoDayPlanId,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "TodoDayPlan")]
pub struct TodoDayPlanId {
    pub id: i32,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct PlanTodayVars {
    pub todo_id: i32,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "Mutation", variables = "UnplanVars")]
pub struct UnplanMut {
    #[arguments(todoId: $todo_id)]
    pub unplan_for_today: bool,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct UnplanVars {
    pub todo_id: i32,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "Mutation", variables = "ReorderVars")]
pub struct ReorderMut {
    #[arguments(date: $date, todoIds: $todo_ids)]
    pub reorder_day_plan: Vec<TodoDayPlanId>,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct ReorderVars {
    pub date: String,
    pub todo_ids: Vec<i32>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "Mutation", variables = "CarryOverVars")]
pub struct CarryOverMut {
    #[arguments(from: $from, to: $to)]
    pub carry_over_unfinished: Vec<TodoDayPlanId>,
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
    pub todo_id: i32,
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
    pub pr_id: i32,
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
    pub id: i32,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(graphql_type = "Mutation", variables = "TodoFromLinearVars")]
pub struct TodoFromLinearMut {
    #[arguments(linearIssueId: $issue_id, planToday: $plan_today)]
    pub todo_from_linear_issue: Todo,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct TodoFromLinearVars {
    pub issue_id: i32,
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

    async fn post_operation<Q, V>(&self, operation: cynic::Operation<Q, V>) -> GqlResult<Q>
    where
        Q: cynic::QueryFragment + serde::de::DeserializeOwned,
        V: cynic::QueryVariables + serde::Serialize,
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
        let vars = FetchAllVars {
            filters: None,
            plan_filters: Some(TodoDayPlanFilterInput {
                plan_date: Some(TextFilterInput {
                    eq: Some(date.into()),
                }),
            }),
            plan_order: Some(TodoDayPlanOrderInput {
                position: Some(OrderBy::Asc),
            }),
            pr_filters: None,
            linear_filters: None,
        };
        let operation = cynic::QueryBuilder::build(vars);
        self.post_operation(operation).await
    }

    pub async fn daily_review(&self, date: &str) -> GqlResult<DailyReview> {
        let vars = DailyReviewVars { date: date.into() };
        let operation = cynic::QueryBuilder::build(vars);
        self.post_operation::<DailyReviewQuery, _>(operation)
            .await
            .map(|q| q.daily_review)
    }

    pub async fn todo_events(&self, todo_id: i32) -> GqlResult<Vec<TodoEvent>> {
        let vars = TodoEventsVars {
            filters: Some(TodoEventFilterInput {
                todo_id: Some(IntegerFilterInput { eq: Some(todo_id) }),
            }),
            order: Some(TodoEventOrderInput {
                occurred_at: Some(OrderBy::Asc),
            }),
        };
        let operation = cynic::QueryBuilder::build(vars);
        self.post_operation::<TodoEventsQuery, _>(operation)
            .await
            .map(|q| q.todo_event.nodes)
    }

    pub async fn create_todo(&self, title: &str) -> GqlResult<Todo> {
        let vars = CreateTodoVars {
            title: title.into(),
            desc: None,
        };
        let operation = cynic::MutationBuilder::build(vars);
        self.post_operation::<CreateTodoMut, _>(operation)
            .await
            .map(|m| m.create_todo)
    }

    pub async fn update_todo(&self, id: i32, title: Option<&str>) -> GqlResult<Todo> {
        let vars = UpdateTodoVars {
            id,
            title: title.map(Into::into),
            desc: None,
        };
        let operation = cynic::MutationBuilder::build(vars);
        self.post_operation::<UpdateTodoMut, _>(operation)
            .await
            .map(|m| m.update_todo)
    }

    pub async fn set_status(
        &self,
        id: i32,
        status: &str,
        blocked: Option<&str>,
    ) -> GqlResult<Todo> {
        let vars = SetStatusVars {
            id,
            status: match status {
                "started" => TodoStatus::Started,
                "blocked" => TodoStatus::Blocked,
                "done" => TodoStatus::Done,
                "cancelled" => TodoStatus::Cancelled,
                _ => TodoStatus::Todo,
            },
            blocked: blocked.map(Into::into),
        };
        let operation = cynic::MutationBuilder::build(vars);
        self.post_operation::<SetStatusMut, _>(operation)
            .await
            .map(|m| m.set_todo_status)
    }

    pub async fn plan_today(&self, todo_id: i32) -> GqlResult<()> {
        let vars = PlanTodayVars { todo_id };
        let operation = cynic::MutationBuilder::build(vars);
        self.post_operation::<PlanTodayMut, _>(operation).await?;
        Ok(())
    }

    pub async fn unplan_today(&self, todo_id: i32) -> GqlResult<()> {
        let vars = UnplanVars { todo_id };
        let operation = cynic::MutationBuilder::build(vars);
        self.post_operation::<UnplanMut, _>(operation).await?;
        Ok(())
    }

    pub async fn reorder(&self, date: &str, ids: &[i32]) -> GqlResult<()> {
        let vars = ReorderVars {
            date: date.into(),
            todo_ids: ids.to_vec(),
        };
        let operation = cynic::MutationBuilder::build(vars);
        self.post_operation::<ReorderMut, _>(operation).await?;
        Ok(())
    }

    pub async fn carry_over(&self, from: &str, to: &str) -> GqlResult<()> {
        let vars = CarryOverVars {
            from: from.into(),
            to: to.into(),
        };
        let operation = cynic::MutationBuilder::build(vars);
        self.post_operation::<CarryOverMut, _>(operation).await?;
        Ok(())
    }

    pub async fn add_tag(&self, todo_id: i32, slug: &str) -> GqlResult<()> {
        let vars = TagVars {
            todo_id,
            slug: slug.into(),
        };
        let operation = cynic::MutationBuilder::build(vars);
        self.post_operation::<AddTagMut, _>(operation).await?;
        Ok(())
    }

    pub async fn remove_tag(&self, todo_id: i32, slug: &str) -> GqlResult<()> {
        let vars = TagVars {
            todo_id,
            slug: slug.into(),
        };
        let operation = cynic::MutationBuilder::build(vars);
        self.post_operation::<RemoveTagMut, _>(operation).await?;
        Ok(())
    }

    pub async fn todo_from_pr(&self, pr_id: i32, plan_today: bool) -> GqlResult<Todo> {
        let vars = TodoFromPrVars { pr_id, plan_today };
        let operation = cynic::MutationBuilder::build(vars);
        self.post_operation::<TodoFromPrMut, _>(operation)
            .await
            .map(|m| m.todo_from_pull_request)
    }

    pub async fn dismiss_pr(&self, id: i32) -> GqlResult<()> {
        let vars = DismissPrVars { id };
        let operation = cynic::MutationBuilder::build(vars);
        self.post_operation::<DismissPrMut, _>(operation).await?;
        Ok(())
    }

    pub async fn todo_from_linear(&self, issue_id: i32, plan_today: bool) -> GqlResult<Todo> {
        let vars = TodoFromLinearVars {
            issue_id,
            plan_today,
        };
        let operation = cynic::MutationBuilder::build(vars);
        self.post_operation::<TodoFromLinearMut, _>(operation)
            .await
            .map(|m| m.todo_from_linear_issue)
    }

    pub async fn sync_github(&self) -> GqlResult<SyncState> {
        let operation = cynic::MutationBuilder::build(());
        self.post_operation::<SyncGithubMut, _>(operation)
            .await
            .map(|m| m.sync_github)
    }

    pub async fn sync_linear(&self) -> GqlResult<SyncState> {
        let operation = cynic::MutationBuilder::build(());
        self.post_operation::<SyncLinearMut, _>(operation)
            .await
            .map(|m| m.sync_linear)
    }
}
