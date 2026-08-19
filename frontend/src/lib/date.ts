// Logical date computation. A logical day starts at day_start_hour (default 4),
// not midnight. Work at 02:00 counts as yesterday.

const DAY_START_HOUR = 4

/** YYYY-MM-DD for the current logical date in the browser's local zone. */
export function logicalDate(now: Date = new Date()): string {
  const d = new Date(now)
  // Roll back day_start_hour hours: if local hour < boundary, use yesterday.
  d.setHours(d.getHours() - DAY_START_HOUR)
  return toDateStr(d)
}

/** YYYY-MM-DD for an arbitrary date. */
export function toDateStr(d: Date): string {
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, "0")
  const day = String(d.getDate()).padStart(2, "0")
  return `${y}-${m}-${day}`
}

/** Date shifted by n days, as YYYY-MM-DD. */
export function shiftDate(dateStr: string, days: number): string {
  const d = new Date(dateStr + "T00:00:00")
  d.setDate(d.getDate() + days)
  return toDateStr(d)
}

/** Human label: "Monday · 2026-08-19 · day 04:00". */
export function dateLabel(dateStr: string): string {
  const d = new Date(dateStr + "T00:00:00")
  const weekday = d.toLocaleDateString("en-US", { weekday: "long" })
  return `${weekday} · ${dateStr} · day ${String(DAY_START_HOUR).padStart(2, "0")}:00`
}

export { DAY_START_HOUR }
