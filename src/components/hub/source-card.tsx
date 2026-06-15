import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import { useHubStore } from "@/lib/hub/cache"
import { labelForSourceKind } from "@/lib/hub/labels"
import { DEFAULT_HUB_ID, type Source } from "@/lib/hub/types"
import {
  ChevronDown,
  ChevronUp,
  Filter,
  RefreshCw,
  Trash2,
} from "lucide-react"
import { useEffect, useMemo, useState } from "react"
import { PluginBrowser } from "./plugin-card"

interface SourceCardProps {
  source: Source
  defaultExpanded?: boolean
}

export function SourceCard({ source, defaultExpanded = false }: SourceCardProps) {
  const [expanded, setExpanded] = useState(defaultExpanded)
  const [confirmingDelete, setConfirmingDelete] = useState(false)
  const [dedup, setDedup] = useState(false)
  const browseBySource = useHubStore((s) => s.browseBySource)
  const browseSource = useHubStore((s) => s.browseSource)
  const refreshSource = useHubStore((s) => s.refreshSource)
  const removeSource = useHubStore((s) => s.removeSource)
  const setHighlighted = useHubStore((s) => s.setHighlightedSource)
  const highlighted = useHubStore((s) => s.highlightedSourceId === source.id)
  const officialSourceId = useHubStore(
    (s) => s.sources.find((src) => src.id === DEFAULT_HUB_ID)?.id ?? null,
  )

  const view = browseBySource[source.id]
  const refreshing = useHubStore(
    (s) => s.loading.perSource[source.id] === true,
  )

  // The dedup toggle only makes sense for a custom source when an official
  // source exists to compare against.
  const isOfficial = source.id === DEFAULT_HUB_ID
  const canDedup = !isOfficial && officialSourceId !== null
  const officialView = officialSourceId ? browseBySource[officialSourceId] : undefined

  // Lazily load the official source's plugins the first time dedup is enabled
  // so we have something to compare against.
  useEffect(() => {
    if (dedup && officialSourceId && !officialView) {
      void browseSource(officialSourceId)
    }
  }, [dedup, officialSourceId, officialView, browseSource])

  const officialPluginIds = useMemo(
    () => new Set(officialView?.available.map((p) => p.id) ?? []),
    [officialView],
  )

  // When deduping, hide plugins the official source already provides.
  const displayedAvailable = useMemo(() => {
    if (!view) return []
    if (!dedup || !canDedup) return view.available
    return view.available.filter((p) => !officialPluginIds.has(p.id))
  }, [view, dedup, canDedup, officialPluginIds])

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
          className="flex w-full items-center gap-2 text-left"
          data-testid="hub-source-toggle"
        >
          {expanded ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
          <span className="truncate text-sm font-medium">{source.label}</span>
          <Badge variant="secondary" className="text-[10px]">
            {labelForSourceKind(source.kind)}
          </Badge>
          {view && (
            <span className="text-xs text-muted-foreground">
              ({displayedAvailable.length})
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
            <span className="ml-1">Refresh</span>
          </Button>
          {canDedup && (
            <Button
              size="xs"
              variant={dedup ? "secondary" : "ghost"}
              onClick={() => setDedup((d) => !d)}
              aria-pressed={dedup}
              aria-label={dedup ? "Show all plugins" : "Show only plugins not in the official source"}
              data-testid="hub-source-dedup"
            >
              <Filter size={12} />
              <span className="ml-1 inline-block w-[52px] text-left">
                {dedup ? "Show all" : "Dedupe"}
              </span>
            </Button>
          )}
          <Button
            size="xs"
            variant="ghost"
            onClick={() => setConfirmingDelete(true)}
            aria-label="Delete source"
            data-testid="hub-source-delete"
          >
            <Trash2 size={12} />
            <span className="ml-1">Delete</span>
          </Button>
        </div>
      </header>

      {expanded && view && <PluginBrowser
        sourceId={source.id}
        available={displayedAvailable}
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