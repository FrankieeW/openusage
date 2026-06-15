import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { initHubSubscriptions, useHubStore } from "@/lib/hub/cache"
import { labelForError } from "@/lib/hub/labels"
import { AlertCircle, Plus } from "lucide-react"
import { useEffect, useState } from "react"
import { SourceCard } from "@/components/hub/source-card"
import { AddSourceDialog } from "@/components/hub/add-source-dialog"
import { PluginBrowser } from "@/components/hub/plugin-card"

export function HubPage() {
  const sources = useHubStore((s) => s.sources)
  const error = useHubStore((s) => s.error)
  const clearError = useHubStore((s) => s.clearError)
  const refreshSources = useHubStore((s) => s.refreshSources)
  const localPlugins = useHubStore((s) => s.localPlugins)
  const [adding, setAdding] = useState(false)

  useEffect(() => {
    refreshSources()
    void initHubSubscriptions()
  }, [refreshSources])

  return (
    <div className="space-y-4 p-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-semibold">Community Collection</h1>
          <p className="text-xs text-muted-foreground">
            Browse the Community Collection to discover and install plugins.
          </p>
        </div>
        <Button
          size="sm"
          onClick={() => setAdding(true)}
          data-testid="hub-add-source-button"
        >
          <Plus size={14} />
          Add Source
        </Button>
      </div>

      {error && (
        <Alert variant="destructive" data-testid="hub-error-alert">
          <AlertCircle size={14} />
          <AlertTitle>Hub error</AlertTitle>
          <AlertDescription className="flex items-start justify-between gap-3">
            <span>{labelForError(error)}</span>
            <Button size="xs" variant="ghost" onClick={clearError}>
              Dismiss
            </Button>
          </AlertDescription>
        </Alert>
      )}

      {sources.length === 0 && localPlugins.length === 0 && (
        <div
          data-testid="hub-empty-state"
          className="rounded-md border border-dashed border-border bg-muted/30 px-6 py-12 text-center"
        >
          <p className="text-sm text-muted-foreground">
            No sources yet. Add a GitHub repo to browse plugins.
          </p>
        </div>
      )}

      {localPlugins.length > 0 && (
        <section className="rounded-md border border-border bg-card">
          <header className="flex items-center gap-3 px-4 py-3">
            <span className="text-sm font-medium">Local</span>
            <span className="rounded bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground">
              Existing
            </span>
            <span className="text-xs text-muted-foreground">
              ({localPlugins.length} plugin{localPlugins.length === 1 ? "" : "s"})
            </span>
          </header>
          <PluginBrowser
            sourceId="_local_"
            available={localPlugins}
            skipped={[]}
          />
        </section>
      )}

      <div className="space-y-3">
        {sources.map((source) => (
          <SourceCard key={source.id} source={source} />
        ))}
      </div>

      {adding && <AddSourceDialog onClose={() => setAdding(false)} />}
    </div>
  )
}