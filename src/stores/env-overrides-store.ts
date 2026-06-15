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

  /** Flatten enabled groups into the list the backend receives. If the same
   *  name appears in two or more enabled groups, the merged entry is a single
   *  literal override with value "[CONFLICT: NAME]". */
  flattened: () => FlattenedOverride[]
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

let persistTimer: ReturnType<typeof setTimeout> | null = null
let pendingSnapshot: { groups: EnvGroup[]; activeGroupIds: string[] } | null = null

function scheduleSync(snapshot: { groups: EnvGroup[]; activeGroupIds: string[] }): void {
  pendingSnapshot = snapshot
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
      if (!isTauri()) return
      try {
        await invoke("set_env_overrides", {
          overrides: flattenGroups(toSync.groups, toSync.activeGroupIds),
        })
      } catch (e) {
        console.error("Failed to sync env overrides:", e)
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
    if (!isTauri()) return
    try {
      await invoke("set_env_overrides", {
        overrides: flattenGroups(snapshot.groups, snapshot.activeGroupIds),
      })
    } catch (e) {
      console.error("Failed to sync env overrides:", e)
    }
  })()
}

export const useEnvOverridesStore = create<EnvOverridesStore>((set, get) => ({
  groups: [],
  activeGroupIds: [],
  loaded: false,

  init: async () => {
    if (get().loaded) return
    try {
      const [groups, activeGroupIds] = await Promise.all([
        loadEnvGroups(),
        loadActiveGroupIds(),
      ])
      set({ groups, activeGroupIds, loaded: true })
      await syncNow({ groups, activeGroupIds })
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
}))