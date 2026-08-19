// Typed GraphQL client. POSTs to /graphql (Vite proxy → 127.0.0.1:8000).
// The server does NOT support multiple root fields in one operation, so each
// request fires exactly one root field.
import type {
  Connection,
  DailyReview,
  LinearIssue,
  LinkRelation,
  OrderBy,
  PullRequest,
  SyncState,
  Todo,
  TodoDayPlan,
  TodoDayPlanFilterInput,
  TodoDayPlanOrderInput,
  TodoEvent,
  TodoEventFilterInput,
  TodoEventOrderInput,
  TodoLinearIssue,
  TodoPullRequest,
  TodoPullRequestFilterInput,
  TodoLinearIssueFilterInput,
  TodoStatus,
  TodoTag,
  TodoTagFilterInput,
} from "@/types"

const ENDPOINT = "/graphql"

export class GraphQLError extends Error {
  errors: unknown[]
  constructor(errors: unknown[]) {
    super("GraphQL errors")
    this.name = "GraphQLError"
    this.errors = errors
  }
}

export class GraphQLNetworkError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "GraphQLNetworkError"
  }
}

/** Raw request: send one operation, return the `data` object. */
async function request<T>(
  query: string,
  variables: Record<string, unknown> = {},
  operationName?: string,
): Promise<T> {
  let resp: Response
  try {
    resp = await fetch(ENDPOINT, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ query, variables, operationName }),
    })
  } catch {
    // Network failure: backend down or unreachable. Let callers degrade.
    throw new GraphQLNetworkError("Cannot reach the backend.")
  }

  if (!resp.ok) {
    throw new GraphQLNetworkError(`HTTP ${resp.status}`)
  }

  const body = await resp.json()
  if (body.errors && body.errors.length > 0) {
    throw new GraphQLError(body.errors)
  }
  return body.data as T
}

// ---- Fragments ----

const TODO_FIELDS = `
  id title description status blockedReason sortKey
  createdAt updatedAt startedAt closedAt
`

const TAG_FIELDS = `id slug name color createdAt`

const TODO_TAG_FIELDS = `todoId tagId createdAt tag { ${TAG_FIELDS} }`

const PLAN_FIELDS = `
  id planDate todoId position carriedOver addedAt removedAt
  todo { ${TODO_FIELDS} }
`

const EVENT_FIELDS = `
  id todoId kind field oldValue newValue actor occurredAt logicalDate
`

const PR_FIELDS = `
  id provider owner repo number title url author state
  reviewRequested authoredByMe
  remoteCreatedAt remoteUpdatedAt syncedAt dismissedAt
`

const LINEAR_FIELDS = `
  id linearId identifier title description url
  stateName stateType priority teamKey assigneeName assignedToMe
  remoteCreatedAt remoteUpdatedAt syncedAt dismissedAt
`

const TODO_PR_FIELDS = `
  todoId pullRequestId relation createdAt
  pullRequest { ${PR_FIELDS} }
`

const TODO_LINEAR_FIELDS = `
  todoId linearIssueId createdAt
  linearIssue { ${LINEAR_FIELDS} }
`

const SYNC_FIELDS = `source cursor lastSyncedAt lastStatus lastError`

// ---- Queries (one root field each) ----

export async function queryTodos(): Promise<Todo[]> {
  const data = await request<{ todo: Connection<Todo> }>(
    `query Todos { todo { nodes { ${TODO_FIELDS} } } }`,
  )
  return data.todo.nodes
}

export async function queryDayPlan(
  date: string,
): Promise<TodoDayPlan[]> {
  const data = await request<{ todoDayPlan: Connection<TodoDayPlan> }>(
    `query DayPlan($filters: TodoDayPlanFilterInput!, $order: TodoDayPlanOrderInput!) {
      todoDayPlan(filters: $filters, orderBy: $order) { nodes { ${PLAN_FIELDS} } }
    }`,
    {
      filters: { planDate: { eq: date } } satisfies TodoDayPlanFilterInput,
      order: { position: "ASC" } satisfies TodoDayPlanOrderInput,
    },
    "DayPlan",
  )
  return data.todoDayPlan.nodes
}

export async function queryPullRequests(): Promise<PullRequest[]> {
  const data = await request<{ pullRequest: Connection<PullRequest> }>(
    `query Pulls { pullRequest { nodes { ${PR_FIELDS} } } }`,
  )
  return data.pullRequest.nodes
}

export async function queryLinearIssues(): Promise<LinearIssue[]> {
  const data = await request<{ linearIssue: Connection<LinearIssue> }>(
    `query Linears { linearIssue { nodes { ${LINEAR_FIELDS} } } }`,
  )
  return data.linearIssue.nodes
}

export async function querySyncStates(): Promise<SyncState[]> {
  const data = await request<{ syncState: Connection<SyncState> }>(
    `query Sync { syncState { nodes { ${SYNC_FIELDS} } } }`,
  )
  return data.syncState.nodes
}

export async function queryDailyReview(date: string): Promise<DailyReview> {
  const data = await request<{ dailyReview: DailyReview }>(
    `query DailyReview($date: String!) {
      dailyReview(date: $date) {
        date
        planned { ${TODO_FIELDS} }
        touched { ${TODO_FIELDS} }
        completed { ${TODO_FIELDS} }
        carriedOver { ${TODO_FIELDS} }
      }
    }`,
    { date },
    "DailyReview",
  )
  return data.dailyReview
}

export async function queryTodoEvents(todoId: number): Promise<TodoEvent[]> {
  const data = await request<{ todoEvent: Connection<TodoEvent> }>(
    `query TodoEvents($filters: TodoEventFilterInput!, $order: TodoEventOrderInput!) {
      todoEvent(filters: $filters, orderBy: $order) { nodes { ${EVENT_FIELDS} } }
    }`,
    {
      filters: { todoId: { eq: todoId } } satisfies TodoEventFilterInput,
      order: { occurredAt: "ASC" as OrderBy } satisfies TodoEventOrderInput,
    },
    "TodoEvents",
  )
  return data.todoEvent.nodes
}

export async function queryTodoTags(todoId: number): Promise<TodoTag[]> {
  const data = await request<{ todoTag: Connection<TodoTag> }>(
    `query TodoTags($filters: TodoTagFilterInput!) {
      todoTag(filters: $filters) { nodes { ${TODO_TAG_FIELDS} } }
    }`,
    { filters: { todoId: { eq: todoId } } satisfies TodoTagFilterInput },
    "TodoTags",
  )
  return data.todoTag.nodes
}

export async function queryTodoPullRequests(
  todoId: number,
): Promise<TodoPullRequest[]> {
  const data = await request<{ todoPullRequest: Connection<TodoPullRequest> }>(
    `query TodoPullRequests($filters: TodoPullRequestFilterInput!) {
      todoPullRequest(filters: $filters) { nodes { ${TODO_PR_FIELDS} } }
    }`,
    { filters: { todoId: { eq: todoId } } satisfies TodoPullRequestFilterInput },
    "TodoPullRequests",
  )
  return data.todoPullRequest.nodes
}

export async function queryTodoLinearIssues(
  todoId: number,
): Promise<TodoLinearIssue[]> {
  const data = await request<{ todoLinearIssue: Connection<TodoLinearIssue> }>(
    `query TodoLinearIssues($filters: TodoLinearIssueFilterInput!) {
      todoLinearIssue(filters: $filters) { nodes { ${TODO_LINEAR_FIELDS} } }
    }`,
    {
      filters: { todoId: { eq: todoId } } satisfies TodoLinearIssueFilterInput,
    },
    "TodoLinearIssues",
  )
  return data.todoLinearIssue.nodes
}

// ---- Mutations (one root field each) ----

export async function mutateCreateTodo(
  title: string,
  description?: string,
): Promise<Todo> {
  const data = await request<{ createTodo: Todo }>(
    `mutation CreateTodo($title: String!, $description: String) {
      createTodo(title: $title, description: $description) { ${TODO_FIELDS} }
    }`,
    { title, description },
    "CreateTodo",
  )
  return data.createTodo
}

export async function mutateUpdateTodo(
  id: number,
  title?: string,
  description?: string,
): Promise<Todo> {
  const data = await request<{ updateTodo: Todo }>(
    `mutation UpdateTodo($id: Int!, $title: String, $description: String) {
      updateTodo(id: $id, title: $title, description: $description) { ${TODO_FIELDS} }
    }`,
    { id, title, description },
    "UpdateTodo",
  )
  return data.updateTodo
}

export async function mutateSetTodoStatus(
  id: number,
  status: TodoStatus,
  blockedReason?: string,
): Promise<Todo> {
  const data = await request<{ setTodoStatus: Todo }>(
    `mutation SetStatus($id: Int!, $status: TodoStatusEnum!, $blockedReason: String) {
      setTodoStatus(id: $id, status: $status, blockedReason: $blockedReason) { ${TODO_FIELDS} }
    }`,
    { id, status, blockedReason },
    "SetStatus",
  )
  return data.setTodoStatus
}

export async function mutatePlanForToday(todoId: number): Promise<TodoDayPlan> {
  const data = await request<{ planForToday: TodoDayPlan }>(
    `mutation PlanToday($todoId: Int!) {
      planForToday(todoId: $todoId) { id planDate todoId position carriedOver addedAt removedAt }
    }`,
    { todoId },
    "PlanToday",
  )
  return data.planForToday
}

export async function mutateUnplanForToday(todoId: number): Promise<boolean> {
  const data = await request<{ unplanForToday: boolean }>(
    `mutation UnplanToday($todoId: Int!) {
      unplanForToday(todoId: $todoId)
    }`,
    { todoId },
    "UnplanToday",
  )
  return data.unplanForToday
}

export async function mutateReorderDayPlan(
  date: string,
  todoIds: number[],
): Promise<TodoDayPlan[]> {
  const data = await request<{ reorderDayPlan: TodoDayPlan[] }>(
    `mutation Reorder($date: String!, $todoIds: [Int!]!) {
      reorderDayPlan(date: $date, todoIds: $todoIds) {
        id planDate todoId position carriedOver addedAt removedAt
      }
    }`,
    { date, todoIds },
    "Reorder",
  )
  return data.reorderDayPlan
}

export async function mutateCarryOver(
  from: string,
  to: string,
): Promise<TodoDayPlan[]> {
  const data = await request<{ carryOverUnfinished: TodoDayPlan[] }>(
    `mutation CarryOver($from: String!, $to: String!) {
      carryOverUnfinished(from: $from, to: $to) {
        id planDate todoId position carriedOver addedAt removedAt
      }
    }`,
    { from, to },
    "CarryOver",
  )
  return data.carryOverUnfinished
}

export async function mutateAddTag(
  todoId: number,
  slug: string,
): Promise<Todo> {
  const data = await request<{ addTag: Todo }>(
    `mutation AddTag($todoId: Int!, $slug: String!) {
      addTag(todoId: $todoId, slug: $slug) { ${TODO_FIELDS} }
    }`,
    { todoId, slug },
    "AddTag",
  )
  return data.addTag
}

export async function mutateRemoveTag(
  todoId: number,
  slug: string,
): Promise<Todo> {
  const data = await request<{ removeTag: Todo }>(
    `mutation RemoveTag($todoId: Int!, $slug: String!) {
      removeTag(todoId: $todoId, slug: $slug) { ${TODO_FIELDS} }
    }`,
    { todoId, slug },
    "RemoveTag",
  )
  return data.removeTag
}

export async function mutateTodoFromPullRequest(
  pullRequestId: number,
  planToday: boolean,
): Promise<Todo> {
  const data = await request<{ todoFromPullRequest: Todo }>(
    `mutation TodoFromPr($pullRequestId: Int!, $planToday: Boolean!) {
      todoFromPullRequest(pullRequestId: $pullRequestId, planToday: $planToday) { ${TODO_FIELDS} }
    }`,
    { pullRequestId, planToday },
    "TodoFromPr",
  )
  return data.todoFromPullRequest
}

export async function mutateLinkPullRequest(
  todoId: number,
  pullRequestId: number,
  relation: LinkRelation,
): Promise<Todo> {
  const data = await request<{ linkPullRequest: Todo }>(
    `mutation LinkPr($todoId: Int!, $pullRequestId: Int!, $relation: LinkRelationEnum!) {
      linkPullRequest(todoId: $todoId, pullRequestId: $pullRequestId, relation: $relation) { ${TODO_FIELDS} }
    }`,
    { todoId, pullRequestId, relation },
    "LinkPr",
  )
  return data.linkPullRequest
}

export async function mutateUnlinkPullRequest(
  todoId: number,
  pullRequestId: number,
): Promise<Todo> {
  const data = await request<{ unlinkPullRequest: Todo }>(
    `mutation UnlinkPr($todoId: Int!, $pullRequestId: Int!) {
      unlinkPullRequest(todoId: $todoId, pullRequestId: $pullRequestId) { ${TODO_FIELDS} }
    }`,
    { todoId, pullRequestId },
    "UnlinkPr",
  )
  return data.unlinkPullRequest
}

export async function mutateDismissPullRequest(
  id: number,
): Promise<PullRequest> {
  const data = await request<{ dismissPullRequest: PullRequest }>(
    `mutation DismissPr($id: Int!) {
      dismissPullRequest(id: $id) { ${PR_FIELDS} }
    }`,
    { id },
    "DismissPr",
  )
  return data.dismissPullRequest
}

export async function mutateTodoFromLinearIssue(
  linearIssueId: number,
  planToday: boolean,
): Promise<Todo> {
  const data = await request<{ todoFromLinearIssue: Todo }>(
    `mutation TodoFromLinear($linearIssueId: Int!, $planToday: Boolean!) {
      todoFromLinearIssue(linearIssueId: $linearIssueId, planToday: $planToday) { ${TODO_FIELDS} }
    }`,
    { linearIssueId, planToday },
    "TodoFromLinear",
  )
  return data.todoFromLinearIssue
}

export async function mutateLinkLinearIssue(
  todoId: number,
  linearIssueId: number,
): Promise<Todo> {
  const data = await request<{ linkLinearIssue: Todo }>(
    `mutation LinkLinear($todoId: Int!, $linearIssueId: Int!) {
      linkLinearIssue(todoId: $todoId, linearIssueId: $linearIssueId) { ${TODO_FIELDS} }
    }`,
    { todoId, linearIssueId },
    "LinkLinear",
  )
  return data.linkLinearIssue
}

export async function mutateUnlinkLinearIssue(
  todoId: number,
  linearIssueId: number,
): Promise<Todo> {
  const data = await request<{ unlinkLinearIssue: Todo }>(
    `mutation UnlinkLinear($todoId: Int!, $linearIssueId: Int!) {
      unlinkLinearIssue(todoId: $todoId, linearIssueId: $linearIssueId) { ${TODO_FIELDS} }
    }`,
    { todoId, linearIssueId },
    "UnlinkLinear",
  )
  return data.unlinkLinearIssue
}

export async function mutateSyncGithub(): Promise<SyncState> {
  const data = await request<{ syncGithub: SyncState }>(
    `mutation SyncGithub { syncGithub { ${SYNC_FIELDS} } }`,
  )
  return data.syncGithub
}

export async function mutateSyncLinear(): Promise<SyncState> {
  const data = await request<{ syncLinear: SyncState }>(
    `mutation SyncLinear { syncLinear { ${SYNC_FIELDS} } }`,
  )
  return data.syncLinear
}

