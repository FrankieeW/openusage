import { create } from "zustand"
import { invoke, isTauri } from "@tauri-apps/api/core"
import {
  loadEnvOverrides,
  saveEnvOverrides,
  normalizeEnvOverrides,
  type EnvOverride,
} from "@/lib/env-overrides"

type EnvOverridesStore = {
  overrides: EnvOverride[]
  loaded: boolean
  init: () => Promise<void>
  addOverride: () => void
  updateOverride: (index: number, patch: Partial<EnvOverride>) => void
  removeOverride: (index: number) => void
}

async function persistAndSync(overrides: EnvOverride[]): Promise<void> {
  const normalized = normalizeEnvOverrides(overrides)
  try {
    await saveEnvOverrides(normalized)
  } catch (error) {
    console.error("Failed to save env overrides:", error)
  }
  if (!isTauri()) return
  try {
    await invoke("set_env_overrides", { overrides: normalized })
  } catch (error) {
    console.error("Failed to sync env overrides:", error)
  }
}

export const useEnvOverridesStore = create<EnvOverridesStore>((set, get) => ({
  overrides: [],
  loaded: false,

  init: async () => {
    if (get().loaded) return
    try {
      const overrides = await loadEnvOverrides()
      set({ overrides, loaded: true })
    } catch (error) {
      console.error("Failed to load env overrides:", error)
      set({ loaded: true })
    }
  },

  addOverride: () => {
    const next: EnvOverride[] = [
      ...get().overrides,
      { name: "", kind: "literal", value: "" },
    ]
    set({ overrides: next })
    void persistAndSync(next)
  },

  updateOverride: (index, patch) => {
    const next = get().overrides.map((entry, i) =>
      i === index ? { ...entry, ...patch } : entry
    )
    set({ overrides: next })
    void persistAndSync(next)
  },

  removeOverride: (index) => {
    const next = get().overrides.filter((_, i) => i !== index)
    set({ overrides: next })
    void persistAndSync(next)
  },
}))
