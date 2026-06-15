// Mirror of the Rust Hub types in src-tauri/src/hub/mod.rs.
// Field names are kept in camelCase to match Tauri's IPC convention — the Rust
// side serializes snake_case via #[serde(rename_all = "camelCase")] where it
// matters; enum variants are PascalCase strings.

export type SourceKind = "Github" | "GenericGit" | "LocalPath"

export interface Source {
  id: string
  label: string
  url: string
  kind: SourceKind
  branch: string | null
  addedAt: number
  lastRefreshedAt: number | null
  autoCheck: boolean
}

export interface PluginInfo {
  id: string
  name: string
  brandColor: string | null
  iconDataUrl: string | null
  sourceId: string
  installed: boolean
  installedSourceId: string | null
  unmanaged: boolean
  installedVersion: string | null
  availableVersion: string
  updateAvailable: boolean
}

export interface SkippedPlugin {
  path: string
  reason: string
}

export interface HubBrowseView {
  source: Source
  available: PluginInfo[]
  skipped: SkippedPlugin[]
}

export interface UpdateInfo {
  sourceId: string
  pluginId: string
  from: string
  to: string
}

export type HubErrorCode =
  | "InvalidUrl"
  | "GitNotInstalled"
  | "CloneFailed"
  | "NotFound"
  | "Conflict"
  | "IoError"
  | "ManifestParse"

export interface HubError {
  code: HubErrorCode
  message: string
  context?: Record<string, unknown>
}

export function isHubError(value: unknown): value is HubError {
  if (typeof value !== "object" || value === null) return false
  const v = value as Record<string, unknown>
  return typeof v.code === "string" && typeof v.message === "string"
}