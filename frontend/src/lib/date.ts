// Display formatting for date strings. The logical date itself comes from the
// server's clock query; this module only formats and shifts dates for the UI.

const DEFAULT_DAY_START_HOUR = 4

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
export function dateLabel(dateStr: string, dayStartHour: number = DEFAULT_DAY_START_HOUR): string {
  const d = new Date(dateStr + "T00:00:00")
  const weekday = d.toLocaleDateString("en-US", { weekday: "long" })
  return `${weekday} · ${dateStr} · day ${String(dayStartHour).padStart(2, "0")}:00`
}
