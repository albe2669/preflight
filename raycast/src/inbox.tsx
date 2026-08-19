/**
 * Inbox command: triage GitHub PRs and Linear issues.
 * Grouped by source; per-row convert/link/dismiss; sync trigger per group.
 */
import {
  Action,
  ActionPanel,
  Color,
  Icon,
  List,
  Toast,
  confirmAlert,
  showToast,
} from "@raycast/api";
import { useState } from "react";
import {
  dismissPullRequest,
  fetchLinearIssues,
  fetchPullRequests,
  fetchSyncStates,
  fetchTodos,
  GraphqlError,
  linkLinearIssue,
  linkPullRequest,
  syncGithub,
  syncLinear,
  todoFromLinearIssue,
  todoFromPullRequest,
} from "./lib/graphql";
import { linearStateTypeVisual, prBadge, prStateVisual, relativeTime, statusVisual } from "./lib/helpers";
import { useFetch } from "./hooks/useFetch";
import type { LinearIssue, PullRequest, SyncState } from "./types";

interface InboxData {
  pulls: PullRequest[];
  linears: LinearIssue[];
  sync: SyncState[];
}

export default function InboxCommand() {
  const { data, loading, error, unreachable, reload } = useFetch(
    async (): Promise<InboxData> => {
      const [pulls, linears, sync] = await Promise.all([
        fetchPullRequests(),
        fetchLinearIssues(),
        fetchSyncStates(),
      ]);
      return { pulls, linears, sync };
    },
    [],
  );

  const [showDismissed, setShowDismissed] = useState(false);

  const pulls = data?.pulls ?? [];
  const linears = data?.linears ?? [];
  const sync = data?.sync ?? [];

  const seenPulls = showDismissed ? pulls : pulls.filter((p) => !p.dismissedAt);
  const seenLinears = showDismissed ? linears : linears.filter((l) => !l.dismissedAt);

  const syncGithubState = sync.find((s) => s.source === "github");
  const syncLinearState = sync.find((s) => s.source === "linear");

  async function failToast(e: unknown, title: string) {
    const t = await showToast({ style: Toast.Style.Failure, title });
    t.message =
      e instanceof GraphqlError ? e.message : e instanceof TypeError ? "Backend unreachable" : String(e);
  }

  async function run(label: string, fn: () => Promise<unknown>) {
    const toast = await showToast({ style: Toast.Style.Animated, title: label });
    try {
      await fn();
      toast.title = "Done";
      toast.style = Toast.Style.Success;
      await showToast(toast);
      reload();
    } catch (e) {
      await failToast(e, label);
    }
  }

  if (loading) {
    return <List isLoading><List.Item title="Loading inbox…" icon={Icon.ArrowClockwise} /></List>;
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
          title="Could not load inbox"
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

  const empty = seenPulls.length === 0 && seenLinears.length === 0 && sync.length === 0;

  return (
    <List
      searchBarPlaceholder="Filter inbox"
      actions={
        <ActionPanel>
          <Action
            title={showDismissed ? "Hide Dismissed" : "Show Dismissed"}
            icon={Icon.Eye}
            onAction={() => setShowDismissed((s) => !s)}
          />
          <Action title="Refresh" icon={Icon.ArrowClockwise} onAction={reload} />
        </ActionPanel>
      }
    >
      {empty ? (
        <List.EmptyView
          icon={{ source: Icon.CheckCircle, tintColor: Color.SecondaryText }}
          title="Inbox is clear"
          description="No GitHub PRs or Linear issues to triage."
        />
      ) : (
        <>
          <List.Section
            title="GitHub Pull Requests"
            subtitle={`${seenPulls.length} ${showDismissed ? "(all)" : "active"}`}
          >
            <SyncRow state={syncGithubState} label="GitHub" onSync={() => run("Syncing GitHub", syncGithub)} />
            {seenPulls.length === 0 ? (
              <List.Item
                title="No pull requests to triage"
                icon={{ source: Icon.CheckCircle, tintColor: Color.SecondaryText }}
                subtitle="Inbox clear"
              />
            ) : (
              seenPulls.map((pr) => <PrRow key={pr.id} pr={pr} onDone={() => reload()} />)
            )}
          </List.Section>

          <List.Section
            title="Linear Issues"
            subtitle={`${seenLinears.length} ${showDismissed ? "(all)" : "active"}`}
          >
            <SyncRow state={syncLinearState} label="Linear" onSync={() => run("Syncing Linear", syncLinear)} />
            {seenLinears.length === 0 ? (
              <List.Item
                title="No Linear issues to triage"
                icon={{ source: Icon.CheckCircle, tintColor: Color.SecondaryText }}
                subtitle="Inbox clear"
              />
            ) : (
              seenLinears.map((li) => <LinearRow key={li.id} issue={li} onDone={() => reload()} />)
            )}
          </List.Section>
        </>
      )}
    </List>
  );
}

function SyncRow({
  state,
  label,
  onSync,
}: {
  state: SyncState | undefined;
  label: string;
  onSync: () => void;
}) {
  if (!state) {
    return (
      <List.Item
        title={`${label} sync not configured`}
        icon={{ source: Icon.Dot, tintColor: Color.SecondaryText }}
        subtitle="No token set"
      />
    );
  }
  const ok = state.lastStatus === "ok" || state.lastStatus === "idle";
  return (
    <List.Item
      title={`${label}: synced ${relativeTime(state.lastSyncedAt)}`}
      icon={{ source: Icon.Repeat, tintColor: ok ? Color.Green : Color.Orange }}
      accessories={state.lastError ? [{ text: state.lastError }] : [{ text: state.lastStatus }]}
      actions={
        <ActionPanel>
          <Action title={`Sync ${label} Now`} icon={Icon.Repeat} onAction={onSync} />
        </ActionPanel>
      }
    />
  );
}

/** Run a triage mutation with a toast; refresh the list only on success. */
async function triage(label: string, fn: () => Promise<unknown>, onDone: () => void) {
  const toast = await showToast({ style: Toast.Style.Animated, title: label });
  try {
    await fn();
    toast.title = "Done";
    toast.style = Toast.Style.Success;
    await showToast(toast);
    onDone();
  } catch (e) {
    toast.style = Toast.Style.Failure;
    toast.title = `Could not ${label.toLowerCase()}`;
    toast.message =
      e instanceof GraphqlError ? e.message : e instanceof TypeError ? "Backend unreachable" : String(e);
    await showToast(toast);
  }
}

/** Picker pushed by "Link to Todo": lists todos and links the chosen one. */
function LinkTodoList({
  title,
  onLink,
}: {
  title: string;
  onLink: (todoId: number) => Promise<void>;
}) {
  const { data, loading, error, unreachable } = useFetch(() => fetchTodos(), []);
  const todos = data ?? [];
  return (
    <List
      isLoading={loading}
      searchBarPlaceholder={`Link: ${title}`}
    >
      {unreachable ? (
        <List.EmptyView
          icon={{ source: Icon.ExclamationMark, tintColor: Color.Red }}
          title="Backend unreachable"
          description="Start the preflight server, then refresh."
        />
      ) : error ? (
        <List.EmptyView
          icon={{ source: Icon.ExclamationMark, tintColor: Color.Orange }}
          title="Could not load todos"
          description={error}
        />
      ) : todos.length === 0 && !loading ? (
        <List.EmptyView
          icon={{ source: Icon.CheckCircle, tintColor: Color.SecondaryText }}
          title="No todos to link"
          description="Create a todo first, then link this item."
        />
      ) : null}
      <List.Section title={`Todos · ${todos.length}`}>
        {todos.map((t) => {
          const v = statusVisual(t.status);
          return (
            <List.Item
              key={t.id}
              title={t.title}
              icon={{ source: v.icon, tintColor: v.color }}
              accessories={[{ tag: v.label }]}
              actions={
                <ActionPanel>
                  <Action
                    title="Link to This Todo"
                    icon={Icon.Link}
                    onAction={async () => {
                      await triage("Linking", () => onLink(t.id), () => {});
                    }}
                  />
                </ActionPanel>
              }
            />
          );
        })}
      </List.Section>
    </List>
  );
}

function PrRow({ pr, onDone }: { pr: PullRequest; onDone: () => void }) {
  const v = prStateVisual(pr.state);
  const accessories: List.Item.Accessory[] = [
    { tag: prBadge(pr.owner, pr.repo, pr.number), icon: Icon.Code },
  ];
  if (pr.reviewRequested) accessories.push({ icon: Icon.Eye, tooltip: "Review requested" });
  if (pr.authoredByMe) accessories.push({ icon: Icon.Person, tooltip: "Authored by me" });
  if (pr.dismissedAt) accessories.push({ icon: Icon.MinusCircle, tooltip: "Dismissed" });

  return (
    <List.Item
      title={pr.title}
      subtitle={pr.author ?? undefined}
      icon={{ source: v.icon, tintColor: v.color }}
      accessories={accessories}
      actions={
        <ActionPanel>
          <ActionPanel.Section title="Triage">
            <Action
              title="Convert to Todo (plan today)"
              icon={Icon.Plus}
              onAction={() => triage("Converting", () => todoFromPullRequest(pr.id, true), onDone)}
            />
            <Action
              title="Convert to Todo (backlog)"
              icon={Icon.PlusCircle}
              onAction={() => triage("Converting", () => todoFromPullRequest(pr.id, false), onDone)}
            />
            <Action.Push
              title="Link to Todo…"
              icon={Icon.Link}
              target={
                <LinkTodoList
                  title={pr.title}
                  onLink={(todoId) =>
                    linkPullRequest(todoId, pr.id, "references").then(() => onDone())
                  }
                />
              }
            />
            <Action
              title="Dismiss"
              icon={Icon.MinusCircle}
              onAction={async () => {
                if (
                  await confirmAlert({ title: "Dismiss this PR?", primaryAction: { title: "Dismiss" } })
                ) {
                  await triage("Dismissing", () => dismissPullRequest(pr.id), onDone);
                }
              }}
            />
          </ActionPanel.Section>
          <ActionPanel.Section>
            <Action.OpenInBrowser title="Open PR" url={pr.url} />
          </ActionPanel.Section>
        </ActionPanel>
      }
    />
  );
}

function LinearRow({ issue, onDone }: { issue: LinearIssue; onDone: () => void }) {
  const v = linearStateTypeVisual(issue.stateType);
  const accessories: List.Item.Accessory[] = [{ tag: issue.identifier, icon: Icon.BulletPoints }];
  if (issue.assigneeName) accessories.push({ text: issue.assigneeName });
  if (issue.assignedToMe) accessories.push({ icon: Icon.Person, tooltip: "Assigned to me" });
  if (issue.priority != null && issue.priority > 0)
    accessories.push({ icon: Icon.Star, tooltip: `Priority ${issue.priority}` });
  if (issue.dismissedAt) accessories.push({ icon: Icon.MinusCircle, tooltip: "Dismissed" });

  return (
    <List.Item
      title={issue.title}
      icon={{ source: v.icon, tintColor: v.color }}
      accessories={accessories}
      actions={
        <ActionPanel>
          <ActionPanel.Section title="Triage">
            <Action
              title="Convert to Todo (plan today)"
              icon={Icon.Plus}
              onAction={() => triage("Converting", () => todoFromLinearIssue(issue.id, true), onDone)}
            />
            <Action
              title="Convert to Todo (backlog)"
              icon={Icon.PlusCircle}
              onAction={() => triage("Converting", () => todoFromLinearIssue(issue.id, false), onDone)}
            />
            <Action.Push
              title="Link to Todo…"
              icon={Icon.Link}
              target={
                <LinkTodoList
                  title={issue.title}
                  onLink={(todoId) =>
                    linkLinearIssue(todoId, issue.id).then(() => onDone())
                  }
                />
              }
            />
          </ActionPanel.Section>
          <ActionPanel.Section>
            <Action.OpenInBrowser title="Open Issue" url={issue.url} />
          </ActionPanel.Section>
        </ActionPanel>
      }
    />
  );
}
