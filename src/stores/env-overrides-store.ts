import { create } from "zustand"
import { invoke, isTauri } from "@tauri-apps/api/core"
import {
  loadEnvGroups,
  saveEnvGroups,
  loadActiveGroupIds,
  saveActiveGroupIds,
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
  activeGroupIds: string[]
  loaded: boolean

  init: () => Promise<void>

  addGroup: () => void
  updateGroup: (groupId: string, patch: Partial<Omit<EnvGroup, "id" | "overrides">>) => void
  removeGroup: (groupId: string) => void

  addOverride: (groupId: string) => void
  updateOverride: (groupId: string, index: number, patch: Partial<EnvOverride>) => void
  removeOverride: (groupId: string, index: number) => void

  setActiveGroupIds: (ids: string[]) => void

  flattened: () => FlattenedOverride[]

  saveAndReload: () => Promise<number | null>
}

const CONFLICT_PREFIX = "[CONFLICT: "

function makeId(): string {
  return `g_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 8)}`
}

function flattenGroups(groups: EnvGroup[], activeIds: string[]): FlattenedOverride[] {
  const activeSet = new Set(activeIds)
  const byName = new Map<string, FlattenedOverride>()
  for (const group of groups) {
    if (!activeSet.has(group.id)) continue
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
let pendingSnapshot: { groups: EnvGroup[]; activeGroupIds: string[] } | null = null

/** Push flattened overrides to the Rust plugin engine immediately. */
async function pushToRust(groups: EnvGroup[], activeGroupIds: string[]): Promise<void> {
  if (!isTauri()) return
  const flattened = flattenGroups(groups, activeGroupIds)
  try {
    await invoke("set_env_overrides", { overrides: flattened })
  } catch (e) {
    console.error("Failed to sync env overrides to Rust:", e)
  }
}

function scheduleSync(snapshot: { groups: EnvGroup[]; activeGroupIds: string[] }): void {
  pendingSnapshot = snapshot
  // Push to Rust immediately so plugins see changes right away.
  void pushToRust(snapshot.groups, snapshot.activeGroupIds)

  if (persistTimer !== null) clearTimeout(persistTimer)
  persistTimer = setTimeout(() => {
    const toSync = pendingSnapshot
    pendingSnapshot = null
    persistTimer = null
    if (!toSync) return
    void (async () => {
      try {
        await saveEnvGroups(toSync.groups)
      } catch (e) {
        console.error("Failed to save env groups:", e)
      }
      try {
        await saveActiveGroupIds(toSync.activeGroupIds)
      } catch (e) {
        console.error("Failed to save active group ids:", e)
      }
    })()
  }, 250)
}

function syncNow(snapshot: { groups: EnvGroup[]; activeGroupIds: string[] }): Promise<void> {
  if (persistTimer !== null) {
    clearTimeout(persistTimer)
    persistTimer = null
    pendingSnapshot = null
  }
  return (async () => {
    await pushToRust(snapshot.groups, snapshot.activeGroupIds)
    try {
      await saveEnvGroups(snapshot.groups)
    } catch (e) {
      console.error("Failed to save env groups:", e)
    }
    try {
      await saveActiveGroupIds(snapshot.activeGroupIds)
    } catch (e) {
      console.error("Failed to save active group ids:", e)
    }
  })()
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

export const useEnvOverridesStore = create<EnvOverridesStore>((set, get) => ({
  groups: [],
  activeGroupIds: [],
  loaded: false,

  init: async () => {
    if (get().loaded) return
    try {
      const groups = await loadEnvGroups()
      const activeGroupIds = await loadActiveGroupIds(groups)
      set({ groups, activeGroupIds, loaded: true })
      // Push to Rust on first load (belt-and-suspenders with the cold-start
      // path in Rust that reads env.json directly).
      await pushToRust(groups, activeGroupIds)
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
    scheduleSync({ groups: next, activeGroupIds: get().activeGroupIds })
  },

  updateGroup: (groupId, patch) => {
    const next = get().groups.map((g) => (g.id === groupId ? { ...g, ...patch } : g))
    set({ groups: next })
    scheduleSync({ groups: next, activeGroupIds: get().activeGroupIds })
  },

  removeGroup: (groupId) => {
    const next = get().groups.filter((g) => g.id !== groupId)
    const activeNext = get().activeGroupIds.filter((id) => id !== groupId)
    set({ groups: next, activeGroupIds: activeNext })
    scheduleSync({ groups: next, activeGroupIds: activeNext })
  },

  addOverride: (groupId) => {
    const next = get().groups.map((g) =>
      g.id === groupId ? { ...g, overrides: [...g.overrides, { name: "", value: "" }] } : g
    )
    set({ groups: next })
    scheduleSync({ groups: next, activeGroupIds: get().activeGroupIds })
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
    scheduleSync({ groups: next, activeGroupIds: get().activeGroupIds })
  },

  removeOverride: (groupId, index) => {
    const next = get().groups.map((g) => {
      if (g.id !== groupId) return g
      return { ...g, overrides: g.overrides.filter((_, i) => i !== index) }
    })
    set({ groups: next })
    scheduleSync({ groups: next, activeGroupIds: get().activeGroupIds })
  },

  setActiveGroupIds: (ids) => {
    set({ activeGroupIds: ids })
    scheduleSync({ groups: get().groups, activeGroupIds: ids })
  },

  flattened: () => flattenGroups(get().groups, get().activeGroupIds),

  saveAndReload: async () => {
    const snapshot = { groups: get().groups, activeGroupIds: get().activeGroupIds }
    await syncNow(snapshot)
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
