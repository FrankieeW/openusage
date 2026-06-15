import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import { useHubStore } from "@/lib/hub/cache"
import { labelForSourceKind } from "@/lib/hub/labels"
import type { Source } from "@/lib/hub/types"
import {
  ChevronDown,
  ChevronUp,
  RefreshCw,
  Settings,
  Trash2,
} from "lucide-react"
import { useState } from "react"
import { PluginBrowser } from "./plugin-card"

interface SourceCardProps {
  source: Source
  defaultExpanded?: boolean
}

export function SourceCard({ source, defaultExpanded = false }: SourceCardProps) {
  const [expanded, setExpanded] = useState(defaultExpanded)
  const [confirmingDelete, setConfirmingDelete] = useState(false)
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
        (highlighted ? "ring-2 ring-primary " : "") +
        "border-border"
      }
      ref={(el) => {
        if (highlighted && el) {
          el.scrollIntoView({ block: "nearest", behavior: "smooth" })
          // Clear highlight once scrolled into view
          setHighlighted(null)
        }
      }}
    >
      <header className="flex items-center gap-3 px-4 py-3">
        <button
          type="button"
          onClick={toggle}
          className="flex flex-1 items-center gap-2 text-left"
          data-testid="hub-source-toggle"
        >
          {expanded ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
          <span className="truncate text-sm font-medium">{source.label}</span>
          <Badge variant="secondary" className="text-[10px]">
            {labelForSourceKind(source.kind)}
          </Badge>
          {view && (
            <span className="text-xs text-muted-foreground">
              ({view.available.length})
            </span>
          )}
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
            <RefreshCw size={12} />
          </Button>
          <Button
            size="xs"
            variant="ghost"
            onClick={() => setConfirmingDelete(true)}
            aria-label="Delete source"
            data-testid="hub-source-delete"
          >
            <Trash2 size={12} />
          </Button>
          <span title="Settings coming soon" className="text-muted-foreground">
            <Settings size={12} className="opacity-30" />
          </span>
        </div>
      </header>

      {expanded && view && <PluginBrowser
        sourceId={source.id}
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
    </section>
  )
}