/**
 * Thin typed GraphQL client. POSTs to the preflight backend.
 *
 * Quirk: the server does NOT support multiple root Query/Mutation fields in
 * one operation, so each call fires exactly one root field. `gql()` builds a
 * single-field document; `request()` returns that one field's data.
 */
import type {
  Clock,
  Connection,
  DailyReview,
  LinearIssue,
  PlanRow,
  PullRequest,
  SyncState,
  Tag,
  Todo,
  TodoDayPlan,
  TodoLinearIssue,
  TodoPullRequest,
  TodoStatus,
  LinkRelation,
  OrderBy,
} from "../types";
import { preferredEndpoint } from "./helpers";


export class GraphqlError extends Error {
  constructor(message: string, readonly errors?: unknown) {
    super(message);
    this.name = "GraphqlError";
  }
}

interface GraphQLResponse<T> {
  data?: T;
  errors?: Array<{ message: string; [k: string]: unknown }>;
}

/** Fire one root-field operation. Returns `data[fieldName]`. */
async function request<T>(query: string, variables?: Record<string, unknown>): Promise<T> {
  const res = await fetch(preferredEndpoint(), {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ query, variables }),
  });
  if (!res.ok) throw new GraphqlError(`backend HTTP ${res.status}`);
  const body = (await res.json()) as GraphQLResponse<T>;
  if (body.errors && body.errors.length) {
    throw new GraphqlError(body.errors.map((e) => e.message).join("; "), body.errors);
  }
  if (body.data == null) throw new GraphqlError("no data in response");
  return body.data as T;
}

/** Is an error a network/unreachable failure (backend down)? */
export function isUnreachable(e: unknown): boolean {
  if (e instanceof GraphqlError && /HTTP 5\d\d/.test(e.message)) return true;
  if (e instanceof TypeError) return true; // fetch throws TypeError on connection refused
  return false;
}

// ---- Fragment strings (selections reused across queries) ----

const TODO_FIELDS = `
  id title description status blockedReason sortKey
  createdAt updatedAt startedAt closedAt
`;

const PLAN_FIELDS = `
  id planDate todoId position carriedOver addedAt removedAt
  todo { ${TODO_FIELDS} }
`;

const TAG_FIELDS = `id slug name color createdAt`;

// ---- Queries: one root field per request ----

export async function fetchTodayPlan(planDate: string): Promise<PlanRow[]> {
  const data = await request<{ todoDayPlan: Connection<TodoDayPlan> }>(
    `query($filters: TodoDayPlanFilterInput, $orderBy: TodoDayPlanOrderInput) {
      todoDayPlan(filters: $filters, orderBy: $orderBy) {
        nodes { ${PLAN_FIELDS} }
      }
    }`,
    { filters: { planDate: { eq: planDate } }, orderBy: { position: "ASC" as OrderBy } },
  );
  const rows = data.todoDayPlan.nodes;
  // Fetch tags and links per todo. The backend fires each as its own request;
  // Promise.all runs them in parallel.
  const enriched = await Promise.all(
    rows.map(async (r) => {
      if (!r.todo) return { ...r, tags: [], pullRequests: [], linearIssues: [] };
      const [tags, prs, linears] = await Promise.all([
        fetchTodoTags(r.todoId),
        fetchTodoPullRequests(r.todoId),
        fetchTodoLinearIssues(r.todoId),
      ]);
      return { ...r, tags, pullRequests: prs, linearIssues: linears };
    }),
  );
  return enriched;
}

export async function fetchTodos(): Promise<Todo[]> {
  const data = await request<{ todo: Connection<Todo> }>(`{ todo { nodes { ${TODO_FIELDS} } } }`);
  return data.todo.nodes;
}

export async function fetchTodoTags(todoId: number): Promise<Tag[]> {
  const data = await request<{ todoTag: Connection<{ tag: Tag | null }> }>(
    `query($filters: TodoTagFilterInput) {
      todoTag(filters: $filters) { nodes { tag { ${TAG_FIELDS} } } }
    }`,
    { filters: { todoId: { eq: todoId } } },
  );
  return data.todoTag.nodes.map((n) => n.tag).filter((t): t is Tag => t != null);
}

export async function fetchTodoPullRequests(todoId: number): Promise<TodoPullRequest[]> {
  const data = await request<{ todoPullRequest: Connection<TodoPullRequest> }>(
    `query($filters: TodoPullRequestFilterInput) {
      todoPullRequest(filters: $filters) {
        nodes { todoId pullRequestId relation createdAt
          pullRequest { id provider owner repo number title url author state reviewRequested authoredByMe syncedAt dismissedAt }
        }
      }
    }`,
    { filters: { todoId: { eq: todoId } } },
  );
  return data.todoPullRequest.nodes;
}

export async function fetchTodoLinearIssues(todoId: number): Promise<TodoLinearIssue[]> {
  const data = await request<{ todoLinearIssue: Connection<TodoLinearIssue> }>(
    `query($filters: TodoLinearIssueFilterInput) {
      todoLinearIssue(filters: $filters) {
        nodes { todoId linearIssueId createdAt
          linearIssue { id linearId identifier title description url stateName stateType priority teamKey assigneeName assignedToMe syncedAt dismissedAt }
        }
      }
    }`,
    { filters: { todoId: { eq: todoId } } },
  );
  return data.todoLinearIssue.nodes;
}

export async function fetchPullRequests(): Promise<PullRequest[]> {
  const data = await request<{ pullRequest: Connection<PullRequest> }>(
    `{ pullRequest { nodes {
      id provider owner repo number title url author state
      reviewRequested authoredByMe approved actionsFailing remoteCreatedAt remoteUpdatedAt syncedAt dismissedAt
    } } }`,
  );
  return data.pullRequest.nodes;
}

export async function fetchLinearIssues(): Promise<LinearIssue[]> {
  const data = await request<{ linearIssue: Connection<LinearIssue> }>(
    `{ linearIssue { nodes {
      id linearId identifier title description url stateName stateType priority teamKey
      assigneeName assignedToMe remoteCreatedAt remoteUpdatedAt syncedAt dismissedAt
    } } }`,
  );
  return data.linearIssue.nodes;
}

export async function fetchSyncStates(): Promise<SyncState[]> {
  const data = await request<{ syncState: Connection<SyncState> }>(
    `{ syncState { nodes { source cursor lastSyncedAt lastStatus lastError } } }`,
  );
  return data.syncState.nodes;
}

/** Fetch the server clock: authoritative logicalDate, timezone, dayStartHour. */
export async function fetchClock(): Promise<Clock> {
  const data = await request<{ clock: Clock }>(`{ clock { logicalDate timezone dayStartHour } }`);
  return data.clock;
}

export async function fetchDailyReview(date: string): Promise<DailyReview> {
  const data = await request<{ dailyReview: DailyReview }>(
    `query($date: String!) {
      dailyReview(date: $date) {
        date
        planned { ${TODO_FIELDS} }
        touched { ${TODO_FIELDS} }
        completed { ${TODO_FIELDS} }
        carriedOver { ${TODO_FIELDS} }
      }
    }`,
    { date },
  );
  return data.dailyReview;
}

// ---- Mutations: one root field per request ----

export async function createTodo(title: string, description?: string): Promise<Todo> {
  const data = await request<{ createTodo: Todo }>(
    `mutation($title: String!, $description: String) {
      createTodo(title: $title, description: $description) { ${TODO_FIELDS} }
    }`,
    { title, description: description ?? null },
  );
  return data.createTodo;
}

export async function setTodoStatus(id: number, status: TodoStatus, blockedReason?: string): Promise<Todo> {
  const data = await request<{ setTodoStatus: Todo }>(
    `mutation($id: Int!, $status: TodoStatusEnum!, $blockedReason: String) {
      setTodoStatus(id: $id, status: $status, blockedReason: $blockedReason) { ${TODO_FIELDS} }
    }`,
    { id, status, blockedReason: blockedReason ?? null },
  );
  return data.setTodoStatus;
}

export async function planForToday(todoId: number): Promise<TodoDayPlan> {
  const data = await request<{ planForToday: TodoDayPlan }>(
    `mutation($todoId: Int!) {
      planForToday(todoId: $todoId) { id planDate todoId position carriedOver addedAt removedAt }
    }`,
    { todoId },
  );
  return data.planForToday;
}

export async function unplanForToday(todoId: number): Promise<boolean> {
  const data = await request<{ unplanForToday: boolean }>(
    `mutation($todoId: Int!) { unplanForToday(todoId: $todoId) }`,
    { todoId },
  );
  return data.unplanForToday;
}

export async function reorderDayPlan(date: string, todoIds: number[]): Promise<TodoDayPlan[]> {
  const data = await request<{ reorderDayPlan: TodoDayPlan[] }>(
    `mutation($date: String!, $todoIds: [Int!]!) {
      reorderDayPlan(date: $date, todoIds: $todoIds) { id planDate todoId position carriedOver addedAt }
    }`,
    { date, todoIds },
  );
  return data.reorderDayPlan;
}

export async function addTag(todoId: number, slug: string): Promise<Todo> {
  const data = await request<{ addTag: Todo }>(
    `mutation($todoId: Int!, $slug: String!) {
      addTag(todoId: $todoId, slug: $slug) { ${TODO_FIELDS} }
    }`,
    { todoId, slug },
  );
  return data.addTag;
}

export async function todoFromPullRequest(pullRequestId: number, planToday: boolean): Promise<Todo> {
  const data = await request<{ todoFromPullRequest: Todo }>(
    `mutation($pullRequestId: Int!, $planToday: Boolean!) {
      todoFromPullRequest(pullRequestId: $pullRequestId, planToday: $planToday) { ${TODO_FIELDS} }
    }`,
    { pullRequestId, planToday },
  );
  return data.todoFromPullRequest;
}

export async function linkPullRequest(todoId: number, pullRequestId: number, relation: LinkRelation): Promise<Todo> {
  const data = await request<{ linkPullRequest: Todo }>(
    `mutation($todoId: Int!, $pullRequestId: Int!, $relation: LinkRelationEnum!) {
      linkPullRequest(todoId: $todoId, pullRequestId: $pullRequestId, relation: $relation) { ${TODO_FIELDS} }
    }`,
    { todoId, pullRequestId, relation },
  );
  return data.linkPullRequest;
}

export async function dismissPullRequest(id: number): Promise<PullRequest> {
  const data = await request<{ dismissPullRequest: PullRequest }>(
    `mutation($id: Int!) {
      dismissPullRequest(id: $id) {
        id provider owner repo number title url author state reviewRequested authoredByMe syncedAt dismissedAt
      }
    }`,
    { id },
  );
  return data.dismissPullRequest;
}

export async function todoFromLinearIssue(linearIssueId: number, planToday: boolean): Promise<Todo> {
  const data = await request<{ todoFromLinearIssue: Todo }>(
    `mutation($linearIssueId: Int!, $planToday: Boolean!) {
      todoFromLinearIssue(linearIssueId: $linearIssueId, planToday: $planToday) { ${TODO_FIELDS} }
    }`,
    { linearIssueId, planToday },
  );
  return data.todoFromLinearIssue;
}

export async function linkLinearIssue(todoId: number, linearIssueId: number): Promise<Todo> {
  const data = await request<{ linkLinearIssue: Todo }>(
    `mutation($todoId: Int!, $linearIssueId: Int!) {
      linkLinearIssue(todoId: $todoId, linearIssueId: $linearIssueId) { ${TODO_FIELDS} }
    }`,
    { todoId, linearIssueId },
  );
  return data.linkLinearIssue;
}

export async function syncLinear(): Promise<SyncState> {
  const data = await request<{ syncLinear: SyncState }>(
    `mutation { syncLinear { source cursor lastSyncedAt lastStatus lastError } }`,
  );
  return data.syncLinear;
}

export async function syncGithub(): Promise<SyncState> {
  const data = await request<{ syncGithub: SyncState }>(
    `mutation { syncGithub { source cursor lastSyncedAt lastStatus lastError } }`,
  );
  return data.syncGithub;
}
