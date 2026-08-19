// Backlog view: all todos, search/filter, grouped by status. Same TodoRow
// component. High information density, calm.

import { useMemo, useState } from "react"
import { Search } from "lucide-react"
import { Input } from "@/components/ui/input"
import { TodoRow } from "@/components/todo-row"
import { EmptyState, LoadingRow, ErrorBanner } from "@/components/primitives"
import { useTodos } from "@/hooks/use-data"
import { STATUS_ORDER, statusMeta } from "@/lib/status"
import type { Todo, TodoStatus } from "@/types"

export function BacklogView() {
  const todos = useTodos()
  const [query, setQuery] = useState("")
  const [dismissedErr, setDismissedErr] = useState(false)

  const filtered = useMemo(() => {
    const all = todos.data ?? []
    if (!query.trim()) return all
    const q = query.toLowerCase()
    return all.filter(
      (t) =>
        t.title.toLowerCase().includes(q) ||
        (t.description ?? "").toLowerCase().includes(q),
    )
  }, [todos.data, query])

  // Group by status.
  const grouped = useMemo(() => {
    const g: Record<TodoStatus, Todo[]> = {
      todo: [],
      started: [],
      blocked: [],
      done: [],
      cancelled: [],
    }
    for (const t of filtered) g[t.status].push(t)
    return g
  }, [filtered])

  const error = todos.error && !dismissedErr ? String((todos.error as Error).message) : null

  return (
    <div className="mx-auto max-w-3xl px-6 py-8">
      <h1 className="mb-4 text-lg font-semibold">Backlog</h1>

      {/* Search */}
      <div className="relative mb-6">
        <Search className="absolute left-2.5 top-2.5 size-4 text-muted-foreground" />
        <Input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search todos…"
          className="pl-9"
        />
      </div>

      {error && <ErrorBanner message={error} onDismiss={() => setDismissedErr(true)} />}

      {todos.isLoading && <LoadingRow label="Loading todos…" />}

      {!todos.isLoading && !error && filtered.length === 0 && (
        <EmptyState
          message={query ? "No todos match the search." : "No todos yet."}
          hint={query ? "Try a different search term." : "Create a todo from the Today view."}
        />
      )}

      {filtered.length > 0 && (
        <div className="space-y-6">
          {STATUS_ORDER.map((status) => {
            const items = grouped[status]
            if (items.length === 0) return null
            const m = statusMeta(status)
            return (
              <section key={status}>
                <div className="mb-2 flex items-center gap-2">
                  <span className={`font-mono text-sm ${m.color}`}>{m.glyph}</span>
                  <h2 className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
                    {m.label}
                  </h2>
                  <span className="text-xs text-muted-foreground/50">{items.length}</span>
                </div>
                <div className="space-y-0.5">
                  {items.map((t) => (
                    <TodoRow key={t.id} todo={t} compact />
                  ))}
                </div>
              </section>
            )
          })}
        </div>
      )}
    </div>
  )
}
