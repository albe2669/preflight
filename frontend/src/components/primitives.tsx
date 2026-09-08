import { AlertCircle, Bot, GitMerge, Inbox, Loader2, Check, AlertTriangle } from "lucide-react"
import { Badge } from "@/components/ui/badge"
import { cn } from "@/lib/utils"
import { statusMeta, RELATION_META, PR_REVIEW_STATUS_META } from "@/lib/status"
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
  changesRequested,
  copilotComments,
  mergeConflicts,
}: {
  owner: string
  repo: string
  number: number
  relation?: string
  url?: string
  changesRequested?: boolean
  copilotComments?: boolean
  mergeConflicts?: boolean
}) {
  const rel = relation ? RELATION_META[relation] : null
  const content = (
    <span className="font-mono text-[11px] text-muted-foreground">
      {owner}/{repo}#{number}
      {rel && <span className="ml-1 text-info">{rel.glyph}</span>}
      {(changesRequested || copilotComments || mergeConflicts) && (
        <PrStatusIcons
          changesRequested={changesRequested}
          copilotComments={copilotComments}
          mergeConflicts={mergeConflicts}
          className="ml-1 inline-flex"
        />
      )}
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

const PR_STATUS_ICONS = {
  changesRequested: { Icon: AlertCircle, ...PR_REVIEW_STATUS_META.changesRequested },
  copilotComments: { Icon: Bot, ...PR_REVIEW_STATUS_META.copilotComments },
  mergeConflicts: { Icon: GitMerge, ...PR_REVIEW_STATUS_META.mergeConflicts },
  approved: { Icon: Check, ...PR_REVIEW_STATUS_META.approved },
  actionsFailing: { Icon: AlertTriangle, ...PR_REVIEW_STATUS_META.actionsFailing },
} as const

export function PrStatusIcons({
  changesRequested,
  copilotComments,
  mergeConflicts,
  approved,
  actionsFailing,
  className,
}: {
  changesRequested?: boolean
  copilotComments?: boolean
  mergeConflicts?: boolean
  approved?: boolean
  actionsFailing?: boolean
  className?: string
}) {
  return (
    <span className={cn("inline-flex items-center gap-0.5", className)}>
      {changesRequested && <PR_STATUS_ICONS.changesRequested.Icon className={cn("size-3", PR_STATUS_ICONS.changesRequested.color)} />}
      {copilotComments && <PR_STATUS_ICONS.copilotComments.Icon className={cn("size-3", PR_STATUS_ICONS.copilotComments.color)} />}
      {mergeConflicts && <PR_STATUS_ICONS.mergeConflicts.Icon className={cn("size-3", PR_STATUS_ICONS.mergeConflicts.color)} />}
      {approved && <PR_STATUS_ICONS.approved.Icon className={cn("size-3", PR_STATUS_ICONS.approved.color)} />}
      {actionsFailing && <PR_STATUS_ICONS.actionsFailing.Icon className={cn("size-3", PR_STATUS_ICONS.actionsFailing.color)} />}
    </span>
  )
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
