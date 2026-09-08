// Inbox view: triage surface. GitHub PRs + Linear issues grouped by source.
// Per-row convert/link/dismiss. Sync indicator per group. Review-requested
// PRs and assigned-to-me issues surface first.

import { useState } from "react"
import { toast } from "sonner"
import { GitPullRequest, ExternalLink, Eye, User, Check, X, Link2, Search, ChevronDown } from "lucide-react"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Separator } from "@/components/ui/separator"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { EmptyState, LoadingRow, ErrorBanner, PrStatusIcons } from "@/components/primitives"
import { PR_STATE_META, LINEAR_STATE_META, relTime } from "@/lib/status"
import {
  usePullRequests,
  useLinearIssues,
  useSyncStates,
  useTodos,
  useLinkPullRequest,
  useTodoFromPullRequest,
  useTodoFromLinearIssue,
  useDismissPullRequest,
  useSyncGithub,
  useSyncLinear,
} from "@/hooks/use-data"
import type { LinearIssue, PullRequest } from "@/types"
import { cn } from "@/lib/utils"

export function InboxView() {
  const pulls = usePullRequests()
  const linears = useLinearIssues()
  const sync = useSyncStates()

  const [dismissedErr, setDismissedErr] = useState(false)
  const errorSources = [pulls.error, linears.error, sync.error].filter(Boolean)
  const error = errorSources.length > 0 && !dismissedErr
    ? "Cannot load the inbox. The backend may be down."
    : null

  // Active (not dismissed) items only.
  const activePulls = (pulls.data ?? []).filter((p) => !p.dismissedAt)
  const openPulls = activePulls.filter((p) => p.state === "open")
  const inactivePulls = activePulls.filter((p) => p.state !== "open")
  const activeLinears = (linears.data ?? []).filter((l) => !l.dismissedAt)
  const dismissedPulls = (pulls.data ?? []).filter((p) => p.dismissedAt)
  const dismissedLinears = (linears.data ?? []).filter((l) => l.dismissedAt)

  const syncGithubMut = useSyncGithub()
  const syncLinearMut = useSyncLinear()

  return (
    <div className="mx-auto max-w-3xl px-6 py-8">
      <h1 className="mb-6 text-lg font-semibold">Inbox</h1>

      {error && <ErrorBanner message={error} onDismiss={() => setDismissedErr(true)} />}

      {(pulls.isLoading || linears.isLoading) && <LoadingRow label="Loading inbox…" />}

      {!pulls.isLoading && !linears.isLoading && activePulls.length === 0 && activeLinears.length === 0 && (
        <EmptyState message="Inbox is clear." hint="No pull requests or Linear issues need triage." />
      )}

      {/* GitHub PRs */}
      <InboxSection
        title="GitHub"
        icon={GitPullRequest}
        count={activePulls.length}
        onSync={() => syncGithubMut.mutateAsync().then(() => toast.success("GitHub synced")).catch(() => toast.error("Sync failed"))}
        syncing={syncGithubMut.isPending}
        syncState={sync.data?.find((s) => s.source === "github")}
      >
        {openPulls.map((pr) => (
          <PrRow key={pr.id} pr={pr} />
        ))}
        {inactivePulls.length > 0 && (
          <details className="mt-2">
            <summary className="flex cursor-pointer items-center gap-1 text-xs text-muted-foreground hover:text-foreground">
              <ChevronDown className="size-3 transition-transform [[open]>&]:rotate-90" />
              Closed / merged / draft ({inactivePulls.length})
            </summary>
            <div className="mt-1 space-y-0.5">
              {inactivePulls.map((pr) => (
                <PrRow key={pr.id} pr={pr} />
              ))}
            </div>
          </details>
        )}
      </InboxSection>

      {(dismissedPulls.length > 0 || dismissedLinears.length > 0) && (
        <details className="mt-6">
          <summary className="cursor-pointer text-xs text-muted-foreground hover:text-foreground">
            Dismissed ({dismissedPulls.length + dismissedLinears.length})
          </summary>
          <div className="mt-2 space-y-0.5">
            {dismissedPulls.map((pr) => (
              <PrRow key={pr.id} pr={pr} dismissed />
            ))}
            {dismissedLinears.map((li) => (
              <LinearRow key={li.id} issue={li} dismissed />
            ))}
          </div>
        </details>
      )}

      <Separator className="my-6" />

      {/* Linear issues */}
      <InboxSection
        title="Linear"
        icon={ExternalLink}
        count={activeLinears.length}
        onSync={() => syncLinearMut.mutateAsync().then(() => toast.success("Linear synced")).catch(() => toast.error("Sync failed"))}
        syncing={syncLinearMut.isPending}
        syncState={sync.data?.find((s) => s.source === "linear")}
      >
        {activeLinears.map((li) => (
          <LinearRow key={li.id} issue={li} />
        ))}
      </InboxSection>
    </div>
  )
}

function InboxSection({
  title,
  icon: Icon,
  count,
  children,
  onSync,
  syncing,
  syncState,
}: {
  title: string
  icon: React.ComponentType<{ className?: string }>
  count: number
  children?: React.ReactNode
  onSync: () => void
  syncing: boolean
  syncState?: { lastStatus: string; lastError: string | null; lastSyncedAt: string | null }
}) {
  const noToken = syncState?.lastStatus === "no_token" || syncState?.lastStatus === "noop"
  return (
    <section className="mb-6">
      <div className="mb-3 flex items-center gap-2">
        <Icon className="size-4 text-muted-foreground" />
        <h2 className="text-xs font-medium uppercase tracking-wide text-muted-foreground">{title}</h2>
        <span className="text-xs text-muted-foreground/50">{count}</span>
        <Button
          variant="ghost"
          size="sm"
          onClick={onSync}
          disabled={syncing || noToken}
          className="ml-auto text-xs text-muted-foreground"
        >
          {syncing ? "Syncing…" : noToken ? "No token" : "Sync"}
        </Button>
      </div>
      {noToken && (
        <p className="mb-2 text-xs text-muted-foreground/60">
          No {title} token configured. Sync is a no-op.
        </p>
      )}
      {syncState?.lastError && (
        <p className="mb-2 text-xs text-warning">{syncState.lastError}</p>
      )}
      {count === 0 && !noToken ? (
        <p className="py-3 text-sm text-muted-foreground/60">No {title} items.</p>
      ) : (
        children
      )}
    </section>
  )
}

function PrRow({ pr, dismissed }: { pr: PullRequest; dismissed?: boolean }) {
  const todoFromPr = useTodoFromPullRequest()
  const dismissPr = useDismissPullRequest()
  const linkPr = useLinkPullRequest()
  const todos = useTodos()
  const meta = PR_STATE_META[pr.state]

  const [linkOpen, setLinkOpen] = useState(false)
  const [todoSearch, setTodoSearch] = useState("")

  const handleConvert = (planToday: boolean) => {
    todoFromPr.mutate(
      { pullRequestId: pr.id, planToday },
      {
        onSuccess: () => toast.success(planToday ? "Converted and planned for today" : "Converted to todo"),
        onError: () => toast.error("Could not convert"),
      },
    )
  }

  const handleDismiss = () => {
    dismissPr.mutate({ id: pr.id }, {
      onSuccess: () => toast.success("Dismissed"),
      onError: () => toast.error("Could not dismiss"),
    })
  }

  const handleLinkTodo = async (todoId: number) => {
    try {
      await linkPr.mutateAsync({ todoId, pullRequestId: pr.id, relation: "references" })
      toast.success("PR linked to todo")
      setLinkOpen(false)
      setTodoSearch("")
    } catch {
      toast.error("Could not link PR")
    }
  }

  const filteredTodos = (todos.data ?? []).filter((t) => {
    if (!todoSearch) return true
    return t.title.toLowerCase().includes(todoSearch.toLowerCase())
  })

  return (
    <div className={cn("flex items-start gap-3 rounded-md px-3 py-2 hover:bg-secondary/30", dismissed && "opacity-50")}>
      <span className={cn("mt-0.5 font-mono text-sm", meta.color)}>{meta.glyph}</span>
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="truncate text-sm">{pr.title}</span>
          {pr.reviewRequested && <Badge variant="outline" className="gap-1 text-[10px]"><Eye className="size-2.5" />Review</Badge>}
          {pr.authoredByMe && <Badge variant="outline" className="gap-1 text-[10px]"><User className="size-2.5" />Author</Badge>}
          <PrStatusIcons
            changesRequested={pr.changesRequested}
            copilotComments={pr.copilotComments}
            mergeConflicts={pr.mergeConflicts}
            approved={pr.approved}
            actionsFailing={pr.actionsFailing}
          />
        </div>
        <div className="mt-0.5 flex items-center gap-2 font-mono text-[11px] text-muted-foreground">
          <a href={pr.url} target="_blank" rel="noreferrer" className="hover:text-foreground">
            {pr.owner}/{pr.repo}#{pr.number}
          </a>
          <span>·</span>
          <span>{meta.label}</span>
          {pr.author && (<><span>·</span><span>by {pr.author}</span></>)}
          <span>·</span>
          <span>{relTime(pr.syncedAt)}</span>
        </div>
      </div>
      {!dismissed && (
        <div className="flex items-center gap-1">
          <Dialog open={linkOpen} onOpenChange={setLinkOpen}>
            <DialogTrigger asChild>
              <Button variant="ghost" size="sm" className="text-xs text-muted-foreground">
                <Link2 className="size-3" />Link
              </Button>
            </DialogTrigger>
            <DialogContent className="max-w-md">
              <DialogHeader>
                <DialogTitle>Link to todo</DialogTitle>
              </DialogHeader>
              <div className="relative">
                <Search className="absolute left-2 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
                <Input
                  value={todoSearch}
                  onChange={(e) => setTodoSearch(e.target.value)}
                  placeholder="Search todos…"
                  className="pl-7 text-sm"
                />
              </div>
              <div className="max-h-64 space-y-0.5 overflow-y-auto">
                {todos.isLoading && <LoadingRow label="Loading todos…" />}
                {filteredTodos.length === 0 && !todos.isLoading && (
                  <p className="py-4 text-center text-sm text-muted-foreground/40">No todos found.</p>
                )}
                {filteredTodos.map((t) => (
                  <button
                    key={t.id}
                    onClick={() => handleLinkTodo(t.id)}
                    disabled={linkPr.isPending}
                    className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left hover:bg-secondary/40 disabled:opacity-50"
                  >
                    <span className="truncate text-sm">{t.title}</span>
                  </button>
                ))}
              </div>
            </DialogContent>
          </Dialog>
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="ghost" size="sm" className="text-xs">
                <Check className="size-3" />Convert
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              <DropdownMenuItem onClick={() => handleConvert(true)}>Convert + plan today</DropdownMenuItem>
              <DropdownMenuItem onClick={() => handleConvert(false)}>Convert to backlog</DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
          <Button variant="ghost" size="sm" onClick={handleDismiss} className="text-muted-foreground">
            <X className="size-3" />
          </Button>
        </div>
      )}
    </div>
  )
}

function LinearRow({ issue, dismissed }: { issue: LinearIssue; dismissed?: boolean }) {
  const todoFromLinear = useTodoFromLinearIssue()
  const meta = LINEAR_STATE_META[issue.stateType] ?? { glyph: "○", label: issue.stateType, color: "text-muted-foreground" }

  const handleConvert = (planToday: boolean) => {
    todoFromLinear.mutate(
      { linearIssueId: issue.id, planToday },
      {
        onSuccess: () => toast.success(planToday ? "Converted and planned for today" : "Converted to todo"),
        onError: () => toast.error("Could not convert"),
      },
    )
  }

  return (
    <div className={cn("flex items-start gap-3 rounded-md px-3 py-2 hover:bg-secondary/30", dismissed && "opacity-50")}>
      <span className={cn("mt-0.5 font-mono text-sm", meta.color)}>{meta.glyph}</span>
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="truncate text-sm">{issue.title}</span>
          {issue.assignedToMe && <Badge variant="outline" className="gap-1 text-[10px]"><User className="size-2.5" />Mine</Badge>}
        </div>
        <div className="mt-0.5 flex items-center gap-2 font-mono text-[11px] text-muted-foreground">
          <a href={issue.url} target="_blank" rel="noreferrer" className="hover:text-foreground">
            {issue.identifier}
          </a>
          <span>·</span>
          <span>{issue.stateName}</span>
          {issue.priority != null && (<><span>·</span><span>P{issue.priority}</span></>)}
          {issue.teamKey && (<><span>·</span><span>{issue.teamKey}</span></>)}
          <span>·</span>
          <span>{relTime(issue.syncedAt)}</span>
        </div>
      </div>
      {!dismissed && (
        <div className="flex items-center gap-1">
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="ghost" size="sm" className="text-xs">
                <Check className="size-3" />Convert
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              <DropdownMenuItem onClick={() => handleConvert(true)}>Convert + plan today</DropdownMenuItem>
              <DropdownMenuItem onClick={() => handleConvert(false)}>Convert to backlog</DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      )}
    </div>
  )
}
