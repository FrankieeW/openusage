# Upstream Sync Point

**Last synced**: 2026-06-14

**Upstream commit**: `35f3188` — Merge pull request #577 from robinebers/codex/codex-rate-limit-resets

**Upstream repo**: `git@github.com:robinebers/openusage.git`

**Upstream version**: v0.6.27

## How to sync future upstream changes

```fish
# 1. Fetch latest upstream
git fetch upstream main

# 2. See what changed (replace 35f3188 with actual commit from above)
git diff 35f3188..upstream/main

# 3. Merge
git merge upstream/main

# 4. Update this file with the new commit hash and date
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
