# Contributing to OpenUsage.cc

OpenUsage.cc — Community Collection — is a community fork of
[OpenUsage](https://github.com/robinebers/openusage). The fork keeps everything
upstream offers (17 providers, menu bar tracking, global shortcut, local HTTP
API, proxy support) and adds the **Plugin Hub**: subscribe to any community
plugin source without rebuilding the app.

This is a fork, not a replacement. PRs that make sense upstream can still go
there; PRs that are fork-specific (Hub, multi-source, env-overrides, anything
in this repo's delta) belong here. Read this whole document before opening a PR.

## Philosophy

- **More sources, no rebuilds.** The fork's whole reason to exist is letting
  users add new providers and sources without waiting on a release.
- **No feature creep beyond Hub + multi-source + env.** If your change is
  about tracking AI subscription usage, fit it in. If it's about something
  else, propose it upstream first.
- **Match the existing design language.** OpenUsage.cc has a specific look
  and feel. Borrow from existing components; don't introduce new visual
  styles.
- **Keep it simple.** Don't over-engineer. Don't add abstractions for
  hypothetical future requirements.

If you're unsure whether your idea fits, open an issue first.

## Ground Rules

- No AI-generated commit messages. Write your own.
- Test your changes. If it touches UI, include before/after screenshots.
- One PR per concern. Don't bundle unrelated changes.
- All new user-facing copy is Title Case (per `AGENTS.md`).
- Don't introduce a new icon library — use `lucide-react` (the codebase
  standard, despite `AGENTS.md` mentioning hugeicons; do not refactor icons
  in unrelated PRs).
- Don't add console.log statements. Fail loudly into error logging.
- Mutations are forbidden. Always create new objects.

## License Agreement

By submitting a pull request, you agree that your contribution is licensed under
the [MIT License](LICENSE) that covers this project.

## How to Contribute

### Fork and PR workflow

1. Fork the repo
2. Create a branch (`feat/my-change`, `fix/some-bug`, etc.)
3. Make your changes
4. Run `bun run build` and `bun run test` to verify nothing is broken
5. Run `cd src-tauri && cargo build` to confirm the Rust side compiles
6. Open a PR against `main` here (`FrankieeW/openusage`)

### Add a provider plugin via the Hub

The fork ships the upstream plugins and adds a Hub for community plugins.
**Plugins no longer need to live in this repo to be installable.**

The recommended path:

1. **Publish to a community collection.** Create or fork a collection repo
   (e.g. fork [`FrankieeW/openusage-collection`](https://github.com/FrankieeW/openusage-collection))
   and add your plugin under `plugins/<provider-id>/` with `plugin.json` and
   `plugin.js`. See the [Plugin API docs](docs/plugins/api.md) for the full
   spec.
2. **Add documentation** in `docs/providers/<provider-id>.md` inside the
   collection repo.
3. **Subscribe to the collection** in OpenUsage.cc via **Hub → Add Source** —
   paste the collection repo's URL. Plugins install one-click from there.
4. **Open a PR against the collection repo** with screenshots showing the
   provider working in `bun tauri dev`.

If you don't want to maintain a collection, you can still drop a plugin
directly into this repo's `plugins/` folder — but the Hub path is preferred
because it doesn't require a fork release to ship.

For a more detailed write-up, see [Recommended Sources](docs/recommended-sources.md).

### Fix a bug

1. Reference the issue number in your PR
2. Describe the root cause and fix
3. Include before/after screenshots for UI bugs
4. Add a regression test when it fits

### Request a feature

Don't open a PR for large features without discussing first.
[Open an issue](https://github.com/FrankieeW/openusage/issues/new) and make
your case.

## What Gets Accepted

- Bug fixes with clear descriptions and a regression test
- Hub-related improvements: better source discovery, faster sync, conflict
  resolution, etc.
- Multi-source support: same provider from different sources each gets its
  own entry
- New providers shipped via a community collection (see above)
- Documentation improvements (including `docs/recommended-sources.md` and
  user-facing guides under `docs/`)
- Performance improvements with benchmarks
- Accessibility improvements

## What Gets Rejected

- Changes that compromise the fork's "subscribe without rebuilding" promise
- PRs without testing evidence (no `bun run test` run, no screenshot for UI)
- Code with no clear purpose or explanation
- Cosmetic-only changes without prior discussion
- Anything that should go upstream first (open there, not here)

## Code Standards

- TypeScript for frontend (`src/`)
- Rust for backend (`src-tauri/`)
- Follow existing patterns in the codebase — see `AGENTS.md` for the
  project's non-negotiables (immutability, small focused files, Title Case
  copy, no silent fallbacks)
- No new dependencies without justification
- Tests are required for any non-trivial change. Aim for the test levels
  listed in `AGENTS.md` (unit + integration; e2e when it fits)

## Maintainers

- [@FrankieeW](https://github.com/FrankieeW) (lead, fork author)
- Maintainers from the upstream project are not auto-active here; if you're
  a maintainer upstream and want to help, ping FrankieeW.

PRs require approval from at least one maintainer before merging. Release
tags (`v*`) are managed by the lead and pushed to trigger
`.github/workflows/publish.yml`, which builds macOS DMGs and updates the
Homebrew tap.

## Upstream

This project is a fork of [robinebers/openusage](https://github.com/robinebers/openusage).
For features that fit upstream (not specific to Hub/multi-source/env), please
open the PR there first. For fork-specific changes, open the PR here.

## Questions?

[Open an issue](https://github.com/FrankieeW/openusage/issues/new) on this
repo, or check the upstream issue tracker for general questions about the
core app.
