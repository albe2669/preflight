// Todo detail: full description, status, blocked reason, tags, linked PRs
// (with relation) and issues, and the event timeline (vertical, time-ordered,
// actor distinguished user/sync/system).

import { useParams, Link } from "react-router-dom"
import { useState } from "react"
import { toast } from "sonner"
import { ArrowLeft, Plus, X, Link2, Search } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Textarea } from "@/components/ui/textarea"
import { Badge } from "@/components/ui/badge"
import { Separator } from "@/components/ui/separator"
import { cn } from "@/lib/utils"
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { StatusGlyph, PrBadge, LinearBadge, EmptyState, LoadingRow, ErrorBanner } from "@/components/primitives"
import {
  useTodos,
  useTodoTags,
  useTodoPullRequests,
  useTodoLinearIssues,
  useTodoEvents,
  useUpdateTodo,
  useSetTodoStatus,
  useAddTag,
  useRemoveTag,
  usePullRequests,
  useLinkPullRequest,
  useUnlinkPullRequest,
} from "@/hooks/use-data"
import { ACTOR_META, eventLabel, absTime, PR_STATE_META } from "@/lib/status"
import { STATUS_ORDER } from "@/lib/status"
import type { LinkRelation, TodoStatus } from "@/types"

export function TodoDetailView() {
  const { id } = useParams<{ id: string }>()
  const todoId = Number(id)

  const todos = useTodos()
  const tags = useTodoTags(todoId)
  const pulls = useTodoPullRequests(todoId)
  const linears = useTodoLinearIssues(todoId)
  const events = useTodoEvents(todoId)

  const updateTodo = useUpdateTodo()
  const setStatus = useSetTodoStatus()
  const addTag = useAddTag()
  const removeTag = useRemoveTag()
  const allPulls = usePullRequests()
  const linkPr = useLinkPullRequest()
  const unlinkPr = useUnlinkPullRequest()

  const [editing, setEditing] = useState(false)
  const [title, setTitle] = useState("")
  const [description, setDescription] = useState("")
  const [blockedReason, setBlockedReason] = useState("")
  const [newTag, setNewTag] = useState("")
  const [dismissedErr, setDismissedErr] = useState(false)
  const [linkPrOpen, setLinkPrOpen] = useState(false)
  const [prSearch, setPrSearch] = useState("")
  const [linkRelation, setLinkRelation] = useState<LinkRelation>("references")

  const todo = todos.data?.find((t) => t.id === todoId)

  // Start editing with current values.
  const startEdit = () => {
    if (!todo) return
    setTitle(todo.title)
    setDescription(todo.description ?? "")
    setBlockedReason(todo.blockedReason ?? "")
    setEditing(true)
  }

  const saveEdit = async () => {
    try {
      await updateTodo.mutateAsync({ id: todoId, title, description })
      if (todo?.status === "blocked" && blockedReason !== (todo.blockedReason ?? "")) {
        await setStatus.mutateAsync({ id: todoId, status: "blocked", blockedReason })
      }
      setEditing(false)
      toast.success("Saved")
    } catch {
      toast.error("Could not save")
    }
  }

  const handleStatusChange = async (status: TodoStatus) => {
    try {
      await setStatus.mutateAsync({ id: todoId, status })
      toast.success("Status updated")
    } catch {
      toast.error("Could not update status")
    }
  }

  const handleAddTag = async () => {
    const slug = newTag.trim().toLowerCase()
    if (!slug) return
    try {
      setNewTag("")
      await addTag.mutateAsync({ todoId, slug })
      toast.success("Tag added")
    } catch {
      toast.error("Could not add tag")
    }
  }

  const handleRemoveTag = async (slug: string) => {
    try {
      await removeTag.mutateAsync({ todoId, slug })
    } catch {
      toast.error("Could not remove tag")
    }
  }

  const handleLinkPr = async (pullRequestId: number) => {
    try {
      await linkPr.mutateAsync({ todoId, pullRequestId, relation: linkRelation })
      toast.success("PR linked")
      setLinkPrOpen(false)
      setPrSearch("")
    } catch {
      toast.error("Could not link PR")
    }
  }

  const handleUnlinkPr = async (pullRequestId: number) => {
    try {
      await unlinkPr.mutateAsync({ todoId, pullRequestId })
      toast.success("PR unlinked")
    } catch {
      toast.error("Could not unlink PR")
    }
  }

  const linkedPrIds = new Set(pulls.data?.map((tp) => tp.pullRequestId) ?? [])
  const filteredPrs = (allPulls.data ?? [])
    .filter((p) => !linkedPrIds.has(p.id))
    .filter((p) => {
      if (!prSearch) return true
      const q = prSearch.toLowerCase()
      return p.title.toLowerCase().includes(q) || `${p.owner}/${p.repo}#${p.number}`.toLowerCase().includes(q)
    })

  if (todos.isLoading) return <LoadingRow label="Loading todo…" />
  if (todos.error && !dismissedErr)
    return <ErrorBanner message={String((todos.error as Error).message)} onDismiss={() => setDismissedErr(true)} />
  if (!todo)
    return (
      <div className="mx-auto max-w-2xl px-6 py-8">
        <Link to="/backlog" className="flex items-center gap-2 text-sm text-muted-foreground hover:text-foreground">
          <ArrowLeft className="size-4" /> Back
        </Link>
        <EmptyState message="Todo not found." hint="It may have been deleted." />
      </div>
    )

  return (
    <div className="mx-auto max-w-2xl px-6 py-8">
      <Link to="/backlog" className="mb-4 flex items-center gap-2 text-sm text-muted-foreground hover:text-foreground">
        <ArrowLeft className="size-4" /> Back
      </Link>

      {/* Status + title */}
      <div className="flex items-start gap-3">
        <span className="mt-1">
          <StatusGlyph status={todo.status} />
        </span>
        <div className="min-w-0 flex-1">
          {editing ? (
            <Input
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              className="text-base"
            />
          ) : (
            <h1 className={
              (todo.status === "done" || todo.status === "cancelled")
                ? "text-base font-medium text-muted-foreground line-through"
                : "text-base font-medium"
            }>
              {todo.title}
            </h1>
          )}
        </div>
      </div>

      {/* Status selector */}
      <div className="mt-4 flex items-center gap-3">
        <Select value={todo.status} onValueChange={(v) => handleStatusChange(v as TodoStatus)}>
          <SelectTrigger className="w-36 text-sm">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {STATUS_ORDER.map((s) => (
              <SelectItem key={s} value={s}>
                <span className="flex items-center gap-2">
                  <StatusGlyph status={s} className="text-sm" />
                  <span className="capitalize">{s}</span>
                </span>
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <Button variant="ghost" size="sm" onClick={editing ? saveEdit : startEdit} disabled={updateTodo.isPending}>
          {editing ? "Save" : "Edit"}
        </Button>
        {editing && (
          <Button variant="ghost" size="sm" onClick={() => setEditing(false)}>
            Cancel
          </Button>
        )}
      </div>

      {/* Description */}
      <div className="mt-4">
        {editing ? (
          <Textarea
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            placeholder="Add a description…"
            className="min-h-24"
          />
        ) : todo.description ? (
          <p className="whitespace-pre-wrap text-sm text-muted-foreground">{todo.description}</p>
        ) : (
          <p className="text-sm text-muted-foreground/40">No description.</p>
        )}
      </div>

      {/* Blocked reason */}
      {todo.status === "blocked" && (
        <div className="mt-4">
          {editing ? (
            <Input
              value={blockedReason}
              onChange={(e) => setBlockedReason(e.target.value)}
              placeholder="Why is this blocked?"
              className="text-warning"
            />
          ) : todo.blockedReason ? (
            <p className="text-sm text-warning">{todo.blockedReason}</p>
          ) : null}
        </div>
      )}

      {/* Tags */}
      <div className="mt-4">
        <div className="flex items-center gap-2">
          {tags.data?.map((tt) =>
            tt.tag ? (
              <Badge key={tt.tagId} variant="secondary" className="gap-1 font-mono text-[11px]">
                {tt.tag.slug}
                <button onClick={() => handleRemoveTag(tt.tag!.slug)} className="text-muted-foreground hover:text-foreground">
                  <X className="size-2.5" />
                </button>
              </Badge>
            ) : null,
          )}
          <div className="flex items-center gap-1">
            <Input
              value={newTag}
              onChange={(e) => setNewTag(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleAddTag()}
              placeholder="tag"
              className="h-6 w-20 font-mono text-[11px]"
            />
            <Button variant="ghost" size="icon" onClick={handleAddTag} className="size-6">
              <Plus className="size-3" />
            </Button>
          </div>
        </div>
      </div>

      <Separator className="my-6" />

      {/* Linked PRs */}
      <section className="mb-6">
        <div className="mb-2 flex items-center gap-2">
          <h2 className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
            Linked pull requests
          </h2>
          <Dialog open={linkPrOpen} onOpenChange={setLinkPrOpen}>
            <DialogTrigger asChild>
              <Button variant="ghost" size="sm" className="h-5 text-xs text-muted-foreground">
                <Link2 className="size-3" />Link PR
              </Button>
            </DialogTrigger>
            <DialogContent className="max-w-md">
              <DialogHeader>
                <DialogTitle>Link pull request</DialogTitle>
              </DialogHeader>
              <Select value={linkRelation} onValueChange={(v) => setLinkRelation(v as LinkRelation)}>
                <SelectTrigger className="w-full text-sm">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="references">references</SelectItem>
                  <SelectItem value="reviews">reviews</SelectItem>
                  <SelectItem value="implements">implements</SelectItem>
                </SelectContent>
              </Select>
              <div className="relative">
                <Search className="absolute left-2 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
                <Input
                  value={prSearch}
                  onChange={(e) => setPrSearch(e.target.value)}
                  placeholder="Search PRs…"
                  className="pl-7 text-sm"
                />
              </div>
              <div className="max-h-64 space-y-0.5 overflow-y-auto">
                {allPulls.isLoading && <LoadingRow label="Loading PRs…" />}
                {filteredPrs.length === 0 && !allPulls.isLoading && (
                  <p className="py-4 text-center text-sm text-muted-foreground/40">
                    {linkedPrIds.size === 0 && !prSearch ? "No PRs available." : "No matches."}
                  </p>
                )}
                {filteredPrs.map((p) => {
                  const meta = PR_STATE_META[p.state]
                  return (
                    <button
                      key={p.id}
                      onClick={() => handleLinkPr(p.id)}
                      disabled={linkPr.isPending}
                      className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left hover:bg-secondary/40 disabled:opacity-50"
                    >
                      <span className={cn("font-mono text-sm", meta.color)}>{meta.glyph}</span>
                      <span className="min-w-0 flex-1">
                        <span className="block truncate text-sm">{p.title}</span>
                        <span className="block font-mono text-[11px] text-muted-foreground">
                          {p.owner}/{p.repo}#{p.number}
                        </span>
                      </span>
                    </button>
                  )
                })}
              </div>
            </DialogContent>
          </Dialog>
        </div>
        {pulls.data && pulls.data.length > 0 ? (
          <div className="space-y-1">
            {pulls.data.map((tp) =>
              tp.pullRequest ? (
                <div key={tp.pullRequestId} className="flex items-center gap-1">
                  <PrBadge
                    owner={tp.pullRequest.owner}
                    repo={tp.pullRequest.repo}
                    number={tp.pullRequest.number}
                    relation={tp.relation}
                    url={tp.pullRequest.url}
                    changesRequested={tp.pullRequest.changesRequested}
                    copilotComments={tp.pullRequest.copilotComments}
                    mergeConflicts={tp.pullRequest.mergeConflicts}
                  />
                  <button
                    onClick={() => handleUnlinkPr(tp.pullRequestId)}
                    className="text-muted-foreground hover:text-foreground"
                  >
                    <X className="size-3" />
                  </button>
                </div>
              ) : null,
            )}
          </div>
        ) : (
          <p className="text-sm text-muted-foreground/40">None.</p>
        )}
      </section>

      {/* Linked Linear issues */}
      <section className="mb-6">
        <h2 className="mb-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
          Linked Linear issues
        </h2>
        {linears.data && linears.data.length > 0 ? (
          <div className="space-y-1">
            {linears.data.map((tl) =>
              tl.linearIssue ? (
                <div key={tl.linearIssueId} className="flex items-center gap-2">
                  <LinearBadge identifier={tl.linearIssue.identifier} url={tl.linearIssue.url} />
                </div>
              ) : null,
            )}
          </div>
        ) : (
          <p className="text-sm text-muted-foreground/40">None.</p>
        )}
      </section>

      <Separator className="my-6" />

      {/* Event timeline */}
      <section>
        <h2 className="mb-3 text-xs font-medium uppercase tracking-wide text-muted-foreground">
          Timeline
        </h2>
        {events.isLoading && <LoadingRow label="Loading events…" />}
        {events.data && events.data.length === 0 && (
          <EmptyState message="No events yet." />
        )}
        {events.data && events.data.length > 0 && (
          <div className="space-y-0">
            {events.data.map((ev, i) => {
              const actor = ACTOR_META[ev.actor] ?? ACTOR_META.system
              return (
                <div key={ev.id} className="flex gap-3 py-2">
                  {/* Timeline rail */}
                  <div className="flex flex-col items-center">
                    <span className={`font-mono text-xs ${actor.color}`}>{actor.glyph}</span>
                    {i < events.data!.length - 1 && (
                      <span className="mt-1 w-px flex-1 bg-border" />
                    )}
                  </div>
                  <div className="min-w-0 flex-1 pb-2">
                    <div className="flex items-center gap-2">
                      <span className="text-sm">{eventLabel(ev.kind)}</span>
                      {ev.field && (
                        <span className="font-mono text-[11px] text-muted-foreground">
                          {ev.field}
                        </span>
                      )}
                    </div>
                    {(ev.oldValue || ev.newValue) && (
                      <div className="mt-0.5 font-mono text-[11px] text-muted-foreground">
                        {ev.oldValue && <span className="line-through">{ev.oldValue}</span>}
                        {ev.oldValue && ev.newValue && <span> → </span>}
                        {ev.newValue && <span>{ev.newValue}</span>}
                      </div>
                    )}
                    <div className="mt-0.5 flex items-center gap-2 text-[11px] text-muted-foreground/60">
                      <span className="capitalize">{ev.actor}</span>
                      <span>·</span>
                      <span>{absTime(ev.occurredAt, true)}</span>
                    </div>
                  </div>
                </div>
              )
            })}
          </div>
        )}
      </section>
    </div>
  )
}
