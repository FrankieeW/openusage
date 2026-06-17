import type { HubError, HubErrorCode, Source } from "./types"
import { DEFAULT_HUB_ID } from "./types"

// Centralized Hub error → human-readable English strings.
// Kept in a plain object so future i18n can wrap this layer without touching
// call sites.

export function labelForError(err: HubError): string {
  switch (err.code) {
    case "InvalidUrl":
      return "That doesn't look like a valid source URL."
    case "GitNotInstalled":
      return "Git is required for non-LocalPath sources but wasn't found on PATH."
    case "CloneFailed":
      return err.message || "Clone failed."
    case "NotFound":
      return err.message || "Not found."
    case "Conflict":
      if (err.context?.otherSourceId === "unmanaged") {
        return "Already installed outside Hub. Uninstall the existing copy first."
      }
      if (typeof err.context?.otherSourceId === "string") {
        return "Already installed from another source. Uninstall first."
      }
      return err.message || "Install conflict."
    case "IoError":
      return `Disk error: ${err.message || "unknown"}`
    case "ManifestParse":
      return `Plugin manifest error: ${err.message || "unknown"}`
    case "SourceHealthFailed":
      return err.message || "Source has no valid plugins."
    default:
      return err.message || "Unknown error."
  }
}

export function labelForSourceKind(kind: string): string {
  switch (kind) {
    case "Github":
      return "GitHub"
    case "GenericGit":
      return "Git"
    case "LocalPath":
      return "Local Source"
    default:
      return kind
  }
}

export function labelForSourceTrust(source: Source | null | undefined): string {
  if (!source) return "Local Development"
  if (source.id === DEFAULT_HUB_ID) return "Curated / Default"
  if (source.kind === "LocalPath") return "Local Development"
  if (source.kind === "GenericGit") return "Unknown Git Source"
  return "Community"
}

export function descriptionForSourceTrust(source: Source | null | undefined): string {
  const label = labelForSourceTrust(source)
  switch (label) {
    case "Curated / Default":
      return "Built-In Recommended Source"
    case "Community":
      return "User Added GitHub Source"
    case "Local Development":
      return "Local Files On This Machine"
    case "Unknown Git Source":
      return "Non-GitHub Git Source"
    default:
      return label
  }
}

export function shortPackageHash(hash: string): string {
  if (!hash) return "Unknown"
  const prefix = "sha256:"
  if (!hash.startsWith(prefix)) return hash
  const digest = hash.slice(prefix.length)
  if (digest.length <= 12) return hash
  return `${prefix}${digest.slice(0, 12)}`
}

export function labelForPluginUpdatedAt(updatedAt: number | null): string | null {
  if (updatedAt === null || !Number.isFinite(updatedAt) || updatedAt <= 0) {
    return null
  }
  return `Updated ${new Date(updatedAt).toISOString().slice(0, 10)}`
}

export const KNOWN_ERROR_CODES: HubErrorCode[] = [
  "InvalidUrl",
  "GitNotInstalled",
  "CloneFailed",
  "NotFound",
  "Conflict",
  "IoError",
  "ManifestParse",
  "SourceHealthFailed",
]
