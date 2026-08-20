/**
 * Menu-bar command: today's count, sync state, quick-create + quick-status.
 * Uses MenuBarExtra. Compact and calm.
 */
import {
  Color,
  Icon,
  LaunchType,
  MenuBarExtra,
  launchCommand,
} from "@raycast/api";
import { useEffect, useState } from "react";
import { fetchClock, fetchSyncStates, fetchTodayPlan, setTodoStatus, createTodo } from "./lib/graphql";
import { statusVisual } from "./lib/helpers";
import type { Clock, PlanRow, SyncState } from "./types";

interface MenuBarState {
  plan: PlanRow[];
  sync: SyncState[];
  date: string;
}

async function load(): Promise<MenuBarState> {
  const clock: Clock = await fetchClock();
  const [plan, sync] = await Promise.all([
    fetchTodayPlan(clock.logicalDate),
    fetchSyncStates(),
  ]);
  return { plan, sync, date: clock.logicalDate };
}

function syncSummary(sync: SyncState[]): { label: string; color: Color; tooltip: string }[] {
  return sync.map((s) => {
    const ok = s.lastStatus === "ok" || s.lastStatus === "idle";
    return {
      label: `${s.source}: ${ok ? "ok" : s.lastStatus}`,
      color: ok ? Color.Green : Color.Orange,
      tooltip: s.lastError
        ? `${s.source} — ${s.lastError}`
        : `${s.source} last synced ${s.lastSyncedAt ?? "never"}`,
    };
  });
}

export default function MenuBarCommand() {
  const [state, setState] = useState<MenuBarState | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [reloadTick, setReloadTick] = useState(0);

  useEffect(() => {
    let cancelled = false;
    load()
      .then((s) => {
        if (!cancelled) {
          setState(s);
          setErr(null);
        }
      })
      .catch((e: unknown) => {
        if (cancelled) return;
        setErr(e instanceof Error ? e.message : String(e));
        setState({ plan: [], sync: [], date: "" });
      });
    return () => {
      cancelled = true;
    };
  }, [reloadTick]);

  const date = state?.date ?? "";
  const count = state?.plan.length ?? 0;
  const done = state?.plan.filter((r) => r.todo?.status === "done").length ?? 0;
  const errored = err != null;

  const glyph = errored ? "!" : count === 0 ? "○" : `${done}/${count}`;
  const glyphColor = errored
    ? Color.Red
    : done === count && count > 0
      ? Color.Green
      : Color.PrimaryText;

  return (
    <MenuBarExtra
      icon={{ source: Icon.List, tintColor: glyphColor }}
      title={errored ? "preflight: offline" : `${glyph} today`}
      tooltip={errored ? "Backend unreachable" : `${count} planned · ${done} done · ${date}`}
      isLoading={state == null && err == null}
    >
      {errored ? (
        <MenuBarExtra.Item
          title="Backend unreachable"
          icon={{ source: Icon.ExclamationMark, tintColor: Color.Red }}
        />
      ) : (
        <>
          <MenuBarExtra.Item
            title={`${count} planned · ${done} done`}
            subtitle={date}
            icon={{ source: Icon.Calendar, tintColor: Color.Blue }}
          />
          {count > 0 ? (
            <>
              <MenuBarExtra.Section title="Today">
                {state!.plan.slice(0, 8).map((row) => {
                  const t = row.todo;
                  if (!t) return null;
                  const v = statusVisual(t.status);
                  return (
                    <MenuBarExtra.Submenu
                      key={row.id}
                      title={t.title}
                      icon={{ source: v.icon, tintColor: v.color }}
                    >
                      <MenuBarExtra.Item
                        title={`Status: ${v.label}`}
                        icon={{ source: v.icon, tintColor: v.color }}
                      />
                      {t.status !== "done" ? (
                        <MenuBarExtra.Item
                          title="Mark Done"
                          icon={{ source: Icon.Checkmark, tintColor: Color.Green }}
                          onAction={async () => {
                            try {
                              await setTodoStatus(t.id, "done");
                              setReloadTick((n) => n + 1);
                            } catch (e) {
                              console.error(e);
                            }
                          }}
                        />
                      ) : (
                        <MenuBarExtra.Item
                          title="Reopen as Todo"
                          icon={{ source: Icon.Circle, tintColor: Color.SecondaryText }}
                          onAction={async () => {
                            try {
                              await setTodoStatus(t.id, "todo");
                              setReloadTick((n) => n + 1);
                            } catch (e) {
                              console.error(e);
                            }
                          }}
                        />
                      )}
                    </MenuBarExtra.Submenu>
                  );
                })}
              </MenuBarExtra.Section>
              <MenuBarExtra.Section>
                <MenuBarExtra.Item
                  title="Open Today in Preflight"
                  icon={{ source: Icon.List }}
                  onAction={() =>
                    launchCommand({ name: "today", type: LaunchType.UserInitiated }).catch(() => {})
                  }
                />
              </MenuBarExtra.Section>
            </>
          ) : (
            <MenuBarExtra.Item
              title="Nothing planned for today"
              icon={{ source: Icon.Circle, tintColor: Color.SecondaryText }}
            />
          )}

          <MenuBarExtra.Section title="Quick Create">
            <MenuBarExtra.Item
              title="New Todo"
              icon={{ source: Icon.Plus, tintColor: Color.Blue }}
              onAction={async () => {
                try {
                  await createTodo("New todo");
                  setReloadTick((n) => n + 1);
                } catch (e) {
                  console.error(e);
                }
              }}
            />
            <MenuBarExtra.Item
              title="Open Quick Create Form"
              icon={{ source: Icon.Pencil }}
              onAction={() =>
                launchCommand({ name: "quick-create", type: LaunchType.UserInitiated }).catch(() => {})
              }
            />
          </MenuBarExtra.Section>

          {state != null && state.sync.length > 0 && (
            <MenuBarExtra.Section title="Sync">
              {syncSummary(state.sync).map((s) => (
                <MenuBarExtra.Item
                  key={s.label}
                  title={s.label}
                  icon={{ source: Icon.Repeat, tintColor: s.color }}
                  tooltip={s.tooltip}
                />
              ))}
            </MenuBarExtra.Section>
          )}

          <MenuBarExtra.Section>
            <MenuBarExtra.Item
              title="Refresh"
              icon={{ source: Icon.ArrowClockwise }}
              onAction={() => setReloadTick((n) => n + 1)}
            />
          </MenuBarExtra.Section>
        </>
      )}
    </MenuBarExtra>
  );
}
