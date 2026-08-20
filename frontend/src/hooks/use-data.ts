// Data hooks over TanStack Query. Each hook wraps one GraphQL query/mutation.
// All degrade gracefully — callers get { data, isLoading, error } and render
// empty/loading/error states from those.

import {
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query"
import { useCallback } from "react"
import {
  mutateAddTag,
  mutateCarryOver,
  mutateCreateTodo,
  mutateDismissPullRequest,
  mutateLinkLinearIssue,
  mutateLinkPullRequest,
  mutatePlanForToday,
  mutateReorderDayPlan,
  mutateRemoveTag,
  mutateSetTodoStatus,
  mutateSyncGithub,
  mutateSyncLinear,
  mutateTodoFromLinearIssue,
  mutateTodoFromPullRequest,
  mutateUnlinkLinearIssue,
  mutateUnlinkPullRequest,
  mutateUnplanForToday,
  mutateUpdateTodo,
  queryClock,
  queryDailyReview,
  queryDayPlan,
  queryLinearIssues,
  queryPullRequests,
  querySyncStates,
  queryTodoEvents,
  queryTodoLinearIssues,
  queryTodoPullRequests,
  queryTodoTags,
  queryTodos,
} from "@/lib/graphql"
import type { LinkRelation, TodoStatus } from "@/types"

// Query keys.
export const qk = {
  todos: ["todos"] as const,
  dayPlan: (date: string) => ["dayPlan", date] as const,
  pulls: ["pulls"] as const,
  linears: ["linears"] as const,
  sync: ["sync"] as const,
  clock: ["clock"] as const,
  review: (date: string) => ["review", date] as const,
  todoEvents: (id: number) => ["todoEvents", id] as const,
  todoTags: (id: number) => ["todoTags", id] as const,
  todoPulls: (id: number) => ["todoPulls", id] as const,
  todoLinears: (id: number) => ["todoLinears", id] as const,
}

// ---- Queries ----

export function useTodos() {
  return useQuery({ queryKey: qk.todos, queryFn: queryTodos })
}

export function useDayPlan(date: string) {
  return useQuery({
    queryKey: qk.dayPlan(date),
    queryFn: () => queryDayPlan(date),
    enabled: date !== "",
  })
}

export function usePullRequests() {
  return useQuery({ queryKey: qk.pulls, queryFn: queryPullRequests })
}

export function useLinearIssues() {
  return useQuery({ queryKey: qk.linears, queryFn: queryLinearIssues })
}

export function useSyncStates() {
  return useQuery({ queryKey: qk.sync, queryFn: querySyncStates })
}

export function useClock() {
  return useQuery({ queryKey: qk.clock, queryFn: queryClock, staleTime: 60_000 })
}

export function useDailyReview(date: string) {
  return useQuery({
    queryKey: qk.review(date),
    queryFn: () => queryDailyReview(date),
    enabled: date !== "",
  })
}

export function useTodoEvents(todoId: number) {
  return useQuery({ queryKey: qk.todoEvents(todoId), queryFn: () => queryTodoEvents(todoId) })
}

export function useTodoTags(todoId: number) {
  return useQuery({ queryKey: qk.todoTags(todoId), queryFn: () => queryTodoTags(todoId) })
}

export function useTodoPullRequests(todoId: number) {
  return useQuery({ queryKey: qk.todoPulls(todoId), queryFn: () => queryTodoPullRequests(todoId) })
}

export function useTodoLinearIssues(todoId: number) {
  return useQuery({ queryKey: qk.todoLinears(todoId), queryFn: () => queryTodoLinearIssues(todoId) })
}

// ---- Mutations ----

/** Invalidate everything that shows todos (today, backlog, review, detail). */
function useInvalidateAll() {
  const qc = useQueryClient()
  return useCallback(() => {
    qc.invalidateQueries({ queryKey: ["todos"] })
    qc.invalidateQueries({ queryKey: ["dayPlan"] })
    qc.invalidateQueries({ queryKey: ["review"] })
    qc.invalidateQueries({ queryKey: ["todoEvents"] })
    qc.invalidateQueries({ queryKey: ["todoTags"] })
    qc.invalidateQueries({ queryKey: ["todoPulls"] })
    qc.invalidateQueries({ queryKey: ["todoLinears"] })
  }, [qc])
}

export function useCreateTodo() {
  const inv = useInvalidateAll()
  return useMutation({
    mutationFn: ({ title, description }: { title: string; description?: string }) =>
      mutateCreateTodo(title, description),
    onSuccess: () => inv(),
  })
}

export function useUpdateTodo() {
  const inv = useInvalidateAll()
  return useMutation({
    mutationFn: ({ id, title, description }: { id: number; title?: string; description?: string }) =>
      mutateUpdateTodo(id, title, description),
    onSuccess: () => inv(),
  })
}

export function useSetTodoStatus() {
  const inv = useInvalidateAll()
  return useMutation({
    mutationFn: ({ id, status, blockedReason }: { id: number; status: TodoStatus; blockedReason?: string }) =>
      mutateSetTodoStatus(id, status, blockedReason),
    onSuccess: () => inv(),
  })
}

export function usePlanForToday() {
  const inv = useInvalidateAll()
  return useMutation({
    mutationFn: ({ todoId }: { todoId: number }) => mutatePlanForToday(todoId),
    onSuccess: () => inv(),
  })
}

export function useUnplanForToday() {
  const inv = useInvalidateAll()
  return useMutation({
    mutationFn: ({ todoId }: { todoId: number }) => mutateUnplanForToday(todoId),
    onSuccess: () => inv(),
  })
}

export function useReorderDayPlan() {
  const inv = useInvalidateAll()
  return useMutation({
    mutationFn: ({ date, todoIds }: { date: string; todoIds: number[] }) =>
      mutateReorderDayPlan(date, todoIds),
    onSuccess: () => inv(),
  })
}

export function useCarryOver() {
  const inv = useInvalidateAll()
  return useMutation({
    mutationFn: ({ from, to }: { from: string; to: string }) => mutateCarryOver(from, to),
    onSuccess: () => inv(),
  })
}

export function useAddTag() {
  const inv = useInvalidateAll()
  return useMutation({
    mutationFn: ({ todoId, slug }: { todoId: number; slug: string }) => mutateAddTag(todoId, slug),
    onSuccess: () => inv(),
  })
}

export function useRemoveTag() {
  const inv = useInvalidateAll()
  return useMutation({
    mutationFn: ({ todoId, slug }: { todoId: number; slug: string }) => mutateRemoveTag(todoId, slug),
    onSuccess: () => inv(),
  })
}
export function useTodoFromPullRequest() {
  const inv = useInvalidateAll()
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ pullRequestId, planToday }: { pullRequestId: number; planToday: boolean }) =>
      mutateTodoFromPullRequest(pullRequestId, planToday),
    onSuccess: () => {
      inv()
      qc.invalidateQueries({ queryKey: ["pulls"] })
    },
  })
}
export function useLinkPullRequest() {
  const inv = useInvalidateAll()
  return useMutation({
    mutationFn: ({ todoId, pullRequestId, relation }: { todoId: number; pullRequestId: number; relation: LinkRelation }) =>
      mutateLinkPullRequest(todoId, pullRequestId, relation),
    onSuccess: () => inv(),
  })
}

export function useUnlinkPullRequest() {
  const inv = useInvalidateAll()
  return useMutation({
    mutationFn: ({ todoId, pullRequestId }: { todoId: number; pullRequestId: number }) =>
      mutateUnlinkPullRequest(todoId, pullRequestId),
    onSuccess: () => inv(),
  })
}

export function useDismissPullRequest() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ id }: { id: number }) => mutateDismissPullRequest(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["pulls"] })
    },
  })
}

export function useTodoFromLinearIssue() {
  const inv = useInvalidateAll()
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ linearIssueId, planToday }: { linearIssueId: number; planToday: boolean }) =>
      mutateTodoFromLinearIssue(linearIssueId, planToday),
    onSuccess: () => {
      inv()
      qc.invalidateQueries({ queryKey: ["linears"] })
    },
  })
}

export function useLinkLinearIssue() {
  const inv = useInvalidateAll()
  return useMutation({
    mutationFn: ({ todoId, linearIssueId }: { todoId: number; linearIssueId: number }) =>
      mutateLinkLinearIssue(todoId, linearIssueId),
    onSuccess: () => inv(),
  })
}

export function useUnlinkLinearIssue() {
  const inv = useInvalidateAll()
  return useMutation({
    mutationFn: ({ todoId, linearIssueId }: { todoId: number; linearIssueId: number }) =>
      mutateUnlinkLinearIssue(todoId, linearIssueId),
    onSuccess: () => inv(),
  })
}

export function useSyncGithub() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: mutateSyncGithub,
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["sync"] })
      qc.invalidateQueries({ queryKey: ["pulls"] })
    },
  })
}

export function useSyncLinear() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: mutateSyncLinear,
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["sync"] })
      qc.invalidateQueries({ queryKey: ["linears"] })
    },
  })
}
