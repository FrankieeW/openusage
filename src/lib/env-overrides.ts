import { settingsStore } from "@/lib/settings"

export type EnvOverrideKind = "literal" | "reference"

export type EnvOverride = { name: string; value: string }

export type EnvGroup = {
  id: string
  name: string
  enabled: boolean
  overrides: EnvOverride[]
}

export type ParsedValue = { kind: EnvOverrideKind; value: string }

const ENV_GROUPS_KEY = "envGroups"
const ENV_ACTIVE_KEY = "envActiveGroupIds"
const LEGACY_ENV_OVERRIDES_KEY = "envOverrides"

const ENV_NAME_PATTERN = /^[A-Za-z_][A-Za-z0-9_]*$/

type LegacyStoreWithDelete = {
  delete?: (key: string) => Promise<void>
}

async function deleteStoreKey(key: string): Promise<void> {
  const maybeDelete = (settingsStore as unknown as LegacyStoreWithDelete).delete
  if (typeof maybeDelete === "function") {
    await maybeDelete.call(settingsStore, key)
    return
  }
  // Fallback for store implementations without delete support.
  await settingsStore.set(key, null)
}

function makeId(): string {
  return `g_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 8)}`
}

// Parse a value-input field. `$$` literal-escape: a value starting with `$$`
// becomes a literal whose text starts with `$` (one `$` is consumed).
// A value starting with a single `$` is a reference to another env var.
export function parseValueInput(input: string): ParsedValue {
  if (input.startsWith("$$")) {
    return { kind: "literal", value: input.slice(1) }
  }
  if (input.startsWith("$") && input.length > 1) {
    return { kind: "reference", value: input.slice(1) }
  }
  return { kind: "literal", value: input }
}

function normalizeOverride(raw: unknown): EnvOverride | null {
  if (typeof raw !== "object" || raw === null) return null
  const { name, value } = raw as Record<string, unknown>
  if (typeof name !== "string" || !ENV_NAME_PATTERN.test(name)) return null
  if (typeof value !== "string" || value.length === 0) return null
  return { name, value }
}

export function normalizeGroups(raw: unknown): EnvGroup[] {
  if (!Array.isArray(raw)) return []
  const groups: EnvGroup[] = []
  for (const entry of raw) {
    if (typeof entry !== "object" || entry === null) continue
    const { id, name, enabled, overrides } = entry as Record<string, unknown>
    if (typeof name !== "string" || name.length === 0) continue
    if (typeof enabled !== "boolean") continue
    const list = Array.isArray(overrides) ? overrides : []
    const byName = new Map<string, EnvOverride>()
    for (const o of list) {
      const n = normalizeOverride(o)
      if (!n) continue
      byName.delete(n.name)
      byName.set(n.name, n)
    }
    groups.push({
      id: typeof id === "string" && id.length > 0 ? id : makeId(),
      name,
      enabled,
      overrides: Array.from(byName.values()),
    })
  }
  return groups
}

export async function loadEnvGroups(): Promise<EnvGroup[]> {
  const legacy = await settingsStore.get<unknown>(LEGACY_ENV_OVERRIDES_KEY)
  if (legacy !== null && legacy !== undefined) {
    const id = makeId()
    const group: EnvGroup = { id, name: "Default", enabled: true, overrides: [] }
    if (Array.isArray(legacy)) {
      const byName = new Map<string, EnvOverride>()
      for (const o of legacy) {
        const n = normalizeOverride(o)
        if (!n) continue
        byName.delete(n.name)
        byName.set(n.name, n)
      }
      group.overrides = Array.from(byName.values())
    }
    const groups = [group]
    await settingsStore.set(ENV_GROUPS_KEY, groups)
    await settingsStore.set(ENV_ACTIVE_KEY, [id])
    await deleteStoreKey(LEGACY_ENV_OVERRIDES_KEY)
    await settingsStore.save()
    return groups
  }
  const stored = await settingsStore.get<unknown>(ENV_GROUPS_KEY)
  return normalizeGroups(stored)
}

export async function saveEnvGroups(groups: EnvGroup[]): Promise<void> {
  await settingsStore.set(ENV_GROUPS_KEY, groups)
  await settingsStore.save()
}

export async function loadActiveGroupIds(): Promise<string[]> {
  const ids = await settingsStore.get<unknown>(ENV_ACTIVE_KEY)
  if (!Array.isArray(ids)) return []
  const groups = await loadEnvGroups()
  // When no groups exist yet there's nothing to filter against; return as-is so
  // a freshly-saved id list round-trips. Once groups are present, drop any
  // stale ids that no longer correspond to a known group.
  if (groups.length === 0) {
    return ids.filter((x): x is string => typeof x === "string")
  }
  const known = new Set(groups.map((g) => g.id))
  return ids.filter((x): x is string => typeof x === "string" && known.has(x))
}

export async function saveActiveGroupIds(ids: string[]): Promise<void> {
  await settingsStore.set(ENV_ACTIVE_KEY, ids)
  await settingsStore.save()
}
