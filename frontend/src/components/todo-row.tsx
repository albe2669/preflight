// Reusable todo row: status glyph, title, tags, link badges, blocked reason.
// Used by Today (with drag handle) and Backlog.

import { Link } from "react-router-dom"
import { GripVertical } from "lucide-react"
import { cn } from "@/lib/utils"
import { StatusGlyph, TagChip, CarriedOverMark, PrBadge, LinearBadge } from "@/components/primitives"
import { useTodoTags, useTodoPullRequests, useTodoLinearIssues } from "@/hooks/use-data"
import type { Todo } from "@/types"

export function TodoRow({
  todo,
  carriedOver,
  dragHandle,
  onClick,
  compact,
}: {
  todo: Todo
  carriedOver?: boolean
  dragHandle?: boolean
  onClick?: () => void
  compact?: boolean
}) {
  const tags = useTodoTags(todo.id)
  const pulls = useTodoPullRequests(todo.id)
  const linears = useTodoLinearIssues(todo.id)

  const isClosed = todo.status === "done" || todo.status === "cancelled"
  const isBlocked = todo.status === "blocked"

  return (
    <Link
      to={`/todo/${todo.id}`}
      onClick={onClick}
      className={cn(
        "group flex items-start gap-3 rounded-md px-3 py-2 transition-colors hover:bg-secondary/40",
        compact && "py-1.5",
      )}
    >
      {dragHandle && (
        <span className="mt-0.5 cursor-grab text-muted-foreground/40 opacity-0 group-hover:opacity-100">
          <GripVertical className="size-4" />
        </span>
      )}
      <span className="mt-0.5">
        <StatusGlyph status={todo.status} />
      </span>
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          {carriedOver && <CarriedOverMark />}
          <span
            className={cn(
              "truncate text-sm",
              isClosed && "text-muted-foreground line-through",
              isBlocked && "text-foreground",
            )}
          >
            {todo.title}
          </span>
        </div>
        {isBlocked && todo.blockedReason && (
          <p className="mt-0.5 pl-0 text-xs text-warning">{todo.blockedReason}</p>
        )}
        {/* Tags and link badges */}
        {(tags.data?.length || pulls.data?.length || linears.data?.length) ? (
          <div className="mt-1 flex flex-wrap items-center gap-1.5">
            {tags.data?.map((tt) =>
              tt.tag ? <TagChip key={tt.tagId} slug={tt.tag.slug} /> : null,
            )}
            {pulls.data?.map((tp) =>
              tp.pullRequest ? (
                <PrBadge
                  key={tp.pullRequestId}
                  owner={tp.pullRequest.owner}
                  repo={tp.pullRequest.repo}
                  number={tp.pullRequest.number}
                  relation={tp.relation}
                  url={tp.pullRequest.url}
                />
              ) : null,
            )}
            {linears.data?.map((tl) =>
              tl.linearIssue ? (
                <LinearBadge
                  key={tl.linearIssueId}
                  identifier={tl.linearIssue.identifier}
                  url={tl.linearIssue.url}
                />
              ) : null,
            )}
          </div>
        ) : null}
      </div>
    </Link>
  )
}

