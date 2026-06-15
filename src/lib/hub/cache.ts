import { create } from "zustand"

import { hubCommands } from "./commands"
import type {
  HubBrowseView,
  HubError,
  Source,
} from "./types"

// Hub zustand store.
//
// State shape:
//   - sources: the registry-backed list (one fetch on boot).
//   - browseBySource: lazy cache of HubBrowseView per source, populated when
//     the user expands a source card or after refresh.
//   - loading: per-source + per-plugin flags so the UI can show spinners.
//   - error: last error to surface as a toast.
//
// Side effects:
//   - subscribes to `plugins-changed` and `hub-orphans-detected` events on
//     init; those are emitted by the Rust side after install/uninstall/sweep.

type LoadingState = {
  sources: boolean
  perSource: Record<string, boolean>
  perPlugin: Record<string, "install" | "uninstall" | null>
}

interface HubState {
  sources: Source[]
  browseBySource: Record<string, HubBrowseView>
  loading: LoadingState
  error: HubError | null
  highlightedSourceId: string | null

  refreshSources: () => Promise<void>
  browseSource: (id: string, force?: boolean) => Promise<HubBrowseView | null>
  addSource: (url: string, label?: string) => Promise<Source | null>
  removeSource: (id: string) => Promise<void>
  refreshSource: (id: string) => Promise<HubBrowseView | null>
  install: (sourceId: string, pluginId: string) => Promise<void>
  uninstall: (pluginId: string) => Promise<void>
  clearError: () => void
  setHighlightedSource: (id: string | null) => void
}

const initialLoading: LoadingState = {
  sources: false,
  perSource: {},
  perPlugin: {},
}

function captureError(err: unknown): HubError {
  if (err && typeof err === "object" && "code" in (err as object)) {
    return err as HubError
  }
  return { code: "IoError", message: String(err) }
}

export const useHubStore = create<HubState>((set, get) => ({
  sources: [],
  browseBySource: {},
  loading: initialLoading,
  error: null,
  highlightedSourceId: null,

  refreshSources: async () => {
    set((s) => ({ loading: { ...s.loading, sources: true } }))
    try {
      const sources = await hubCommands.listSources()
      set({ sources })
    } catch (err) {
      set({ error: captureError(err) })
    } finally {
      set((s) => ({ loading: { ...s.loading, sources: false } }))
    }
  },

  browseSource: async (id, force = false) => {
    const existing = get().browseBySource[id]
    if (existing && !force) return existing
    set((s) => ({
      loading: { ...s.loading, perSource: { ...s.loading.perSource, [id]: true } },
    }))
    try {
      const view = await hubCommands.browseSource(id)
      set((s) => ({
        browseBySource: { ...s.browseBySource, [id]: view },
      }))
      return view
    } catch (err) {
      set({ error: captureError(err) })
      return null
    } finally {
      set((s) => ({
        loading: { ...s.loading, perSource: { ...s.loading.perSource, [id]: false } },
      }))
    }
  },

  addSource: async (url, label) => {
    try {
      const source = await hubCommands.addSource(url, label)
      set((s) => ({ sources: [...s.sources, source] }))
      return source
    } catch (err) {
      set({ error: captureError(err) })
      return null
    }
  },

  removeSource: async (id) => {
    try {
      await hubCommands.removeSource(id)
      set((s) => {
        const { [id]: _removed, ...rest } = s.browseBySource
        return {
          sources: s.sources.filter((src) => src.id !== id),
          browseBySource: rest,
        }
      })
    } catch (err) {
      set({ error: captureError(err) })
    }
  },

  refreshSource: async (id) => {
    set((s) => ({
      loading: { ...s.loading, perSource: { ...s.loading.perSource, [id]: true } },
    }))
    try {
      const view = await hubCommands.refreshSource(id)
      set((s) => ({
        browseBySource: { ...s.browseBySource, [id]: view },
      }))
      return view
    } catch (err) {
      set({ error: captureError(err) })
      return null
    } finally {
      set((s) => ({
        loading: { ...s.loading, perSource: { ...s.loading.perSource, [id]: false } },
      }))
    }
  },

  install: async (sourceId, pluginId) => {
    const key = `${sourceId}:${pluginId}`
    set((s) => ({
      loading: {
        ...s.loading,
        perPlugin: { ...s.loading.perPlugin, [key]: "install" },
      },
    }))
    try {
      await hubCommands.install(sourceId, pluginId)
      // The Rust side fires `plugins-changed`; we re-browse to refresh metadata
      const view = await hubCommands.browseSource(sourceId)
      set((s) => ({
        browseBySource: { ...s.browseBySource, [sourceId]: view },
      }))
    } catch (err) {
      set({ error: captureError(err) })
    } finally {
      set((s) => ({
        loading: {
          ...s.loading,
          perPlugin: { ...s.loading.perPlugin, [key]: null },
        },
      }))
    }
  },

  uninstall: async (pluginId) => {
    const key = `uninstall:${pluginId}`
    set((s) => ({
      loading: {
        ...s.loading,
        perPlugin: { ...s.loading.perPlugin, [key]: "uninstall" },
      },
    }))
    try {
      await hubCommands.uninstall(pluginId)
      // Mark uninstalled across all browsed sources
      set((s) => {
        const next: Record<string, HubBrowseView> = {}
        for (const [sid, view] of Object.entries(s.browseBySource)) {
          next[sid] = {
            ...view,
            available: view.available.map((p) =>
              p.id === pluginId
                ? { ...p, installed: false, installedSourceId: null, installedVersion: null, updateAvailable: false, unmanaged: false }
                : p,
            ),
          }
        }
        return { browseBySource: next }
      })
    } catch (err) {
      set({ error: captureError(err) })
    } finally {
      set((s) => ({
        loading: {
          ...s.loading,
          perPlugin: { ...s.loading.perPlugin, [key]: null },
        },
      }))
    }
  },

  clearError: () => set({ error: null }),

  setHighlightedSource: (id) => set({ highlightedSourceId: id }),
}))

// Helpers exported for component code.
export function pluginLoadingKey(sourceId: string, pluginId: string) {
  return `${sourceId}:${pluginId}`
}
export function uninstallLoadingKey(pluginId: string) {
  return `uninstall:${pluginId}`
}