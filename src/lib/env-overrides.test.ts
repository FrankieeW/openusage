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
    async delete(key: string): Promise<void> {
      storeState.delete(key)
    }
    async save(): Promise<void> {}
  },
}))

import {
  parseValueInput,
  loadEnvGroups,
  saveEnvGroups,
  normalizeGroups,
  resetEnvMigrationForTest,
  type EnvGroup,
} from "@/lib/env-overrides"

describe("parseValueInput", () => {
  it("treats plain text as a literal", () => {
    expect(parseValueInput("api")).toEqual({ kind: "literal", value: "api" })
  })
  it("treats $B as a reference", () => {
    expect(parseValueInput("$B")).toEqual({ kind: "reference", value: "B" })
  })
  it("treats $$B as a literal starting with $", () => {
    expect(parseValueInput("$$B")).toEqual({ kind: "literal", value: "$B" })
  })
  it("treats $$$B as a literal $$B", () => {
    expect(parseValueInput("$$$B")).toEqual({ kind: "literal", value: "$$B" })
  })
  it("treats a single $ as a literal (incomplete reference)", () => {
    expect(parseValueInput("$")).toEqual({ kind: "literal", value: "$" })
  })
  it("treats empty as literal empty (callers drop empty rows)", () => {
    expect(parseValueInput("")).toEqual({ kind: "literal", value: "" })
  })
})

describe("normalizeGroups", () => {
  it("returns [] for non-array input", () => {
    expect(normalizeGroups(null)).toEqual([])
  })
  it("keeps groups with valid names, drops invalid", () => {
    const result = normalizeGroups([
      { id: "g1", name: "Dev", enabled: true, overrides: [] },
      { id: "g2", name: "", enabled: true, overrides: [] },
      { id: "g3", name: "Prod", enabled: false, overrides: [] },
    ])
    expect(result).toEqual([
      { id: "g1", name: "Dev", enabled: true, overrides: [] },
      { id: "g3", name: "Prod", enabled: false, overrides: [] },
    ])
  })
  it("generates an id when missing", () => {
    const result = normalizeGroups([
      { name: "Dev", enabled: true, overrides: [] },
    ])
    expect(result).toHaveLength(1)
    expect(result[0].id).toEqual(expect.any(String))
    expect(result[0].id.length).toBeGreaterThan(0)
  })
  it("normalizes each override: name, non-empty value, dedupes by name", () => {
    const result = normalizeGroups([
      {
        id: "g1",
        name: "Dev",
        enabled: true,
        overrides: [
          { name: "9bad", value: "x" },
          { name: "B", value: "" },
          { name: "A", value: "first" },
          { name: "A", value: "second" },
        ],
      },
    ])
    expect(result[0].overrides).toEqual([
      { name: "A", value: "second" },
    ])
  })
})

describe("persistence", () => {
  beforeEach(() => {
    storeState.clear()
    resetEnvMigrationForTest()
  })

  it("loadEnvGroups returns [] by default", async () => {
    await expect(loadEnvGroups()).resolves.toEqual([])
  })
  it("saveEnvGroups round-trips", async () => {
    const groups: EnvGroup[] = [{ id: "g1", name: "Dev", enabled: true, overrides: [{ name: "A", value: "api" }] }]
    await saveEnvGroups(groups)
    await expect(loadEnvGroups()).resolves.toEqual(groups)
    expect(storeState.get("envSchemaVersion")).toBe(2)
  })

  it("migrates env.json activeGroupIds into group enabled flags", async () => {
    storeState.set("groups", [
      { id: "g1", name: "Dev", enabled: true, overrides: [] },
      { id: "g2", name: "Prod", enabled: false, overrides: [] },
    ])
    storeState.set("activeGroupIds", ["g2"])

    await expect(loadEnvGroups()).resolves.toEqual([
      { id: "g1", name: "Dev", enabled: false, overrides: [] },
      { id: "g2", name: "Prod", enabled: true, overrides: [] },
    ])
    expect(storeState.get("envSchemaVersion")).toBe(2)
    expect(storeState.has("activeGroupIds")).toBe(false)
  })

  it("keeps env.json v2 group enabled flags without activeGroupIds", async () => {
    storeState.set("envSchemaVersion", 2)
    storeState.set("groups", [
      { id: "g1", name: "Dev", enabled: false, overrides: [] },
      { id: "g2", name: "Prod", enabled: true, overrides: [] },
    ])
    storeState.set("activeGroupIds", ["g1"])

    await expect(loadEnvGroups()).resolves.toEqual([
      { id: "g1", name: "Dev", enabled: false, overrides: [] },
      { id: "g2", name: "Prod", enabled: true, overrides: [] },
    ])
    expect(storeState.has("activeGroupIds")).toBe(false)
  })

  it("loadEnvGroups migrates legacy envOverrides into a Default group", async () => {
    // Legacy data lives in settings.json, keyed as "envOverrides".
    // The mock shares a single Map across all LazyStore instances, so
    // settingsStore and envStore read/write the same map.
    storeState.set("envOverrides", [{ name: "A", kind: "literal", value: "api" }])
    const groups = await loadEnvGroups()
    expect(groups).toEqual([
      { id: expect.any(String), name: "Default", enabled: true, overrides: [{ name: "A", value: "api" }] },
    ])
    // After migration, the legacy key should be deleted from settings.json.
    expect(storeState.has("envOverrides")).toBe(false)
    // New data should be in env.json format.
    expect(storeState.has("groups")).toBe(true)
    expect(storeState.get("envSchemaVersion")).toBe(2)
    expect(storeState.has("activeGroupIds")).toBe(false)
  })
})
