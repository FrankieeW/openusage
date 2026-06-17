import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { initHubSubscriptions, useHubStore } from "@/lib/hub/cache"
import { labelForError } from "@/lib/hub/labels"
import { useEffect, useState } from "react"
import { SourceCard } from "@/components/hub/source-card"
import { AddSourceDialog } from "@/components/hub/add-source-dialog"
import { PluginBrowser } from "@/components/hub/plugin-card"
import type { SkippedPlugin } from "@/lib/hub/types"

export function HubPage() {
  const sources = useHubStore((s) => s.sources)
  const error = useHubStore((s) => s.error)
  const clearError = useHubStore((s) => s.clearError)
  const refreshSources = useHubStore((s) => s.refreshSources)
  const localPlugins = useHubStore((s) => s.localPlugins)
  const [adding, setAdding] = useState(false)
  const skippedReasons = skippedReasonsFromError(error)

  useEffect(() => {
    refreshSources()
    void initHubSubscriptions()
  }, [refreshSources])

  return (
    <div className="py-3 px-1 space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-lg font-semibold mb-0">Community Collection</h1>
          <p className="text-sm text-muted-foreground mb-1">
            Browse the Community Collection to discover and install plugins.
          </p>
        </div>
        <div className="flex flex-col gap-2">
          <Button
            size="sm"
            variant="secondary"
            onClick={() => window.location.reload()}
            data-testid="hub-reload-button"
          >
            Reload
          </Button>
          <Button
            size="sm"
            onClick={() => setAdding(true)}
            data-testid="hub-add-source-button"
          >
            Add Source
          </Button>
        </div>
      </div>

      {error && (
        <Alert variant="destructive" data-testid="hub-error-alert">
          <AlertTitle>Hub Error</AlertTitle>
          <AlertDescription className="space-y-2">
            <div className="flex items-start justify-between gap-3">
              <span>{labelForError(error)}</span>
              <Button size="xs" variant="ghost" onClick={clearError}>
                Dismiss
              </Button>
            </div>
            {skippedReasons.length > 0 && (
              <details open data-testid="hub-error-skipped-reasons">
                <summary className="cursor-pointer text-xs">
                  Skipped Plugin Reasons
                </summary>
                <ul className="mt-2 list-disc pl-4 text-xs">
                  {skippedReasons.map((skipped) => (
                    <li key={`${skipped.path}:${skipped.reason}`}>
                      <code>{skipped.path.split("/").slice(-2).join("/")}</code>
                      : {skipped.reason}
                    </li>
                  ))}
                </ul>
              </details>
            )}
          </AlertDescription>
        </Alert>
      )}

      {sources.length === 0 && localPlugins.length === 0 && (
        <div
          data-testid="hub-empty-state"
          className="bg-muted/50 rounded-lg border border-dashed/30 px-6 py-12 text-center"
        >
          <p className="text-sm text-muted-foreground">
            No sources yet. Add a GitHub repo to browse plugins.
          </p>
        </div>
      )}

      {localPlugins.length > 0 && (
        <section className="rounded-md border bg-card">
          <header className="flex items-center gap-3 py-2">
            <span className="text-sm font-medium">Local</span>
            <span className="rounded bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground">
              Existing
            </span>
            <span className="text-sm text-muted-foreground mb-1">
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

function skippedReasonsFromError(
  error: ReturnType<typeof useHubStore.getState>["error"],
): SkippedPlugin[] {
  const skipped = error?.context?.skipped
  if (!Array.isArray(skipped)) return []
  return skipped.filter(isSkippedPlugin)
}

function isSkippedPlugin(value: unknown): value is SkippedPlugin {
  if (typeof value !== "object" || value === null) return false
  const record = value as Record<string, unknown>
  return typeof record.path === "string" && typeof record.reason === "string"
}
