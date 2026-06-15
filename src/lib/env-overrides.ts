import { settingsStore } from "@/lib/settings"

export type EnvOverrideKind = "literal" | "reference"

export type EnvOverride = {
  name: string
  kind: EnvOverrideKind
  value: string
}

const ENV_OVERRIDES_KEY = "envOverrides"

const ENV_NAME_PATTERN = /^[A-Za-z_][A-Za-z0-9_]*$/

const OVERRIDE_KINDS: EnvOverrideKind[] = ["literal", "reference"]

const store = settingsStore

function isKind(value: unknown): value is EnvOverrideKind {
  return OVERRIDE_KINDS.includes(value as EnvOverrideKind)
}

// Validate + sanitize raw overrides: valid env-var name, known kind, non-empty
// value. Later definitions of the same name win; original order is preserved by
// keeping each name at the position of its LAST occurrence.
export function normalizeEnvOverrides(raw: unknown): EnvOverride[] {
  if (!Array.isArray(raw)) return []

  const byName = new Map<string, EnvOverride>()
  for (const entry of raw) {
    if (typeof entry !== "object" || entry === null) continue
    const { name, kind, value } = entry as Record<string, unknown>
    if (typeof name !== "string" || !ENV_NAME_PATTERN.test(name)) continue
    if (!isKind(kind)) continue
    if (typeof value !== "string" || value.length === 0) continue
    // Delete-then-set moves the name to the end (last occurrence wins position).
    byName.delete(name)
    byName.set(name, { name, kind, value })
  }
  return Array.from(byName.values())
}

export async function loadEnvOverrides(): Promise<EnvOverride[]> {
  const stored = await store.get<unknown>(ENV_OVERRIDES_KEY)
  return normalizeEnvOverrides(stored)
}

export async function saveEnvOverrides(overrides: EnvOverride[]): Promise<void> {
  await store.set(ENV_OVERRIDES_KEY, overrides)
  await store.save()
}
