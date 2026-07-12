import { describe, expect, it } from "vitest"
import { parseGithubTreeUrl } from "./parse-source-url"

describe("parseGithubTreeUrl", () => {
  it("splits a github tree URL into url, label, and branch", () => {
    expect(
      parseGithubTreeUrl(
        "https://github.com/JulianKniephoff/openusage/tree/feat/warp",
      ),
    ).toEqual({
      url: "JulianKniephoff/openusage",
      label: "JulianKniephoff",
      branch: "feat/warp",
    })
  })

  it("handles a simple single-segment branch", () => {
    expect(
      parseGithubTreeUrl(
        "https://github.com/rohithgoud30/openusage/tree/add-zed-ai-support",
      ),
    ).toEqual({
      url: "rohithgoud30/openusage",
      label: "rohithgoud30",
      branch: "add-zed-ai-support",
    })
  })

  it("strips a trailing slash and .git suffix", () => {
    expect(
      parseGithubTreeUrl("https://github.com/foo/bar.git/tree/main/"),
    ).toEqual({ url: "foo/bar", label: "foo", branch: "main" })
  })

  it("returns null for non-tree github URLs", () => {
    expect(parseGithubTreeUrl("https://github.com/foo/bar")).toBeNull()
  })

  it("returns null for unrelated input", () => {
    expect(parseGithubTreeUrl("foo/bar")).toBeNull()
    expect(parseGithubTreeUrl("/path/to/repo")).toBeNull()
  })

  it("returns null when the branch has a malformed percent escape", () => {
    expect(
      parseGithubTreeUrl("https://github.com/foo/bar/tree/feature%ZZ"),
    ).toBeNull()
  })
})
