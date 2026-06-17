// Mirror of the Rust Hub types in src-tauri/src/hub/mod.rs.
// Field names are kept in camelCase to match Tauri's IPC convention — the Rust
// side serializes snake_case via #[serde(rename_all = "camelCase")] where it
// matters.

// Mirrors `DEFAULT_HUB_ID` in src-tauri/src/hub/registry.rs — the id of the
// built-in official source ("Frankie's"). Used to distinguish it from
// user-added custom sources in the UI.
export const DEFAULT_HUB_ID = "default"

export type SourceKind = "Github" | "GenericGit" | "LocalPath"

export type PackageStatus =
  | "notInstalled"
  | "installed"
  | "updateAvailable"
  | "sourceChanged"
  | "installedNewerThanSource"
  | "samePackageFromOtherSource"
  | "differentPackageSamePluginId"
  | "unmanagedInstalled"
  | "orphanedSource"

export interface Source {
  id: string
  label: string
  url: string
  kind: SourceKind
  branch: string | null
  /**
   * If non-empty, only plugins whose id is in this list are shown / installable
   * for this source. `null` or empty means "all plugins in the source".
   */
  pluginFilter: string[] | null
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
  updatedAt: number | null
  packageHash: string
  packageStatus: PackageStatus
  updateAvailable: boolean
}

export interface SkippedPlugin {
  path: string
  reason: string
}

export interface SourceSnapshot {
  branch: string | null
  commitSha: string | null
  checkedAt: number
  discoveredCount: number
  skippedCount: number
}

export interface HubBrowseView {
  source: Source
  available: PluginInfo[]
  skipped: SkippedPlugin[]
  snapshot: SourceSnapshot
}

export interface UpdateInfo {
  sourceId: string
  pluginId: string
  from: string
  to: string
  packageHash: string
}

export type HubErrorCode =
  | "InvalidUrl"
  | "GitNotInstalled"
  | "CloneFailed"
  | "NotFound"
  | "Conflict"
  | "IoError"
  | "ManifestParse"
  | "SourceHealthFailed"

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
