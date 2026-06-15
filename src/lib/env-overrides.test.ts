import { beforeEach, describe, expect, it, vi } from "vitest"

const storeState = new Map<string, unknown>()

vi.mock("@tauri-apps/plugin-store", () => ({
  LazyStore: class {
    async get<T>(key: string): Promise<T | null> {
      if (!storeState.has(key)) return null
      return storeState.get(key) as T | null
    }
    async set<T>(key: string, value: T): Promise<void> {
      storeState.set(key, value)
    }
    async save(): Promise<void> {}
  },
}))

import {
  loadEnvOverrides,
  saveEnvOverrides,
  normalizeEnvOverrides,
  type EnvOverride,
} from "@/lib/env-overrides"

describe("env-overrides", () => {
  beforeEach(() => {
    storeState.clear()
  })

  it("returns [] when nothing stored", async () => {
    await expect(loadEnvOverrides()).resolves.toEqual([])
  })

  it("round-trips saved overrides", async () => {
    const overrides: EnvOverride[] = [
      { name: "A", kind: "literal", value: "api" },
      { name: "C", kind: "reference", value: "D" },
    ]
    await saveEnvOverrides(overrides)
    await expect(loadEnvOverrides()).resolves.toEqual(overrides)
  })

  it("drops entries with invalid names", () => {
    const result = normalizeEnvOverrides([
      { name: "9bad", kind: "literal", value: "x" },
      { name: "has space", kind: "literal", value: "x" },
      { name: "GOOD_1", kind: "literal", value: "x" },
    ])
    expect(result).toEqual([{ name: "GOOD_1", kind: "literal", value: "x" }])
  })

  it("dedupes by name keeping the last definition, preserving order", () => {
    const result = normalizeEnvOverrides([
      { name: "A", kind: "literal", value: "first" },
      { name: "B", kind: "reference", value: "X" },
      { name: "A", kind: "literal", value: "second" },
    ])
    expect(result).toEqual([
      { name: "B", kind: "reference", value: "X" },
      { name: "A", kind: "literal", value: "second" },
    ])
  })

  it("drops entries with an unknown kind or empty value", () => {
    const result = normalizeEnvOverrides([
      { name: "A", kind: "literal", value: "" },
      { name: "B", kind: "bogus" as unknown as "literal", value: "x" },
      { name: "C", kind: "reference", value: "T" },
    ])
    expect(result).toEqual([{ name: "C", kind: "reference", value: "T" }])
  })

  it("ignores non-array stored values", async () => {
    storeState.set("envOverrides", { not: "an array" })
    await expect(loadEnvOverrides()).resolves.toEqual([])
  })
})
