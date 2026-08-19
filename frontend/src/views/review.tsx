// Review view: DailyReview for a picked date. Four lists: planned, touched,
// completed, carried over. Date picker in the header. Event-log nature is
// legible — this is a review of a day, not a live list.

import { useState } from "react"
import { ChevronLeft, ChevronRight } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { TodoRow } from "@/components/todo-row"
import { EmptyState, LoadingRow, ErrorBanner } from "@/components/primitives"
import { useDailyReview } from "@/hooks/use-data"
import { logicalDate, dateLabel, shiftDate } from "@/lib/date"
import type { Todo } from "@/types"

const SECTIONS: { key: "planned" | "touched" | "completed" | "carriedOver"; label: string }[] = [
  { key: "planned", label: "Planned" },
  { key: "touched", label: "Touched" },
  { key: "completed", label: "Completed" },
  { key: "carriedOver", label: "Carried over" },
]

export function ReviewView() {
  const [date, setDate] = useState(logicalDate())
  const review = useDailyReview(date)
  const [dismissedErr, setDismissedErr] = useState(false)

  const error = review.error && !dismissedErr ? String((review.error as Error).message) : null
  const isToday = date === logicalDate()

  return (
    <div className="mx-auto max-w-3xl px-6 py-8">
      {/* Header with date navigation */}
      <div className="mb-6 flex items-center gap-3">
        <div>
          <h1 className="text-lg font-semibold">Review</h1>
          <p className="mt-0.5 font-mono text-xs text-muted-foreground">{dateLabel(date)}</p>
        </div>
        <div className="ml-auto flex items-center gap-1">
          <Button
            variant="ghost"
            size="icon"
            onClick={() => setDate(shiftDate(date, -1))}
          >
            <ChevronLeft className="size-4" />
          </Button>
          <Input
            type="date"
            value={date}
            onChange={(e) => e.target.value && setDate(e.target.value)}
            className="w-36 font-mono text-xs"
          />
          <Button
            variant="ghost"
            size="icon"
            onClick={() => setDate(shiftDate(date, 1))}
          >
            <ChevronRight className="size-4" />
          </Button>
          {!isToday && (
            <Button variant="ghost" size="sm" onClick={() => setDate(logicalDate())} className="text-xs">
              Today
            </Button>
          )}
        </div>
      </div>

      {error && <ErrorBanner message={error} onDismiss={() => setDismissedErr(true)} />}

      {review.isLoading && <LoadingRow label="Loading review…" />}

      {!review.isLoading && !error && review.data && (
        <div className="space-y-6">
          <p className="text-xs text-muted-foreground/60">
            A review of the day — planned work, what was touched, completed, and carried over.
          </p>
          {SECTIONS.map((sec) => {
            const items = review.data![sec.key] as Todo[]
            return (
              <section key={sec.key}>
                <div className="mb-2 flex items-center gap-2">
                  <h2 className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
                    {sec.label}
                  </h2>
                  <span className="text-xs text-muted-foreground/50">{items.length}</span>
                </div>
                {items.length === 0 ? (
                  <p className="py-2 pl-3 text-sm text-muted-foreground/40">—</p>
                ) : (
                  <div className="space-y-0.5">
                    {items.map((t) => (
                      <TodoRow key={t.id} todo={t} compact />
                    ))}
                  </div>
                )}
              </section>
            )
          })}
        </div>
      )}

      {!review.isLoading && !error && !review.data && itemsEmpty(review.data) && (
        <EmptyState
          message="No data for this date."
          hint="Pick a different date, or this day had no activity."
        />
      )}
    </div>
  )
}

// True when all four lists are empty.
function itemsEmpty(data: { planned: unknown[]; touched: unknown[]; completed: unknown[]; carriedOver: unknown[] } | undefined): boolean {
  if (!data) return true
  return data.planned.length === 0 && data.touched.length === 0 && data.completed.length === 0 && data.carriedOver.length === 0
}
