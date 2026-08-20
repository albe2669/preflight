// TypeScript types mirroring the preflight GraphQL schema.
// Ground truth: api/schema.graphql and local:/preflight-build-context.md.

export type TodoStatus = "todo" | "started" | "blocked" | "done" | "cancelled"

export type LinkRelation = "reviews" | "implements" | "references"

export type OrderBy = "ASC" | "DESC"

export type Actor = "user" | "sync" | "system"

export interface Todo {
  id: number
  title: string
  description: string | null
  status: TodoStatus
  blockedReason: string | null
  sortKey: number
  createdAt: string
  updatedAt: string
  startedAt: string | null
  closedAt: string | null
}

export interface Tag {
  id: number
  slug: string
  name: string
  color: string | null
  createdAt: string
}

export interface TodoTag {
  todoId: number
  tagId: number
  createdAt: string
  tag: Tag | null
}

export interface TodoDayPlan {
  id: number
  planDate: string
  todoId: number
  position: number
  carriedOver: boolean
  addedAt: string
  removedAt: string | null
  todo: Todo | null
}

export interface TodoEvent {
  id: number
  todoId: number
  kind: string
  field: string | null
  oldValue: string | null
  newValue: string | null
  actor: Actor
  occurredAt: string
  logicalDate: string
}

export interface PullRequest {
  id: number
  provider: string
  owner: string
  repo: string
  number: number
  title: string
  url: string
  author: string | null
  state: "open" | "closed" | "merged" | "draft"
  reviewRequested: boolean
  authoredByMe: boolean
  remoteCreatedAt: string | null
  remoteUpdatedAt: string | null
  syncedAt: string
  dismissedAt: string | null
}

export interface LinearIssue {
  id: number
  linearId: string
  identifier: string
  title: string
  description: string | null
  url: string
  stateName: string
  stateType: "started" | "completed" | "canceled" | "backlog"
  priority: number | null
  teamKey: string | null
  assigneeName: string | null
  assignedToMe: boolean
  remoteCreatedAt: string | null
  remoteUpdatedAt: string | null
  syncedAt: string
  dismissedAt: string | null
}

export interface TodoPullRequest {
  todoId: number
  pullRequestId: number
  relation: LinkRelation
  createdAt: string
  pullRequest: PullRequest | null
}

export interface TodoLinearIssue {
  todoId: number
  linearIssueId: number
  createdAt: string
  linearIssue: LinearIssue | null
}

export interface SyncState {
  source: "linear" | "github"
  cursor: string | null
  lastSyncedAt: string | null
  lastStatus: string
  lastError: string | null
}

export interface DailyReview {
  date: string
  planned: Todo[]
  touched: Todo[]
  completed: Todo[]
  carriedOver: Todo[]
}

/** Server clock: authoritative logical date, timezone, day-start hour. */
export interface Clock {
  logicalDate: string
  timezone: string
  dayStartHour: number
}

// Connection wrappers.
export interface Connection<T> {
  pageInfo: PageInfo
  paginationInfo: PaginationInfo | null
  nodes: T[]
}

export interface PageInfo {
  hasPreviousPage: boolean
  hasNextPage: boolean
  startCursor: string | null
  endCursor: string | null
}

export interface PaginationInfo {
  pages: number
  current: number
  offset: number
  total: number
}

// Input filters — only the shapes the UI uses.
export interface IntegerFilter {
  eq?: number
}

export interface TextFilter {
  eq?: string
}

export interface TodoDayPlanFilterInput {
  planDate?: TextFilter
}

export interface TodoDayPlanOrderInput {
  position?: OrderBy
}

export interface TodoEventFilterInput {
  todoId?: IntegerFilter
}

export interface TodoEventOrderInput {
  occurredAt?: OrderBy
}

export interface TodoPullRequestFilterInput {
  todoId?: IntegerFilter
}

export interface TodoLinearIssueFilterInput {
  todoId?: IntegerFilter
}

export interface TodoTagFilterInput {
  todoId?: IntegerFilter
}
