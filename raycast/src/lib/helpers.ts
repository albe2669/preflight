/**
 * Shared helpers: config-file endpoint, status glyphs/icons/colors, link badges.
 * Color is always a secondary cue; a glyph or icon carries the meaning.
 */
import { Color, Icon } from "@raycast/api";
import * as fs from "node:fs";
import * as path from "node:path";
import * as os from "node:os";
import type {
  TodoStatus,
  PrState,
  LinearStateType,
  EventActor,
  LinkRelation,
} from "../types";

const DEFAULT_ENDPOINT = "http://127.0.0.1:8000/";

/** Path to the shared preflight config file ($XDG_CONFIG_HOME or $HOME/.config). */
function configPath(): string {
  const base = process.env.XDG_CONFIG_HOME || path.join(os.homedir(), ".config");
  return path.join(base, "preflight", "config.toml");
}

/** Read host and port from the [server] section of the shared config file. */
export function readConfig(): { host?: string; port?: number } {
  let text: string;
  try {
    text = fs.readFileSync(configPath(), "utf8");
  } catch {
    return {};
  }
  // Parse the [server] section line-by-line: from its header to the next
  // header (or end of file). Avoids regex multiline-anchor pitfalls.
  const lines = text.split(/\r?\n/);
  let inServer = false;
  let host: string | undefined;
  let port: number | undefined;
  for (const line of lines) {
    const header = line.match(/^\s*\[(.*)\]\s*$/);
    if (header) {
      inServer = header[1] === "server";
      continue;
    }
    if (inServer) {
      const h = line.match(/^\s*host\s*=\s*"([^"]*)"/);
      if (h) host = h[1];
      const p = line.match(/^\s*port\s*=\s*(\d+)/);
      if (p) port = Number(p[1]);
    }
  }
  return { host, port };
}

/** Endpoint from the shared config file, falling back to the default. */
export function preferredEndpoint(): string {
  const { host, port } = readConfig();
  if (host && port != null && Number.isFinite(port)) {
    return `http://${host}:${port}/`;
  }
  return DEFAULT_ENDPOINT;
}

// ---- Todo status: glyph + icon + color ----

export interface StatusVisual {
  glyph: string;
  icon: Icon;
  color: Color;
  label: string;
}

export function statusVisual(status: TodoStatus): StatusVisual {
  switch (status) {
    case "todo":
      return { glyph: "○", icon: Icon.Circle, color: Color.SecondaryText, label: "Todo" };
    case "started":
      return { glyph: "◐", icon: Icon.CircleProgress25, color: Color.Blue, label: "Started" };
    case "blocked":
      return { glyph: "⊘", icon: Icon.ExclamationMark, color: Color.Orange, label: "Blocked" };
    case "done":
      return { glyph: "✓", icon: Icon.Checkmark, color: Color.Green, label: "Done" };
    case "cancelled":
      return { glyph: "✕", icon: Icon.Xmark, color: Color.SecondaryText, label: "Cancelled" };
  }
}

export const TODO_STATUSES: TodoStatus[] = ["todo", "started", "blocked", "done", "cancelled"];

// ---- PR state: icon + color (glyph is the state word) ----

export interface PrStateVisual {
  icon: Icon;
  color: Color;
  label: string;
}

export function prStateVisual(state: PrState): PrStateVisual {
  switch (state) {
    case "open":
      return { icon: Icon.Circle, color: Color.Green, label: "Open" };
    case "closed":
      return { icon: Icon.XMarkCircle, color: Color.SecondaryText, label: "Closed" };
    case "merged":
      return { icon: Icon.Repeat, color: Color.Purple, label: "Merged" };
    case "draft":
      return { icon: Icon.Circle, color: Color.SecondaryText, label: "Draft" };
  }
}

export function linearStateTypeVisual(stateType: LinearStateType): PrStateVisual {
  switch (stateType) {
    case "started":
      return { icon: Icon.CircleProgress25, color: Color.Blue, label: "Started" };
    case "completed":
      return { icon: Icon.Checkmark, color: Color.Green, label: "Completed" };
    case "canceled":
      return { icon: Icon.Xmark, color: Color.SecondaryText, label: "Canceled" };
    case "backlog":
      return { icon: Icon.Circle, color: Color.SecondaryText, label: "Backlog" };
  }
}

// ---- Event actor: distinguish user / sync / system ----

export function actorVisual(actor: EventActor): { icon: Icon; color: Color; label: string } {
  switch (actor) {
    case "user":
      return { icon: Icon.Person, color: Color.Blue, label: "User" };
    case "sync":
      return { icon: Icon.Repeat, color: Color.Purple, label: "Sync" };
    case "system":
      return { icon: Icon.Gear, color: Color.SecondaryText, label: "System" };
  }
}

// ---- Link marks ----

const RELATION_MARK: Record<LinkRelation, string> = {
  reviews: "reviews",
  implements: "implements",
  references: "references",
};

/** Short PR badge: `owner/repo#number` plus relation. */
export function prBadge(owner: string, repo: string, number: number, relation?: LinkRelation): string {
  const base = `${owner}/${repo}#${number}`;
  return relation ? `${base} · ${RELATION_MARK[relation]}` : base;
}

/** Format an ISO timestamp as a short local time string. */
export function shortTime(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

/** Compact relative time ("2h ago", "just now"). */
export function relativeTime(iso: string | null): string {
  if (!iso) return "never";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  const diff = Date.now() - d.getTime();
  const s = Math.round(diff / 1000);
  if (s < 60) return "just now";
  const m = Math.round(s / 60);
  if (m < 60) return `${m}m ago`;
  const h = Math.round(m / 60);
  if (h < 24) return `${h}h ago`;
  const days = Math.round(h / 24);
  return `${days}d ago`;
}
