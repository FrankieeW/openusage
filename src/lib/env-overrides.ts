import { LazyStore } from "@tauri-apps/plugin-store"
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

// ---------------------------------------------------------------------------
// env.json — dedicated store so env data never interferes with settings.json
// keys or the legacy migration path.
// ---------------------------------------------------------------------------
const ENV_STORE_PATH = "env.json"
export const envStore = new LazyStore(ENV_STORE_PATH)

const GROUPS_KEY = "groups"
const ACTIVE_KEY = "activeGroupIds"

// Legacy keys in settings.json (migrated once → then deleted).
const LEGACY_GROUPS_KEY = "envGroups"
const LEGACY_ACTIVE_KEY = "envActiveGroupIds"
const LEGACY_OVERRIDES_KEY = "envOverrides"

const ENV_NAME_PATTERN = /^[A-Za-z_][A-Za-z0-9_]*$/

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeId(): string {
  return `g_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 8)}`
}

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

// ---------------------------------------------------------------------------
// One-time migration from settings.json → env.json
// ---------------------------------------------------------------------------

type StoreWithDelete = { delete?: (key: string) => Promise<void> }

async function deleteSettingsKey(key: string): Promise<void> {
  try {
    const maybeDelete = (settingsStore as unknown as StoreWithDelete).delete
    if (typeof maybeDelete === "function") {
      await maybeDelete.call(settingsStore, key)
    } else {
      await settingsStore.set(key, null)
    }
  } catch {
    // Best-effort — the key will linger in settings.json harmlessly.
  }
}

let migrationDone = false

/** Reset migration state — exposed for tests. */
export function resetEnvMigrationForTest(): void {
  migrationDone = false
}

async function migrateToEnvFile(): Promise<void> {
  if (migrationDone) return

  // Already have data in env.json?
  const existing = await envStore.get<unknown>(GROUPS_KEY)
  if (existing !== null && existing !== undefined) {
    migrationDone = true
    return
  }

  // Try groups format first (already migrated in settings.json)
  const legacyGroups = await settingsStore.get<unknown>(LEGACY_GROUPS_KEY)
  if (legacyGroups !== null && legacyGroups !== undefined) {
    await envStore.set(GROUPS_KEY, legacyGroups)
    const legacyActive = await settingsStore.get<unknown>(LEGACY_ACTIVE_KEY)
    if (Array.isArray(legacyActive)) {
      await envStore.set(ACTIVE_KEY, legacyActive)
    }
    await envStore.save()
    // Clean up settings.json
    await deleteSettingsKey(LEGACY_GROUPS_KEY)
    await deleteSettingsKey(LEGACY_ACTIVE_KEY)
    await deleteSettingsKey(LEGACY_OVERRIDES_KEY)
    await settingsStore.save()
    migrationDone = true
    return
  }

  // Try true-legacy flat overrides (pre-groups era)
  const legacyOverrides = await settingsStore.get<unknown>(LEGACY_OVERRIDES_KEY)
  if (Array.isArray(legacyOverrides) && legacyOverrides.length > 0) {
    const id = makeId()
    const byName = new Map<string, EnvOverride>()
    for (const o of legacyOverrides) {
      const n = normalizeOverride(o)
      if (!n) continue
      byName.set(n.name, n)
    }
    const group: EnvGroup = {
      id,
      name: "Default",
      enabled: true,
      overrides: Array.from(byName.values()),
    }
    await envStore.set(GROUPS_KEY, [group])
    await envStore.set(ACTIVE_KEY, [id])
    await envStore.save()
    await deleteSettingsKey(LEGACY_OVERRIDES_KEY)
    await settingsStore.save()
  }

  migrationDone = true
}

// ---------------------------------------------------------------------------
// Public API — same signatures, now backed by env.json
// ---------------------------------------------------------------------------

export async function loadEnvGroups(): Promise<EnvGroup[]> {
  await migrateToEnvFile()
  const stored = await envStore.get<unknown>(GROUPS_KEY)
  return normalizeGroups(stored)
}

export async function saveEnvGroups(groups: EnvGroup[]): Promise<void> {
  await envStore.set(GROUPS_KEY, groups)
  await envStore.save()
}

export async function loadActiveGroupIds(
  existingGroups?: EnvGroup[],
): Promise<string[]> {
  const ids = await envStore.get<unknown>(ACTIVE_KEY)
  if (!Array.isArray(ids)) return []
  const groups = existingGroups ?? (await loadEnvGroups())
  if (groups.length === 0) {
    return ids.filter((x): x is string => typeof x === "string")
  }
  const known = new Set(groups.map((g) => g.id))
  return ids.filter((x): x is string => typeof x === "string" && known.has(x))
}

export async function saveActiveGroupIds(ids: string[]): Promise<void> {
  await envStore.set(ACTIVE_KEY, ids)
  await envStore.save()
}
