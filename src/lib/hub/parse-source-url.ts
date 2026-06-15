// Parse a GitHub "tree" URL into the fields the Add Source dialog uses.
//
// A URL like:
//   https://github.com/JulianKniephoff/openusage/tree/feat/warp
// is split into:
//   { url: "JulianKniephoff/openusage", label: "JulianKniephoff", branch: "feat/warp" }
//
// The branch segment may itself contain slashes (e.g. "feat/warp"), so
// everything after "/tree/" is treated as the branch. Returns null when the
// input is not a recognizable GitHub tree URL, leaving the caller's fields
// untouched.

export interface ParsedSourceUrl {
  url: string
  label: string
  branch: string
}

const GITHUB_TREE_RE =
  /^https?:\/\/github\.com\/([^/\s]+)\/([^/\s]+)\/tree\/(.+?)\/?$/i

export function parseGithubTreeUrl(input: string): ParsedSourceUrl | null {
  const match = GITHUB_TREE_RE.exec(input.trim())
  if (!match) return null

  const owner = match[1]
  const repo = match[2].replace(/\.git$/i, "")
  const branch = decodeURIComponent(match[3])

  if (!owner || !repo || !branch) return null

  return {
    url: `${owner}/${repo}`,
    label: owner,
    branch,
  }
}
