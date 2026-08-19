// App shell: sidebar navigation + content area. Calm, dense, monochrome.

import { NavLink } from "react-router-dom"
import {
  Inbox,
  ListTodo,
  RefreshCw,
  CalendarDays,
  LayoutGrid,
} from "lucide-react"
import { cn } from "@/lib/utils"

const NAV = [
  { to: "/today", label: "Today", icon: ListTodo },
  { to: "/backlog", label: "Backlog", icon: LayoutGrid },
  { to: "/inbox", label: "Inbox", icon: Inbox },
  { to: "/review", label: "Review", icon: CalendarDays },
  { to: "/sync", label: "Sync", icon: RefreshCw },
]

export function Sidebar() {
  return (
    <nav className="flex w-14 flex-col items-center gap-1 border-r border-border py-4 md:w-44 md:items-stretch md:px-2">
      <div className="mb-4 px-2 font-mono text-sm font-semibold tracking-tight text-foreground">
        <span className="md:hidden">p</span>
        <span className="hidden md:inline">preflight</span>
      </div>
      {NAV.map((item) => (
        <NavLink
          key={item.to}
          to={item.to}
          className={({ isActive }) =>
            cn(
              "flex items-center gap-3 rounded-md px-2.5 py-2 text-sm transition-colors",
              isActive
                ? "bg-secondary text-foreground"
                : "text-muted-foreground hover:bg-secondary/50 hover:text-foreground",
            )
          }
        >
          <item.icon className="size-4 shrink-0" />
          <span className="hidden md:inline">{item.label}</span>
        </NavLink>
      ))}
      <div className="mt-auto hidden px-2 text-[10px] text-muted-foreground/50 md:block">
        calm · focused
      </div>
    </nav>
  )
}
