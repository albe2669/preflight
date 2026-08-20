/**
 * Today command: ordered day-plan list for the logical date.
 * Status glyphs, tags, link badges, blockedReason, carried-over marks.
 * Inline actions: set status, unplan, reorder, add tag, quick-create.
 */
import {
  Action,
  ActionPanel,
  Color,
  Form,
  Icon,
  List,
  Toast,
  confirmAlert,
  showToast,
} from "@raycast/api";
import { useState } from "react";
import {
  addTag,
  createTodo,
  fetchClock,
  fetchTodayPlan,
  GraphqlError,
  planForToday,
  reorderDayPlan,
  setTodoStatus,
  unplanForToday,
} from "./lib/graphql";
import { prBadge, statusVisual } from "./lib/helpers";
import { SetStatusAction } from "./lib/actions";
import { useFetch } from "./hooks/useFetch";
import type { TodoStatus } from "./types";

export default function TodayCommand() {
  const { data: clock, loading: clockLoading } = useFetch(() => fetchClock(), []);
  const date = clock?.logicalDate ?? "";
  const { data, loading, error, unreachable, reload } = useFetch(
    () => (date ? fetchTodayPlan(date) : Promise.resolve([])),
    [date],
  );
  const [showCreate, setShowCreate] = useState(false);
  const [order, setOrder] = useState<number[] | null>(null);

  const rows = data ?? [];
  const ordered = order != null ? order.map((id) => rows.find((r) => r.todoId === id)!).filter(Boolean) : rows;

  async function refresh(toast: Toast, title: string) {
    toast.title = title;
    toast.style = Toast.Style.Success;
    await showToast(toast);
    setOrder(null);
    reload();
  }

  async function failToast(e: unknown, title: string) {
    const t = await showToast({ style: Toast.Style.Failure, title });
    t.message =
      e instanceof GraphqlError ? e.message : e instanceof TypeError ? "Backend unreachable" : String(e);
  }

  async function onSetStatus(id: number, status: TodoStatus) {
    const toast = await showToast({ style: Toast.Style.Animated, title: "Updating status" });
    try {
      await setTodoStatus(id, status);
      await refresh(toast, "Status updated");
    } catch (e) {
      await failToast(e, "Could not update status");
    }
  }

  async function onUnplan(id: number) {
    const toast = await showToast({ style: Toast.Style.Animated, title: "Unplanning" });
    try {
      await unplanForToday(id);
      await refresh(toast, "Removed from today");
    } catch (e) {
      await failToast(e, "Could not unplan");
    }
  }

  async function onMoveUp(index: number) {
    if (index <= 0) return;
    const ids = ordered.map((r) => r.todoId);
    [ids[index - 1], ids[index]] = [ids[index], ids[index - 1]];
    setOrder(ids);
    await persistOrder(ids);
  }

  async function onMoveDown(index: number) {
    if (index >= ordered.length - 1) return;
    const ids = ordered.map((r) => r.todoId);
    [ids[index + 1], ids[index]] = [ids[index], ids[index + 1]];
    setOrder(ids);
    await persistOrder(ids);
  }

  async function persistOrder(ids: number[]) {
    const toast = await showToast({ style: Toast.Style.Animated, title: "Reordering" });
    try {
      await reorderDayPlan(date, ids);
      toast.title = "Order saved";
      toast.style = Toast.Style.Success;
      await showToast(toast);
      setOrder(null);
      reload();
    } catch (e) {
      await failToast(e, "Could not reorder");
      setOrder(null);
    }
  }

  async function onAddTag(id: number, slug: string) {
    const toast = await showToast({ style: Toast.Style.Animated, title: "Adding tag" });
    try {
      await addTag(id, slug);
      await refresh(toast, "Tag added");
    } catch (e) {
      await failToast(e, "Could not add tag");
    }
  }

  async function onCreate(values: { title: string; planToday: boolean }) {
    const trimmed = values.title.trim();
    if (!trimmed) return;
    const toast = await showToast({ style: Toast.Style.Animated, title: "Creating todo" });
    try {
      const todo = await createTodo(trimmed);
      if (values.planToday) await planForToday(todo.id);
      setShowCreate(false);
      await refresh(toast, "Todo created");
    } catch (e) {
      await failToast(e, "Could not create todo");
    }
  }

  if (clockLoading || loading) {
    return (
      <List isLoading>
        <List.Item title={`Loading today's plan · ${date}…`} icon={Icon.ArrowClockwise} />
      </List>
    );
  }

  if (unreachable) {
    return (
      <List>
        <List.EmptyView
          icon={{ source: Icon.ExclamationMark, tintColor: Color.Red }}
          title="Backend unreachable"
          description="Start the preflight server on 127.0.0.1:8000, then refresh."
          actions={
            <ActionPanel>
              <Action title="Refresh" icon={Icon.ArrowClockwise} onAction={reload} />
            </ActionPanel>
          }
        />
      </List>
    );
  }

  if (error) {
    return (
      <List>
        <List.EmptyView
          icon={{ source: Icon.ExclamationMark, tintColor: Color.Orange }}
          title="Could not load today's plan"
          description={error}
          actions={
            <ActionPanel>
              <Action title="Refresh" icon={Icon.ArrowClockwise} onAction={reload} />
            </ActionPanel>
          }
        />
      </List>
    );
  }

  if (ordered.length === 0 && !showCreate) {
    return (
      <List
        actions={
          <ActionPanel>
            <Action
              title="New Todo"
              icon={Icon.Plus}
              shortcut={{ modifiers: ["cmd"], key: "n" }}
              onAction={() => setShowCreate(true)}
            />
            <Action title="Refresh" icon={Icon.ArrowClockwise} onAction={reload} />
          </ActionPanel>
        }
      >
        <List.EmptyView
          icon={{ source: Icon.Sun, tintColor: Color.SecondaryText }}
          title="Nothing planned for today"
          description={`A new day starts empty (${date}). Add a todo to begin.`}
        />
      </List>
    );
  }

  return (
    <List
      searchBarPlaceholder="Filter today's plan"
      actions={
        <ActionPanel>
          <Action
            title="New Todo"
            icon={Icon.Plus}
            shortcut={{ modifiers: ["cmd"], key: "n" }}
            onAction={() => setShowCreate(true)}
          />
          <Action title="Refresh" icon={Icon.ArrowClockwise} onAction={reload} />
        </ActionPanel>
      }
    >
      <List.Section title={`Today · ${date}`} subtitle={ordered.length > 0 ? `${ordered.length} planned` : undefined}>
        {ordered.map((row, i) => {
          const t = row.todo;
          if (!t) {
            return (
              <List.Item
                key={row.id}
                title={`(missing todo ${row.todoId})`}
                icon={{ source: Icon.QuestionMark, tintColor: Color.Red }}
              />
            );
          }
          const v = statusVisual(t.status);
          const accessories: List.Item.Accessory[] = [
            { tag: v.label, icon: { source: v.icon, tintColor: v.color } },
          ];
          if (row.carriedOver)
            accessories.push({ icon: Icon.ArrowDown, tooltip: "Carried over from a prior day" });
          if (t.blockedReason) accessories.push({ icon: Icon.ExclamationMark, tooltip: t.blockedReason });
          for (const tag of row.tags) {
            accessories.push({ tag: tag.slug, icon: Icon.Tag });
          }
          for (const pr of row.pullRequests) {
            if (pr.pullRequest) {
              accessories.push({
                tag: prBadge(
                  pr.pullRequest.owner,
                  pr.pullRequest.repo,
                  pr.pullRequest.number,
                  pr.relation,
                ),
                icon: Icon.Code,
              });
            }
          }
          for (const li of row.linearIssues) {
            if (li.linearIssue) {
              accessories.push({ tag: li.linearIssue.identifier, icon: Icon.BulletPoints });
            }
          }

          return (
            <List.Item
              key={row.id}
              id={String(row.id)}
              title={t.title}
              icon={{ source: v.icon, tintColor: v.color }}
              accessories={accessories}
              detail={
                t.description || t.blockedReason ? (
                  <List.Item.Detail
                    markdown={[t.description, t.blockedReason ? `**Blocked:** ${t.blockedReason}` : ""]
                      .filter(Boolean)
                      .join("\n\n")}
                  />
                ) : undefined
              }
              actions={
                <ActionPanel>
                  <SetStatusAction current={t.status} onSet={(s) => onSetStatus(t.id, s)} />
                  <ActionPanel.Section title="Plan">
                    <Action
                      title="Remove from Today"
                      icon={Icon.Minus}
                      onAction={async () => {
                        if (
                          await confirmAlert({
                            title: "Remove from today's plan?",
                            primaryAction: { title: "Remove" },
                          })
                        ) {
                          onUnplan(t.id);
                        }
                      }}
                    />
                    <Action title="Move Up" icon={Icon.ArrowUp} onAction={() => onMoveUp(i)} />
                    <Action title="Move Down" icon={Icon.ArrowDown} onAction={() => onMoveDown(i)} />
                  </ActionPanel.Section>
                  <ActionPanel.Section title="Tags">
                    <Action.Push
                      title="Add Tag"
                      icon={Icon.Tag}
                      target={<AddTagForm onAdd={(slug) => onAddTag(t.id, slug)} />}
                    />
                  </ActionPanel.Section>
                  <ActionPanel.Section>
                    <Action title="Refresh" icon={Icon.ArrowClockwise} onAction={reload} />
                  </ActionPanel.Section>
                </ActionPanel>
              }
            />
          );
        })}
      </List.Section>

      {showCreate && (
        <List.Section title="New Todo">
          <List.Item
            title="Create a todo…"
            icon={Icon.Plus}
            actions={
              <ActionPanel>
                <Action.Push title="Create Todo" icon={Icon.Plus} target={<CreateForm onCreate={onCreate} />} />
              </ActionPanel>
            }
          />
        </List.Section>
      )}
    </List>
  );
}

function CreateForm({ onCreate }: { onCreate: (values: { title: string; planToday: boolean }) => void }) {
  return (
    <Form
      actions={
        <ActionPanel>
          <Action.SubmitForm title="Create Todo" onSubmit={onCreate} />
        </ActionPanel>
      }
    >
      <Form.TextField id="title" title="Title" placeholder="What needs to happen?" autoFocus />
      <Form.Checkbox id="planToday" title="Plan for today" label="Add to today's plan" defaultValue />
    </Form>
  );
}

function AddTagForm({ onAdd }: { onAdd: (slug: string) => void }) {
  const [slug, setSlug] = useState("");
  return (
    <Form
      actions={
        <ActionPanel>
          <Action.SubmitForm title="Add Tag" onSubmit={() => onAdd(slug.trim())} />
        </ActionPanel>
      }
    >
      <Form.TextField
        id="slug"
        title="Tag slug"
        placeholder="e.g. bug, design, urgent"
        value={slug}
        onChange={setSlug}
        autoFocus
      />
    </Form>
  );
}
