// Status glyphs and semantic colors. Color is ALWAYS a secondary cue — pair
// every color with a distinct glyph/shape. Color-blind safe: status is
// distinguishable without red/green alone.

import type { PullRequest, TodoStatus } from "@/types"

export interface StatusMeta {
  glyph: string
  label: string
  /** Tailwind text color class (secondary cue only). */
  color: string
  /** True when the todo is "closed" (done/cancelled). */
  closed: boolean
}

export const STATUS_META: Record<TodoStatus, StatusMeta> = {
  todo: { glyph: "○", label: "To do", color: "text-muted-foreground", closed: false },
  started: { glyph: "◐", label: "Started", color: "text-info", closed: false },
  blocked: { glyph: "⊘", label: "Blocked", color: "text-warning", closed: false },
  done: { glyph: "✓", label: "Done", color: "text-success", closed: true },
  cancelled: { glyph: "✕", label: "Cancelled", color: "text-muted-foreground", closed: true },
}

export const STATUS_ORDER: TodoStatus[] = ["todo", "started", "blocked", "done", "cancelled"]

export function statusMeta(s: TodoStatus): StatusMeta {
  return STATUS_META[s]
}

// PR state → glyph + color.
export interface PrStateMeta {
  glyph: string
  label: string
  color: string
}

export const PR_STATE_META: Record<PullRequest["state"], PrStateMeta> = {
  open: { glyph: "○", label: "Open", color: "text-success" },
  closed: { glyph: "✕", label: "Closed", color: "text-muted-foreground" },
  merged: { glyph: "◐", label: "Merged", color: "text-purple" },
  draft: { glyph: "◌", label: "Draft", color: "text-muted-foreground" },
}

// PR review-status flags → lucide icon name + semantic color.
export interface PrReviewStatusMeta {
  icon: "AlertCircle" | "Bot" | "GitMerge"
  label: string
  color: string
}

export const PR_REVIEW_STATUS_META = {
  changesRequested: { icon: "AlertCircle", label: "Changes requested", color: "text-warning" } as PrReviewStatusMeta,
  copilotComments: { icon: "Bot", label: "Copilot comments", color: "text-info" } as PrReviewStatusMeta,
  mergeConflicts: { icon: "GitMerge", label: "Merge conflicts", color: "text-destructive" } as PrReviewStatusMeta,
} as const

// Linear stateType → glyph + color.
export interface LinearStateMeta {
  glyph: string
  label: string
  color: string
}

export const LINEAR_STATE_META: Record<string, LinearStateMeta> = {
  started: { glyph: "◐", label: "Started", color: "text-info" },
  completed: { glyph: "✓", label: "Completed", color: "text-success" },
  canceled: { glyph: "✕", label: "Canceled", color: "text-muted-foreground" },
  backlog: { glyph: "○", label: "Backlog", color: "text-muted-foreground" },
}

// Actor glyph.
export const ACTOR_META: Record<string, { glyph: string; color: string }> = {
  user: { glyph: "●", color: "text-info" },
  sync: { glyph: "↻", color: "text-muted-foreground" },
  system: { glyph: "•", color: "text-muted-foreground" },
}

// Relation indicator.
export const RELATION_META: Record<string, { glyph: string; label: string }> = {
  reviews: { glyph: "→", label: "reviews" },
  implements: { glyph: "⊃", label: "implements" },
  references: { glyph: "↗", label: "references" },
}

/** Human-readable event kind. */
export function eventLabel(kind: string): string {
  const map: Record<string, string> = {
    created: "Created",
    title_changed: "Title changed",
    description_changed: "Description changed",
    status_changed: "Status changed",
    blocked: "Blocked",
    unblocked: "Unblocked",
    tag_added: "Tag added",
    tag_removed: "Tag removed",
    linked_pr: "Linked pull request",
    linked_linear: "Linked Linear issue",
    planned: "Planned for today",
    unplanned: "Unplanned",
    carried_over: "Carried over",
  }
  return map[kind] ?? kind
}

/** Relative time: "2h ago", "3d ago", or absolute date if old. */
export function relTime(iso: string): string {
  const t = new Date(iso).getTime()
  if (isNaN(t)) return iso
  const diff = Date.now() - t
  const min = Math.floor(diff / 60000)
  if (min < 1) return "just now"
  if (min < 60) return `${min}m ago`
  const hr = Math.floor(min / 60)
  if (hr < 24) return `${hr}h ago`
  const day = Math.floor(hr / 24)
  if (day < 7) return `${day}d ago`
  return new Date(iso).toLocaleDateString("en-US", { month: "short", day: "numeric" })
}

/** Absolute time: "08:19" or "Aug 19, 08:19". */
export function absTime(iso: string, includeDate = false): string {
  const d = new Date(iso)
  const time = d.toLocaleTimeString("en-US", { hour: "2-digit", minute: "2-digit", hour12: false })
  if (includeDate) {
    const date = d.toLocaleDateString("en-US", { month: "short", day: "numeric" })
    return `${date}, ${time}`
  }
  return time
}
