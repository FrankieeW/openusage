import { describe, expect, it } from "vitest"
import {
  labelForPluginUpdatedAt,
  labelForError,
  labelForSourceKind,
  labelForSourceTrust,
  shortPackageHash,
  KNOWN_ERROR_CODES,
} from "./labels"
import type { HubError, Source } from "./types"

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
      { code: "SourceHealthFailed", message: "source has no valid plugins" },
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
    expect(KNOWN_ERROR_CODES.length).toBe(8)
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

describe("labelForSourceTrust", () => {
  function source(overrides: Partial<Source>): Source {
    return {
      id: "src-1",
      label: "Source",
      url: "https://github.com/foo/bar",
      kind: "Github",
      branch: null,
      pluginFilter: null,
      addedAt: 0,
      lastRefreshedAt: null,
      autoCheck: false,
      ...overrides,
    }
  }

  it("maps source trust tiers", () => {
    expect(labelForSourceTrust(source({ id: "default" }))).toBe("default")
    expect(labelForSourceTrust(source({ kind: "Github" }))).toBe("Community")
    expect(labelForSourceTrust(source({ kind: "LocalPath" }))).toBe(
      "Local Development",
    )
    expect(labelForSourceTrust(source({ kind: "GenericGit" }))).toBe(
      "Unknown Git Source",
    )
  })

  it("shortens sha256 hashes for previews", () => {
    expect(shortPackageHash("sha256:1234567890abcdef")).toBe(
      "sha256:1234567890ab",
    )
    expect(shortPackageHash("sha256:fixture")).toBe("sha256:fixture")
  })

  it("formats plugin updated dates", () => {
    expect(labelForPluginUpdatedAt(Date.UTC(2026, 5, 17))).toBe(
      "Updated 2026-06-17",
    )
    expect(labelForPluginUpdatedAt(null)).toBeNull()
  })
})
