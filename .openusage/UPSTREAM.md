# Upstream Sync Point

**Last synced**: 2026-06-14

**Upstream commit**: `35f3188` — Merge pull request #577 from robinebers/codex/codex-rate-limit-resets

**Upstream repo**: `git@github.com:robinebers/openusage.git`

**Upstream version**: v0.6.27

## How to sync future upstream changes (PR workflow)

```fish
# 1. Fetch latest upstream
git fetch upstream main

# 2. See the diff
git diff 35f3188..upstream/main

# 3. Create a sync branch from your main
git checkout main
git checkout -b sync/upstream-YYYY-MM-DD

# 4. Merge upstream into the sync branch
git merge upstream/main --no-ff

# 5. Push and open a PR on GitHub
git push -u origin sync/upstream-YYYY-MM-DD
# → Open PR: sync/upstream-YYYY-MM-DD → main
# → Review diff, resolve conflicts, then merge via GitHub UI

# 6. Update this file with the new upstream commit hash and date
```

## Fork-specific changes on top of upstream

- **Plugin Hub** (`feat/plugin-hub` branch): external plugin sources, install/uninstall from Hub UI
- See `docs/superpowers/specs/2026-06-14-plugin-hub-design.md` for architecture
- Key files to watch for merge conflicts:
  - `src-tauri/src/lib.rs` (AppState, command registration)
  - `src-tauri/src/plugin_engine/mod.rs` (bundled copy removed, reload_from_install_dir)
  - `src-tauri/tauri.conf.json` (bundle:plugins removed from beforeBuildCommand)
  - `src-tauri/Cargo.toml` (tokio time/process features)
  - `package.json` (bundle:plugins script removed)
  - `copy-bundled.cjs` (deleted)
  - `.github/workflows/publish.yml` (Bundle + Verify steps removed)
