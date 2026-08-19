import { Routes, Route, Navigate } from "react-router-dom"
import { Toaster } from "@/components/ui/sonner"
import { Sidebar } from "@/components/sidebar"
import { TodayView } from "@/views/today"
import { BacklogView } from "@/views/backlog"
import { InboxView } from "@/views/inbox"
import { ReviewView } from "@/views/review"
import { TodoDetailView } from "@/views/todo-detail"
import { SyncView } from "@/views/sync"

export default function App() {
  return (
    <div className="dark flex h-screen overflow-hidden bg-background text-foreground">
      <Sidebar />
      <main className="flex-1 overflow-y-auto">
        <Routes>
          <Route path="/" element={<Navigate to="/today" replace />} />
          <Route path="/today" element={<TodayView />} />
          <Route path="/backlog" element={<BacklogView />} />
          <Route path="/inbox" element={<InboxView />} />
          <Route path="/review" element={<ReviewView />} />
          <Route path="/todo/:id" element={<TodoDetailView />} />
          <Route path="/sync" element={<SyncView />} />
        </Routes>
      </main>
      <Toaster position="bottom-right" />
    </div>
  )
}
