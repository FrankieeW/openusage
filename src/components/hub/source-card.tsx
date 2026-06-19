import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import { useHubStore } from "@/lib/hub/cache"
import {
  labelForSourceKind,
  labelForSourceTrust,
} from "@/lib/hub/labels"
import type { Source } from "@/lib/hub/types"
import { useState } from "react"
import { EditSourceDialog } from "./edit-source-dialog"
import { PluginBrowser } from "./plugin-card"

interface SourceCardProps {
  source: Source
  defaultExpanded?: boolean
}

export function SourceCard({ source, defaultExpanded = false }: SourceCardProps) {
  const [expanded, setExpanded] = useState(defaultExpanded)
  const [confirmingDelete, setConfirmingDelete] = useState(false)
  const [editing, setEditing] = useState(false)
  const browseBySource = useHubStore((s) => s.browseBySource)
  const browseSource = useHubStore((s) => s.browseSource)
  const refreshSource = useHubStore((s) => s.refreshSource)
  const removeSource = useHubStore((s) => s.removeSource)
  const setHighlighted = useHubStore((s) => s.setHighlightedSource)
  const highlighted = useHubStore((s) => s.highlightedSourceId === source.id)

  const view = browseBySource[source.id]
  const refreshing = useHubStore(
    (s) => s.loading.perSource[source.id] === true,
  )

  async function toggle() {
    const next = !expanded
    setExpanded(next)
    if (next && !view) {
      await browseSource(source.id)
    }
  }

  return (
    <section
      data-testid="hub-source-card"
      data-source-id={source.id}
      className={
        "rounded-md border bg-card " +
        (highlighted ? "ring-2 ring-primary " : "")
      }
      ref={(el) => {
        if (highlighted && el) {
          el.scrollIntoView({ block: "nearest", behavior: "smooth" })
          // Clear highlight once scrolled into view
          setHighlighted(null)
        }
      }}
    >
      <header className="px-4 py-3 space-y-2">
        <button
          type="button"
          onClick={toggle}
          className="flex w-full flex-col gap-1 text-left sm:flex-row sm:items-center sm:gap-2"
          data-testid="hub-source-toggle"
        >
          <span className="flex min-w-0 flex-1 items-center gap-2">
            <span className="truncate text-sm font-medium" title={source.label}>
              {source.label}
            </span>
            {view && (
              <span className="shrink-0 text-xs text-muted-foreground">
                ({view.available.length})
              </span>
            )}
          </span>
          <span className="flex shrink-0 flex-wrap gap-1">
            <Badge variant="secondary" className="text-[10px]">
              {labelForSourceKind(source.kind)}
            </Badge>
            <Badge variant="outline" className="text-[10px]">
              {labelForSourceTrust(source)}
            </Badge>
          </span>
        </button>
        <div className="flex items-center gap-1">
          <Button
            size="xs"
            variant="ghost"
            onClick={() => refreshSource(source.id)}
            disabled={refreshing}
            aria-label="Refresh source"
            data-testid="hub-source-refresh"
          >
            Refresh
          </Button>
          <Button
            size="xs"
            variant="ghost"
            onClick={() => setEditing(true)}
            aria-label="Edit source"
            data-testid="hub-source-edit"
          >
            Edit
          </Button>
          <Button
            size="xs"
            variant="ghost"
            onClick={() => setConfirmingDelete(true)}
            aria-label="Delete source"
            data-testid="hub-source-delete"
          >
            Delete
          </Button>
        </div>
        {expanded && view && (
          <div
            data-testid="hub-source-snapshot"
            className="flex flex-wrap gap-2 text-xs text-muted-foreground"
          >
            <span>Branch {view.snapshot.branch ?? "Default"}</span>
            {view.snapshot.commitSha && (
              <span>Commit {view.snapshot.commitSha.slice(0, 12)}</span>
            )}
            <span>{view.snapshot.discoveredCount} Discovered</span>
            <span>{view.snapshot.skippedCount} Skipped</span>
            <span>Refreshed {formatSnapshotTime(view.snapshot.checkedAt)}</span>
          </div>
        )}
      </header>

      {expanded && view && <PluginBrowser
        sourceId={source.id}
        source={view.source}
        available={view.available}
        skipped={view.skipped}
      />}

      {expanded && !view && (
        <div className="px-4 pb-3 text-sm text-muted-foreground">Loading…</div>
      )}

      {confirmingDelete && (
        <div
          data-testid="hub-confirm-delete"
          className="flex items-center gap-2 border-t border-border bg-muted px-4 py-2 text-sm"
        >
          <span>Delete source? Installed plugins stay.</span>
          <Button
            size="xs"
            variant="destructive"
            onClick={async () => {
              setConfirmingDelete(false)
              await removeSource(source.id)
            }}
            data-testid="hub-confirm-delete-yes"
          >
            Delete
          </Button>
          <Button
            size="xs"
            variant="ghost"
            onClick={() => setConfirmingDelete(false)}
            data-testid="hub-confirm-delete-no"
          >
            Cancel
          </Button>
        </div>
      )}
      {editing && (
        <EditSourceDialog
          source={source}
          onClose={() => setEditing(false)}
        />
      )}
    </section>
  )
}

function formatSnapshotTime(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return "Unknown"
  return new Date(value).toLocaleString()
}
