"use client"

import { useState } from "react"
import { Sidebar, type ViewId } from "@/components/sidebar"
import { Topbar } from "@/components/topbar"
import { LogsPanel } from "@/components/logs-panel"
import { DashboardView } from "@/components/views/dashboard-view"
import { UpdateView } from "@/components/views/update-view"
import { PodsView } from "@/components/views/pods-view"
import { UsersView } from "@/components/views/users-view"
import { AdminView } from "@/components/views/admin-view"
import { WelcomeView } from "@/components/views/welcome-view"
import { TasksView } from "@/components/views/tasks-view"

export default function Page() {
  const [view, setView] = useState<ViewId>("dashboard")
  const [logsOpen, setLogsOpen] = useState(true)

  return (
    <div className="flex h-screen w-full overflow-hidden bg-background text-foreground">
      <Sidebar active={view} onSelect={setView} />

      <div className="flex min-w-0 flex-1 flex-col">
        <Topbar view={view} logsOpen={logsOpen} onToggleLogs={() => setLogsOpen((o) => !o)} />
        <main className="flex-1 overflow-y-auto">
          {view === "dashboard" && <DashboardView />}
          {view === "update" && <UpdateView />}
          {view === "pods" && <PodsView />}
          {view === "users" && <UsersView />}
          {view === "admin" && <AdminView />}
          {view === "welcome" && <WelcomeView />}
          {view === "tasks" && <TasksView />}
        </main>
      </div>

      <LogsPanel open={logsOpen} onToggle={() => setLogsOpen((o) => !o)} />
    </div>
  )
}
