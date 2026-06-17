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
const SCHEMA_KEY = "envSchemaVersion"
const CURRENT_ENV_SCHEMA_VERSION = 2

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

type StoreWithDelete = {
  delete?: (key: string) => Promise<void>
  set: (key: string, value: unknown) => Promise<void>
}

async function deleteStoreKey(store: StoreWithDelete, key: string): Promise<void> {
  try {
    const maybeDelete = store.delete
    if (typeof maybeDelete === "function") {
      await maybeDelete.call(store, key)
    } else {
      await store.set(key, null)
    }
  } catch {
    // Best-effort — stale keys are harmless after the schema marker is written.
  }
}

let migrationDone = false

/** Reset migration state — exposed for tests. */
export function resetEnvMigrationForTest(): void {
  migrationDone = false
}

function applyActiveIdsToGroups(groups: EnvGroup[], rawActiveIds: unknown): EnvGroup[] {
  if (!Array.isArray(rawActiveIds)) return groups
  const active = new Set(rawActiveIds.filter((id): id is string => typeof id === "string"))
  return groups.map((group) => ({ ...group, enabled: active.has(group.id) }))
}

async function writeEnvGroupsV2(groups: EnvGroup[]): Promise<void> {
  await envStore.set(GROUPS_KEY, groups)
  await envStore.set(SCHEMA_KEY, CURRENT_ENV_SCHEMA_VERSION)
  await deleteStoreKey(envStore as unknown as StoreWithDelete, ACTIVE_KEY)
  await envStore.save()
}

async function migrateToEnvFile(): Promise<void> {
  if (migrationDone) return

  // Already have data in env.json?
  const existing = await envStore.get<unknown>(GROUPS_KEY)
  if (existing !== null && existing !== undefined) {
    const schemaVersion = await envStore.get<unknown>(SCHEMA_KEY)
    const rawActive = await envStore.get<unknown>(ACTIVE_KEY)
    const groups = normalizeGroups(existing)
    const migrated =
      schemaVersion === CURRENT_ENV_SCHEMA_VERSION
        ? groups
        : applyActiveIdsToGroups(groups, rawActive)
    await writeEnvGroupsV2(migrated)
    migrationDone = true
    return
  }

  // Try groups format first (already migrated in settings.json)
  const legacyGroups = await settingsStore.get<unknown>(LEGACY_GROUPS_KEY)
  if (legacyGroups !== null && legacyGroups !== undefined) {
    let groups = normalizeGroups(legacyGroups)
    const legacyActive = await settingsStore.get<unknown>(LEGACY_ACTIVE_KEY)
    groups = applyActiveIdsToGroups(groups, legacyActive)
    await writeEnvGroupsV2(groups)
    // Clean up settings.json
    await deleteStoreKey(settingsStore as unknown as StoreWithDelete, LEGACY_GROUPS_KEY)
    await deleteStoreKey(settingsStore as unknown as StoreWithDelete, LEGACY_ACTIVE_KEY)
    await deleteStoreKey(settingsStore as unknown as StoreWithDelete, LEGACY_OVERRIDES_KEY)
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
    await writeEnvGroupsV2([group])
    await deleteStoreKey(settingsStore as unknown as StoreWithDelete, LEGACY_OVERRIDES_KEY)
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
  await writeEnvGroupsV2(groups)
}
