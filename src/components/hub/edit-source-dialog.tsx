import { Button } from "@/components/ui/button"
import { useHubStore } from "@/lib/hub/cache"
import type { Source } from "@/lib/hub/types"
import { useEffect, useState } from "react"
import { parsePluginFilter } from "./add-source-dialog"

interface EditSourceDialogProps {
  source: Source
  onClose: () => void
}

export function EditSourceDialog({ source, onClose }: EditSourceDialogProps) {
  const [label, setLabel] = useState(source.label)
  const [branch, setBranch] = useState(source.branch ?? "")
  const [pluginFilterText, setPluginFilterText] = useState(
    (source.pluginFilter ?? []).join(", "),
  )
  const [submitting, setSubmitting] = useState(false)
  const updateSource = useHubStore((s) => s.updateSource)

  // Sync local state if the source prop changes (defensive — guards against
  // stale form data if the parent reuses the dialog).
  useEffect(() => {
    setLabel(source.label)
    setBranch(source.branch ?? "")
    setPluginFilterText((source.pluginFilter ?? []).join(", "))
  }, [source])

  const trimmedLabel = label.trim()
  const pluginFilter = parsePluginFilter(pluginFilterText)
  const trimmedBranch = branch.trim()
  // Disabled if nothing changed compared to the source snapshot.
  const labelChanged = trimmedLabel !== source.label
  const branchChanged = trimmedBranch !== (source.branch ?? "")
  const filterChanged =
    JSON.stringify(pluginFilter ?? []) !==
    JSON.stringify(source.pluginFilter ?? [])
  const dirty = labelChanged || branchChanged || filterChanged
  const canSubmit = trimmedLabel.length > 0 && dirty && !submitting

  async function submit() {
    if (!canSubmit) return
    setSubmitting(true)
    try {
      const updated = await updateSource(source.id, {
        label: trimmedLabel,
        branch: trimmedBranch === "" ? null : trimmedBranch,
        pluginFilter,
      })
      if (updated) {
        onClose()
      }
      // else: store.error is set; leave dialog open so user can fix
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <div
      data-testid="hub-edit-source-dialog"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/40"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose()
      }}
    >
      <div className="w-full max-w-md space-y-3 rounded-md border border-border bg-card p-5 shadow-lg">
        <h2 className="text-base font-semibold">Edit Source</h2>
        <label className="block">
          <span className="text-xs text-muted-foreground">URL</span>
          <input
            data-testid="hub-edit-source-url"
            className="mt-1 w-full cursor-not-allowed rounded-sm border border-input bg-muted px-2 py-1.5 text-sm text-muted-foreground focus-visible:outline-none"
            value={source.url}
            disabled
            readOnly
          />
        </label>
        <label className="block">
          <span className="text-xs text-muted-foreground">Label</span>
          <input
            data-testid="hub-edit-source-label"
            className="mt-1 w-full rounded-sm border border-input bg-background px-2 py-1.5 text-sm focus-visible:border-ring focus-visible:outline-none"
            value={label}
            onChange={(e) => setLabel(e.target.value)}
            autoFocus
          />
        </label>
        <label className="block">
          <span className="text-xs text-muted-foreground">
            Branch (optional)
          </span>
          <input
            data-testid="hub-edit-source-branch"
            className="mt-1 w-full rounded-sm border border-input bg-background px-2 py-1.5 text-sm focus-visible:border-ring focus-visible:outline-none"
            placeholder="main"
            value={branch}
            onChange={(e) => setBranch(e.target.value)}
          />
        </label>
        <label className="block">
          <span className="text-xs text-muted-foreground">
            Plugins (optional)
          </span>
          <input
            data-testid="hub-edit-source-plugin-filter"
            className="mt-1 w-full rounded-sm border border-input bg-background px-2 py-1.5 text-sm focus-visible:border-ring focus-visible:outline-none"
            placeholder="openrouter, claude, foo-bar"
            value={pluginFilterText}
            onChange={(e) => setPluginFilterText(e.target.value)}
          />
          <span className="mt-1 block text-[10px] text-muted-foreground">
            Comma or space separated. Leave empty to show all plugins.
          </span>
        </label>
        <div className="flex justify-end gap-2 pt-2">
          <Button variant="ghost" size="sm" onClick={onClose}>
            Cancel
          </Button>
          <Button
            data-testid="hub-edit-source-submit"
            size="sm"
            disabled={!canSubmit}
            onClick={submit}
          >
            {submitting ? "Saving…" : "Save"}
          </Button>
        </div>
      </div>
    </div>
  )
}
