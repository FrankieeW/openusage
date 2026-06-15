import type { HubError, HubErrorCode } from "./types"

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

export const KNOWN_ERROR_CODES: HubErrorCode[] = [
  "InvalidUrl",
  "GitNotInstalled",
  "CloneFailed",
  "NotFound",
  "Conflict",
  "IoError",
  "ManifestParse",
]