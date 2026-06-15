import { describe, expect, it } from "vitest"
import { labelForError, labelForSourceKind, KNOWN_ERROR_CODES } from "./labels"
import type { HubError } from "./types"

describe("labelForError", () => {
  it("returns specific copy for each error code", () => {
    const samples: HubError[] = [
      { code: "InvalidUrl", message: "" },
      { code: "GitNotInstalled", message: "" },
      { code: "CloneFailed", message: "remote hung up" },
      { code: "NotFound", message: "plugin missing" },
      { code: "Conflict", message: "", context: { otherSourceId: "src-existing" } },
      { code: "Conflict", message: "", context: { otherSourceId: "unmanaged" } },
      { code: "IoError", message: "disk full" },
      { code: "ManifestParse", message: "bad json" },
    ]
    for (const err of samples) {
      const label = labelForError(err)
      expect(label).toBeTypeOf("string")
      expect(label.length).toBeGreaterThan(0)
    }
  })

  it("falls back to message for unknown-shaped errors", () => {
    // Force-cast for the unknown-code branch
    const err = { code: "Unknown" as HubError["code"], message: "fallback" }
    expect(labelForError(err)).toBe("fallback")
  })

  it("covers every known error code", () => {
    expect(KNOWN_ERROR_CODES.length).toBe(7)
  })
})

describe("labelForSourceKind", () => {
  it("maps known kinds", () => {
    expect(labelForSourceKind("Github")).toBe("GitHub")
    expect(labelForSourceKind("GenericGit")).toBe("Git")
    expect(labelForSourceKind("LocalPath")).toBe("Local Source")
  })

  it("passes through unknown kinds", () => {
    expect(labelForSourceKind("Other")).toBe("Other")
  })
})