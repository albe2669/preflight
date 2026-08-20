/**
 * TypeScript types that mirror api/schema.graphql.
 * Field names are camelCase, as in the SDL (async_graphql keeps SDL casing).
 */
export type TodoStatus = "todo" | "started" | "blocked" | "done" | "cancelled";
export type LinkRelation = "reviews" | "implements" | "references";
export type OrderBy = "ASC" | "DESC";
export type PrState = "open" | "closed" | "merged" | "draft";
export type LinearStateType = "started" | "completed" | "canceled" | "backlog";
export type SyncSource = "linear" | "github";
export type EventActor = "user" | "sync" | "system";

export interface Tag {
  id: number;
  slug: string;
  name: string;
  color: string | null;
  createdAt: string;
}

export interface Todo {
  id: number;
  title: string;
  description: string | null;
  status: TodoStatus;
  blockedReason: string | null;
  sortKey: number;
  createdAt: string;
  updatedAt: string;
  startedAt: string | null;
  closedAt: string | null;
}

export interface TodoDayPlan {
  id: number;
  planDate: string;
  todoId: number;
  position: number;
  carriedOver: boolean;
  addedAt: string;
  removedAt: string | null;
  todo: Todo | null;
}

export interface TodoEvent {
  id: number;
  todoId: number;
  kind: string;
  field: string | null;
  oldValue: string | null;
  newValue: string | null;
  actor: EventActor;
  occurredAt: string;
  logicalDate: string;
}

export interface PullRequest {
  id: number;
  provider: string;
  owner: string;
  repo: string;
  number: number;
  title: string;
  url: string;
  author: string | null;
  state: PrState;
  reviewRequested: boolean;
  authoredByMe: boolean;
  remoteCreatedAt: string | null;
  remoteUpdatedAt: string | null;
  syncedAt: string;
  dismissedAt: string | null;
}

export interface LinearIssue {
  id: number;
  linearId: string;
  identifier: string;
  title: string;
  description: string | null;
  url: string;
  stateName: string;
  stateType: LinearStateType;
  priority: number | null;
  teamKey: string | null;
  assigneeName: string | null;
  assignedToMe: boolean;
  remoteCreatedAt: string | null;
  remoteUpdatedAt: string | null;
  syncedAt: string;
  dismissedAt: string | null;
}

export interface SyncState {
  source: string;
  cursor: string | null;
  lastSyncedAt: string | null;
  lastStatus: string;
  lastError: string | null;
}

export interface TodoPullRequest {
  todoId: number;
  pullRequestId: number;
  relation: LinkRelation;
  createdAt: string;
  pullRequest: PullRequest | null;
  todo: Todo | null;
}

export interface TodoLinearIssue {
  todoId: number;
  linearIssueId: number;
  createdAt: string;
  linearIssue: LinearIssue | null;
  todo: Todo | null;
}

export interface TodoTag {
  todoId: number;
  tagId: number;
  createdAt: string;
  tag: Tag | null;
  todo: Todo | null;
}

export interface DailyReview {
  date: string;
  planned: Todo[];
  touched: Todo[];
  completed: Todo[];
  carriedOver: Todo[];
}

/** Server clock: authoritative logical date, timezone, day-start hour. */
export interface Clock {
  logicalDate: string;
  timezone: string;
  dayStartHour: number;
}

/** A row on the Today view: plan row plus its todo and tags. */
export interface PlanRow extends TodoDayPlan {
  todo: Todo | null;
  tags: Tag[];
  pullRequests: TodoPullRequest[];
  linearIssues: TodoLinearIssue[];
}

/** Connection shape returned by list root fields. */
export interface Connection<T> {
  nodes: T[];
  pageInfo: { hasPreviousPage: boolean; hasNextPage: boolean };
  paginationInfo: { pages: number; current: number; offset: number; total: number } | null;
}
