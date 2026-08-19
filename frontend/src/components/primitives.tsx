// Shared small presentational components: status glyph, tag chip, link
// badge, empty state, loading skeleton, error banner.

import { AlertCircle, Inbox, Loader2 } from "lucide-react"
import { Badge } from "@/components/ui/badge"
import { cn } from "@/lib/utils"
import { statusMeta, RELATION_META } from "@/lib/status"
import type { TodoStatus } from "@/types"

// ---- Status glyph ----

export function StatusGlyph({
  status,
  className,
}: {
  status: TodoStatus
  className?: string
}) {
  const m = statusMeta(status)
  return (
    <span
      className={cn("select-none font-mono text-base leading-none", m.color, className)}
      aria-label={m.label}
      role="img"
    >
      {m.glyph}
    </span>
  )
}

// ---- Tag chip ----


export function TagChip({ slug }: { slug: string }) {
  return (
    <Badge variant="secondary" className="font-mono text-[11px] font-normal">
      {slug}
    </Badge>
  )
}

export function PrBadge({
  owner,
  repo,
  number,
  relation,
  url,
}: {
  owner: string
  repo: string
  number: number
  relation?: string
  url?: string
}) {
  const rel = relation ? RELATION_META[relation] : null
  const content = (
    <span className="font-mono text-[11px] text-muted-foreground">
      {owner}/{repo}#{number}
      {rel && <span className="ml-1 text-info">{rel.glyph}</span>}
    </span>
  )
  if (url) {
    return (
      <a href={url} target="_blank" rel="noreferrer" className="hover:text-foreground">
        {content}
      </a>
    )
  }
  return content
}

export function LinearBadge({
  identifier,
  url,
}: {
  identifier: string
  url?: string
}) {
  const content = (
    <span className="font-mono text-[11px] text-muted-foreground">
      {identifier}
    </span>
  )
  if (url) {
    return (
      <a href={url} target="_blank" rel="noreferrer" className="hover:text-foreground">
        {content}
      </a>
    )
  }
  return content
}

// ---- Empty state ----

export function EmptyState({
  message,
  hint,
  icon: Icon = Inbox,
}: {
  message: string
  hint?: string
  icon?: React.ComponentType<{ className?: string }>
}) {
  return (
    <div className="flex flex-col items-center justify-center gap-2 py-12 text-center">
      <Icon className="size-6 text-muted-foreground/50" />
      <p className="text-sm text-muted-foreground">{message}</p>
      {hint && <p className="text-xs text-muted-foreground/70">{hint}</p>}
    </div>
  )
}

// ---- Loading ----

export function LoadingRow({ label }: { label?: string }) {
  return (
    <div className="flex items-center gap-2 py-3 text-sm text-muted-foreground">
      <Loader2 className="size-4 animate-spin" />
      {label ?? "Loading…"}
    </div>
  )
}

// ---- Error banner (dismissible) ----

export function ErrorBanner({
  message,
  onDismiss,
}: {
  message: string
  onDismiss?: () => void
}) {
  return (
    <div className="flex items-start gap-2 rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm">
      <AlertCircle className="mt-0.5 size-4 shrink-0 text-destructive" />
      <p className="flex-1 text-destructive">{message}</p>
      {onDismiss && (
        <button
          onClick={onDismiss}
          className="text-xs text-muted-foreground hover:text-foreground"
        >
          Dismiss
        </button>
      )}
    </div>
  )
}

// ---- Carried-over mark ----

export function CarriedOverMark() {
  return (
    <span
      className="font-mono text-xs text-warning"
      title="Carried over from a previous day"
      role="img"
      aria-label="carried over"
    >
      ↻
    </span>
  )
}
