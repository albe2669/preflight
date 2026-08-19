// Today view: ordered day-plan list for the logical date. Status glyphs,
// tags, link badges, blocked reason, carried-over marks. Inline create at
// the bottom. Drag to reorder. Empty-day state reads as intentional.

import { useState, useCallback } from "react"
import { toast } from "sonner"
import { Plus, ArrowDownToLine } from "lucide-react"
import { Input } from "@/components/ui/input"
import { Button } from "@/components/ui/button"
import { Separator } from "@/components/ui/separator"
import { TodoRow } from "@/components/todo-row"
import { EmptyState, LoadingRow, ErrorBanner } from "@/components/primitives"
import { useDayPlan, useCreateTodo, usePlanForToday, useReorderDayPlan, useCarryOver } from "@/hooks/use-data"
import { logicalDate, dateLabel, shiftDate } from "@/lib/date"
import { cn } from "@/lib/utils"
import type { TodoDayPlan } from "@/types"

export function TodayView() {
  const today = logicalDate()
  const yesterday = shiftDate(today, -1)
  const plan = useDayPlan(today)
  const createTodo = useCreateTodo()
  const planForToday = usePlanForToday()
  const reorder = useReorderDayPlan()
  const carryOver = useCarryOver()

  const [newTitle, setNewTitle] = useState("")
  const [dismissedErr, setDismissedErr] = useState(false)
  const [dragIndex, setDragIndex] = useState<number | null>(null)
  const [overIndex, setOverIndex] = useState<number | null>(null)
  const [ordering, setOrdering] = useState(false)


  // The plan rows are already sorted by position from the query.
  const rows = plan.data ?? []

  // Inline create: create a todo and plan it for today.
  const handleCreate = useCallback(async () => {
    const title = newTitle.trim()
    if (!title) return
    try {
      setNewTitle("")
      const todo = await createTodo.mutateAsync({ title })
      await planForToday.mutateAsync({ todoId: todo.id })
      toast.success("Added to today")
    } catch {
      toast.error("Could not create the todo")
    }
  }, [newTitle, createTodo, planForToday])

  // Carry over yesterday's unfinished work to today.
  const handleCarryOver = useCallback(async () => {
    try {
      const moved = await carryOver.mutateAsync({ from: yesterday, to: today })
      toast.success(`Carried over ${moved.length} item${moved.length === 1 ? "" : "s"}`)
    } catch {
      toast.error("Could not carry over")
    }
  }, [carryOver, yesterday, today])

  // ---- Drag reorder (HTML5 drag-and-drop) ----
  const onDragStart = (i: number) => {
    setDragIndex(i)
  }
  const onDragOver = (e: React.DragEvent, i: number) => {
    e.preventDefault()
    if (dragIndex === null || dragIndex === i) return
    setOverIndex(i)
  }
  const onDrop = async (i: number) => {
    if (dragIndex === null || dragIndex === i) {
      setDragIndex(null)
      setOverIndex(null)
      return
    }
    const reordered = [...rows]
    const [moved] = reordered.splice(dragIndex, 1)
    reordered.splice(i, 0, moved)
    const todoIds = reordered.map((r) => r.todoId)
    // Optimistic local order; send to server.
    setOrdering(true)
    try {
      await reorder.mutateAsync({ date: today, todoIds })
    } catch {
      toast.error("Could not reorder")
    } finally {
      setOrdering(false)
      setDragIndex(null)
      setOverIndex(null)
    }
  }

  const error = plan.error && !dismissedErr ? String((plan.error as Error).message) : null

  return (
    <div className="mx-auto max-w-3xl px-6 py-8">
      {/* Header */}
      <div className="mb-6 flex items-center justify-between">
        <div>
          <h1 className="text-lg font-semibold">Today</h1>
          <p className="mt-0.5 font-mono text-xs text-muted-foreground">{dateLabel(today)}</p>
        </div>
        {rows.length > 0 && (
          <Button
            variant="ghost"
            size="sm"
            onClick={handleCarryOver}
            disabled={carryOver.isPending}
            className="text-muted-foreground"
          >
            <ArrowDownToLine className="size-3.5" />
            Carry over
          </Button>
        )}
      </div>

      {error && <ErrorBanner message={error} onDismiss={() => setDismissedErr(true)} />}

      {plan.isLoading && <LoadingRow label="Loading today's plan…" />}

      {!plan.isLoading && !error && rows.length === 0 && (
        <div className="space-y-4">
          <EmptyState
            message="Today starts empty."
            hint="A new day begins with no plan. Add a todo below or carry over unfinished work from yesterday."
          />
          <div className="flex justify-center">
            <Button variant="outline" size="sm" onClick={handleCarryOver} disabled={carryOver.isPending}>
              <ArrowDownToLine className="size-3.5" />
              Carry over from yesterday
            </Button>
          </div>
        </div>
      )}

      {rows.length > 0 && (
        <div className={cn("space-y-0.5", ordering && "pointer-events-none opacity-60")}>
          {rows.map((row: TodoDayPlan, i: number) => (
            <div
              key={row.id}
              draggable
              onDragStart={() => onDragStart(i)}
              onDragOver={(e) => onDragOver(e, i)}
              onDrop={() => onDrop(i)}
              onDragEnd={() => { setDragIndex(null); setOverIndex(null) }}
              className={cn(
                "rounded-md transition-colors",
                overIndex === i && dragIndex !== null && "bg-secondary/60",
                dragIndex === i && "opacity-40",
              )}
            >
              <TodoRow todo={row.todo!} carriedOver={row.carriedOver} dragHandle />
            </div>
          ))}
        </div>
      )}

      <Separator className="my-4" />

      {/* Inline create */}
      <div className="flex items-center gap-2">
        <Plus className="size-4 text-muted-foreground" />
        <Input
          value={newTitle}
          onChange={(e) => setNewTitle(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && handleCreate()}
          placeholder="Add a todo for today…"
          className="border-transparent bg-transparent px-0 shadow-none focus-visible:border-border focus-visible:bg-secondary/30"
        />
      </div>
    </div>
  )
}
