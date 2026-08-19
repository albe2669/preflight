/**
 * Shared status action panel builder. Builds the set-status sub-actions
 * plus the common actions (toggle plan, add tag, open links). One source of
 * truth for the status menu so Today, Inbox-converted, and MenuBar agree.
 */
import { Action, ActionPanel } from "@raycast/api";
import { TODO_STATUSES, statusVisual } from "./helpers";
import type { TodoStatus } from "../types";

export function SetStatusAction({
  current,
  onSet,
}: {
  current: TodoStatus;
  onSet: (status: TodoStatus) => void;
}) {
  return (
    <ActionPanel.Section title="Set Status">
      {TODO_STATUSES.filter((s) => s !== current).map((s) => {
        const v = statusVisual(s);
        return (
          <Action
            key={s}
            icon={{ source: v.icon, tintColor: v.color }}
            title={`Mark ${v.label}`}
            onAction={() => onSet(s)}
          />
        );
      })}
    </ActionPanel.Section>
  );
}
