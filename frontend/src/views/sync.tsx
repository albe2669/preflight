// Sync status: last sync per source, cursor, manual trigger, no-token state.

import { useState } from "react"
import { toast } from "sonner"
import { GitPullRequest, RefreshCw, AlertTriangle, CheckCircle2 } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import { EmptyState, LoadingRow, ErrorBanner } from "@/components/primitives"
import { useSyncStates, useSyncGithub, useSyncLinear } from "@/hooks/use-data"
import { relTime } from "@/lib/status"
import { cn } from "@/lib/utils"
import type { SyncState } from "@/types"

export function SyncView() {
  const sync = useSyncStates()
  const syncGithub = useSyncGithub()
  const syncLinear = useSyncLinear()
  const [dismissedErr, setDismissedErr] = useState(false)

  const states = sync.data ?? []
  const github = states.find((s) => s.source === "github")
  const linear = states.find((s) => s.source === "linear")

  const error = sync.error && !dismissedErr ? String((sync.error as Error).message) : null

  const handleGithub = () => {
    syncGithub.mutateAsync()
      .then(() => toast.success("GitHub synced"))
      .catch(() => toast.error("Could not sync GitHub"))
  }
  const handleLinear = () => {
    syncLinear.mutateAsync()
      .then(() => toast.success("Linear synced"))
      .catch(() => toast.error("Could not sync Linear"))
  }

  return (
    <div className="mx-auto max-w-2xl px-6 py-8">
      <h1 className="mb-6 text-lg font-semibold">Sync</h1>

      {error && <ErrorBanner message={error} onDismiss={() => setDismissedErr(true)} />}

      {sync.isLoading && <LoadingRow label="Loading sync state…" />}

      {!sync.isLoading && !error && states.length === 0 && (
        <EmptyState message="No sync sources configured." />
      )}

      {states.length > 0 && (
        <div className="space-y-4">
          <SyncCard
            source="github"
            state={github}
            icon={GitPullRequest}
            onSync={handleGithub}
            syncing={syncGithub.isPending}
          />
          <SyncCard
            source="linear"
            state={linear}
            icon={RefreshCw}
            onSync={handleLinear}
            syncing={syncLinear.isPending}
          />
        </div>
      )}

      <p className="mt-8 text-xs text-muted-foreground/50">
        Sync pulls pull requests and Linear issues into the inbox. Tokens may be absent —
        the app works without them; sync is a no-op.
      </p>
    </div>
  )
}

function SyncCard({
  source,
  state,
  icon: Icon,
  onSync,
  syncing,
}: {
  source: string
  state: SyncState | undefined
  icon: React.ComponentType<{ className?: string }>
  onSync: () => void
  syncing: boolean
}) {
  const noToken = state?.lastStatus === "no_token" || state?.lastStatus === "noop"
  const hasError = state?.lastStatus === "error" || !!state?.lastError
  const ok = state?.lastStatus === "ok"

  return (
    <div className="rounded-lg border border-border bg-card p-4">
      <div className="flex items-center gap-3">
        <Icon className="size-5 text-muted-foreground" />
        <h2 className="text-sm font-medium capitalize">{source}</h2>
        <div className="ml-auto flex items-center gap-2">
          {noToken && <Badge variant="outline" className="text-[10px] text-muted-foreground">No token</Badge>}
          {hasError && (
            <Badge variant="outline" className="gap-1 text-[10px] text-warning">
              <AlertTriangle className="size-2.5" />Error
            </Badge>
          )}
          {ok && (
            <Badge variant="outline" className="gap-1 text-[10px] text-success">
              <CheckCircle2 className="size-2.5" />OK
            </Badge>
          )}
          <Button variant="outline" size="sm" onClick={onSync} disabled={syncing || noToken}>
            <RefreshCw className={cn("size-3.5", syncing && "animate-spin")} />
            {syncing ? "Syncing…" : "Sync now"}
          </Button>
        </div>
      </div>

      <div className="mt-3 space-y-1 text-xs text-muted-foreground">
        {state ? (
          <>
            <div className="flex justify-between">
              <span>Last sync</span>
              <span>{state.lastSyncedAt ? relTime(state.lastSyncedAt) : "never"}</span>
            </div>
            <div className="flex justify-between">
              <span>Status</span>
              <span className={cn(
                hasError && "text-warning",
                ok && "text-success",
                noToken && "text-muted-foreground/60",
              )}>
                {state.lastStatus}
              </span>
            </div>
            {state.cursor && (
              <div className="flex justify-between">
                <span>Cursor</span>
                <span className="font-mono text-[10px]">{state.cursor.slice(0, 16)}…</span>
              </div>
            )}
            {state.lastError && (
              <div className="mt-2 rounded-md bg-warning/10 px-2 py-1.5 text-warning">
                {state.lastError}
              </div>
            )}
          </>
        ) : (
          <p>No state recorded.</p>
        )}
      </div>
    </div>
  )
}
