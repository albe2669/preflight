/**
 * Review command: DailyReview for a picked date.
 * Four sections: planned, touched, completed, carried over.
 */
import { Action, ActionPanel, Color, Icon, List } from "@raycast/api";
import { useState } from "react";
import { fetchDailyReview } from "./lib/graphql";
import { statusVisual, todayLogical } from "./lib/helpers";
import { useFetch } from "./hooks/useFetch";
import type { Todo } from "./types";

function shiftDays(date: string, days: number): string {
  const d = new Date(`${date}T12:00:00`);
  d.setDate(d.getDate() + days);
  return d.toISOString().slice(0, 10);
}

export default function ReviewCommand() {
  const [date, setDate] = useState<string>(() => todayLogical());
  const { data, loading, error, unreachable, reload } = useFetch(() => fetchDailyReview(date), [date]);

  function shiftDate(days: number) {
    setDate((d) => shiftDays(d, days));
  }

  return (
    <List
      searchBarPlaceholder="Filter review"
      actions={
        <ActionPanel>
          <Action
            title="Previous Day"
            icon={Icon.ChevronLeft}
            shortcut={{ modifiers: ["cmd"], key: "[" }}
            onAction={() => shiftDate(-1)}
          />
          <Action
            title="Next Day"
            icon={Icon.ChevronRight}
            shortcut={{ modifiers: ["cmd"], key: "]" }}
            onAction={() => shiftDate(1)}
          />
          <Action
            title="Today"
            icon={Icon.Calendar}
            shortcut={{ modifiers: ["cmd"], key: "t" }}
            onAction={() => setDate(todayLogical())}
          />
          <Action title="Refresh" icon={Icon.ArrowClockwise} onAction={reload} />
        </ActionPanel>
      }
    >
      <List.Section title={`Review · ${date}`}>
        {loading ? (
          <List.Item title="Loading review…" icon={{ source: Icon.ArrowClockwise, tintColor: Color.SecondaryText }} />
        ) : unreachable ? (
          <List.Item
            title="Backend unreachable"
            icon={{ source: Icon.ExclamationMark, tintColor: Color.Red }}
            subtitle="Start the server, then refresh"
          />
        ) : error ? (
          <List.Item
            title="Could not load review"
            icon={{ source: Icon.ExclamationMark, tintColor: Color.Orange }}
            subtitle={error}
          />
        ) : null}
      </List.Section>

      {data && !loading && !error && !unreachable && (
        <>
          <ReviewSection title="Planned" todos={data.planned} icon={Icon.Calendar} />
          <ReviewSection title="Touched" todos={data.touched} icon={Icon.Pencil} />
          <ReviewSection title="Completed" todos={data.completed} icon={Icon.Checkmark} />
          <ReviewSection title="Carried Over" todos={data.carriedOver} icon={Icon.ArrowDown} />
        </>
      )}
    </List>
  );
}

function ReviewSection({ title, todos, icon }: { title: string; todos: Todo[]; icon: Icon }) {
  return (
    <List.Section title={`${title} · ${todos.length}`}>
      {todos.length === 0 ? (
        <List.Item title={`Nothing ${title.toLowerCase()}`} icon={{ source: icon, tintColor: Color.SecondaryText }} />
      ) : (
        todos.map((t) => {
          const v = statusVisual(t.status);
          return (
            <List.Item
              key={`${title}-${t.id}`}
              title={t.title}
              icon={{ source: v.icon, tintColor: v.color }}
              accessories={[{ tag: v.label, icon: { source: v.icon, tintColor: v.color } }]}
            />
          );
        })
      )}
    </List.Section>
  );
}
