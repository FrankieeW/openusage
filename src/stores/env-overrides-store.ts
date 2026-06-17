import { create } from "zustand"
import { invoke, isTauri } from "@tauri-apps/api/core"
import {
  loadEnvGroups,
  saveEnvGroups,
  parseValueInput,
  type EnvGroup,
  type EnvOverride,
  type EnvOverrideKind,
} from "@/lib/env-overrides"

export type FlattenedOverride = {
  name: string
  kind: EnvOverrideKind
  value: string
}

type EnvOverridesStore = {
  groups: EnvGroup[]
  loaded: boolean

  init: () => Promise<void>

  addGroup: () => void
  updateGroup: (groupId: string, patch: Partial<Omit<EnvGroup, "id" | "overrides">>) => void
  removeGroup: (groupId: string) => void

  addOverride: (groupId: string) => void
  updateOverride: (groupId: string, index: number, patch: Partial<EnvOverride>) => void
  removeOverride: (groupId: string, index: number) => void

  flattened: () => FlattenedOverride[]

  saveAndReload: () => Promise<number | null>
}

const CONFLICT_PREFIX = "[CONFLICT: "

function makeId(): string {
  return `g_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 8)}`
}

function flattenGroups(groups: EnvGroup[]): FlattenedOverride[] {
  const byName = new Map<string, FlattenedOverride>()
  for (const group of groups) {
    if (!group.enabled) continue
    for (const o of group.overrides) {
      const parsed = parseValueInput(o.value)
      if (parsed.kind === "literal" && parsed.value.length === 0) continue
      const existing = byName.get(o.name)
      if (existing) {
        byName.set(o.name, {
          name: o.name,
          kind: "literal",
          value: `${CONFLICT_PREFIX}${o.name}]`,
        })
      } else {
        byName.set(o.name, { name: o.name, kind: parsed.kind, value: parsed.value })
      }
    }
  }
  return Array.from(byName.values())
}

// ---------------------------------------------------------------------------
// Debounced persistence to env.json  +  immediate push to Rust
// ---------------------------------------------------------------------------

let persistTimer: ReturnType<typeof setTimeout> | null = null
let pendingSnapshot: EnvGroup[] | null = null

/** Push flattened overrides to the Rust plugin engine immediately. */
async function pushToRust(groups: EnvGroup[]): Promise<void> {
  if (!isTauri()) return
  const flattened = flattenGroups(groups)
  try {
    await invoke("set_env_overrides", { overrides: flattened })
  } catch (e) {
    console.error("Failed to sync env overrides to Rust:", e)
  }
}

function scheduleSync(groups: EnvGroup[]): void {
  pendingSnapshot = groups
  // Push to Rust immediately so plugins see changes right away.
  void pushToRust(groups)

  if (persistTimer !== null) clearTimeout(persistTimer)
  persistTimer = setTimeout(() => {
    const toSync = pendingSnapshot
    pendingSnapshot = null
    persistTimer = null
    if (!toSync) return
    void (async () => {
      try {
        await saveEnvGroups(toSync)
      } catch (e) {
        console.error("Failed to save env groups:", e)
      }
    })()
  }, 250)
}

function syncNow(groups: EnvGroup[]): Promise<void> {
  if (persistTimer !== null) {
    clearTimeout(persistTimer)
    persistTimer = null
    pendingSnapshot = null
  }
  return (async () => {
    await pushToRust(groups)
    try {
      await saveEnvGroups(groups)
    } catch (e) {
      console.error("Failed to save env groups:", e)
    }
  })()
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

export const useEnvOverridesStore = create<EnvOverridesStore>((set, get) => ({
  groups: [],
  loaded: false,

  init: async () => {
    if (get().loaded) return
    try {
      const groups = await loadEnvGroups()
      set({ groups, loaded: true })
      // Push to Rust on first load (belt-and-suspenders with the cold-start
      // path in Rust that reads env.json directly).
      await pushToRust(groups)
    } catch (e) {
      console.error("Failed to load env overrides:", e)
      set({ loaded: true })
    }
  },

  addGroup: () => {
    const next: EnvGroup[] = [
      ...get().groups,
      { id: makeId(), name: "New Group", enabled: true, overrides: [] },
    ]
    set({ groups: next })
    scheduleSync(next)
  },

  updateGroup: (groupId, patch) => {
    const next = get().groups.map((g) => (g.id === groupId ? { ...g, ...patch } : g))
    set({ groups: next })
    scheduleSync(next)
  },

  removeGroup: (groupId) => {
    const next = get().groups.filter((g) => g.id !== groupId)
    set({ groups: next })
    scheduleSync(next)
  },

  addOverride: (groupId) => {
    const next = get().groups.map((g) =>
      g.id === groupId ? { ...g, overrides: [...g.overrides, { name: "", value: "" }] } : g
    )
    set({ groups: next })
    scheduleSync(next)
  },

  updateOverride: (groupId, index, patch) => {
    const next = get().groups.map((g) => {
      if (g.id !== groupId) return g
      return {
        ...g,
        overrides: g.overrides.map((o, i) => (i === index ? { ...o, ...patch } : o)),
      }
    })
    set({ groups: next })
    scheduleSync(next)
  },

  removeOverride: (groupId, index) => {
    const next = get().groups.map((g) => {
      if (g.id !== groupId) return g
      return { ...g, overrides: g.overrides.filter((_, i) => i !== index) }
    })
    set({ groups: next })
    scheduleSync(next)
  },

  flattened: () => flattenGroups(get().groups),

  saveAndReload: async () => {
    const groups = get().groups
    await syncNow(groups)
    if (!isTauri()) return null
    try {
      const count = await invoke<number>("hub_reload_plugins")
      window.location.reload()
      return count
    } catch (e) {
      console.error("Failed to reload plugins after env save:", e)
      return null
    }
  },
}))
