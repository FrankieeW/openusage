import { Button } from "@/components/ui/button"
import { useHubStore } from "@/lib/hub/cache"
import { parseGithubTreeUrl } from "@/lib/hub/parse-source-url"
import { openUrl } from "@tauri-apps/plugin-opener"
import { ExternalLink } from "lucide-react"
import { useState } from "react"

const RECOMMENDED_SOURCES_URL =
  "https://github.com/FrankieeW/openusage/blob/main/docs/recommended-sources.md"

interface AddSourceDialogProps {
  onClose: () => void
}

export function AddSourceDialog({ onClose }: AddSourceDialogProps) {
  const [url, setUrl] = useState("")
  const [label, setLabel] = useState("")
  const [branch, setBranch] = useState("")
  const [submitting, setSubmitting] = useState(false)
  const addSource = useHubStore((s) => s.addSource)
  const browseSource = useHubStore((s) => s.browseSource)

  const trimmed = url.trim()
  const looksValid =
    trimmed.length > 0 &&
    (trimmed.startsWith("https://") ||
      trimmed.startsWith("git@") ||
      trimmed.startsWith("file://") ||
      /^[\w.-]+\/[\w.-]+$/.test(trimmed) ||
      trimmed.startsWith("/"))

  // Pasting a GitHub "tree" URL (e.g. .../tree/feat/warp) auto-fills the repo
  // URL, label, and branch. Label is only filled if the user hasn't typed one.
  function handleUrlChange(value: string) {
    const parsed = parseGithubTreeUrl(value)
    if (!parsed) {
      setUrl(value)
      return
    }
    setUrl(parsed.url)
    setBranch(parsed.branch)
    setLabel((prev) => (prev.trim() ? prev : parsed.label))
  }

  async function submit() {
    if (!looksValid || submitting) return
    setSubmitting(true)
    try {
      const source = await addSource(trimmed, label.trim() || undefined, branch.trim() || undefined)
      if (source) {
        await browseSource(source.id)
        onClose()
      }
      // else: store.error is set; leave dialog open so user can fix
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <div
      data-testid="hub-add-source-dialog"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/40"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose()
      }}
    >
      <div className="w-full max-w-md space-y-3 rounded-md border border-border bg-card p-5 shadow-lg">
        <h2 className="text-base font-semibold">Add Source</h2>
        <p className="text-xs text-muted-foreground">
          GitHub URL, generic git URL, local path, or{" "}
          <code>owner/repo</code> shorthand. Paste a{" "}
          <code>owner/repo/tree/branch</code> link to auto-fill the branch.
        </p>
        <label className="block">
          <span className="text-xs text-muted-foreground">URL or path</span>
          <input
            data-testid="hub-add-source-url"
            className="mt-1 w-full rounded-sm border border-input bg-background px-2 py-1.5 text-sm focus-visible:border-ring focus-visible:outline-none"
            placeholder="owner/repo or /path/to/repo"
            value={url}
            onChange={(e) => handleUrlChange(e.target.value)}
            autoFocus
          />
        </label>
        <label className="block">
          <span className="text-xs text-muted-foreground">
            Label (optional)
          </span>
          <input
            data-testid="hub-add-source-label"
            className="mt-1 w-full rounded-sm border border-input bg-background px-2 py-1.5 text-sm focus-visible:border-ring focus-visible:outline-none"
            value={label}
            onChange={(e) => setLabel(e.target.value)}
          />
        </label>
        <label className="block">
          <span className="text-xs text-muted-foreground">
            Branch (optional)
          </span>
          <input
            data-testid="hub-add-source-branch"
            className="mt-1 w-full rounded-sm border border-input bg-background px-2 py-1.5 text-sm focus-visible:border-ring focus-visible:outline-none"
            placeholder="main"
            value={branch}
            onChange={(e) => setBranch(e.target.value)}
          />
        </label>
        <div className="flex justify-end gap-2 pt-2">
          <Button variant="ghost" size="sm" onClick={onClose}>
            Cancel
          </Button>
          <Button
            data-testid="hub-add-source-submit"
            size="sm"
            disabled={!looksValid || submitting}
            onClick={submit}
          >
            {submitting ? "Adding…" : "Add & Fetch"}
          </Button>
        </div>
        <button
          type="button"
          data-testid="hub-recommended-sources-link"
          className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground"
          onClick={() => openUrl(RECOMMENDED_SOURCES_URL).catch(console.error)}
        >
          <ExternalLink size={12} />
          Browse recommended sources
        </button>
      </div>
    </div>
  )
}