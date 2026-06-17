---
comet_change: plugin-hub
role: technical-design
canonical_spec: openspec
---

# Plugin Hub — Design

Date: 2026-06-14
Status: Approved (pending written-spec user review)

Compatibility update: package identity, cross-source comparison, branch source
handling, and update status are specified in
`docs/superpowers/specs/2026-06-17-plugin-package-identity-design.md`.
That document supersedes this file's older `updateAvailable: boolean` and
version-only install metadata model.

## Summary

Replace the built-in plugin model with a Hub: a UI page where the user subscribes to one or more sources (GitHub repos, generic git hosts, local paths), browses available plugins from each, and installs/uninstalls them. Discovery/install stays data-driven via existing `plugin.json` schema and `__openusage_plugin.probe(ctx)` contract — Hub is the supply side, plugin engine is the runtime.

Default source `https://github.com/robinebers/openusage` is pre-registered on first launch with `autoCheck: false` and no auto-install. The app starts with no installed plugins; upstream is only the default source to browse from, not an embedded plugin bundle. Existing upstream `plugin.json` schema is the sole publisher contract; Hub writes OpenUsage-owned install metadata next to installed plugins so it can track source ownership without changing publisher manifests.

## Goals

- Add/install plugins without rebuilding the Tauri app
- Pull plugins from any source that follows the existing convention (`plugins/<id>/{plugin.json, plugin.js, icon.svg}`)
- Hot-reload installed plugin set without restarting the menubar app
- Survive `git pull upstream main` — all Hub code lives in new modules, with only small additive wiring in existing app/plugin files

## Non-Goals (this iteration)

- Plugin signing/verification beyond trusting the source URL the user typed
- Plugin ratings, reviews, search, screenshots in browse UI
- Auto-update push notifications (manual refresh + optional launch-time check only)
- Marketplace/centralized registry
- True concurrent runtime installs for the same `pluginId`
- Generic plugin dependencies or version constraints
- GitHub-API-optimized fetch path (deferred — `github.rs` ships as TODO stub; GitHub and GenericGit sources both require the local `git` binary this iteration)
- Symlink-based zero-copy install (rejected: Windows compat)

## Architecture

### Constraints driving layout

- `src-tauri/src/plugin_engine/*` (upstream-owned) must stay minimally modified
- JS bundle (`vite.config.ts`) must not statically import any plugin
- Existing runtime contract (`globalThis.__openusage_plugin.probe(ctx)` returning `{ plan?, lines: [...] }`) must stay valid — plugins from any source run through the same installed-plugin runtime

### Module layout (all new files)

```
src-tauri/src/hub/
  mod.rs              # public API + HubError + command entry points
  source.rs           # Source / SourceKind types + URL canonicalization
  git_ops.rs          # clone / fetch / reset / sparse checkout
  github.rs           # TODO stub — future GitHub REST API optimization
  registry.rs         # app_data_dir/hub/sources.json atomic read/write
  install.rs          # copy from cache → plugins/, write install metadata, trigger hot reload, orphan sweep

src/lib/hub/
  commands.ts         # thin invoke() wrappers (Rust → typed HubError)
  types.ts            # Source / PluginInfo / HubBrowseView / UpdateInfo / HubError
  cache.ts            # zustand store, loading flags, error toast wiring
  labels.ts           # HubErrorCode → user-facing English strings

src/pages/
  hub-page.tsx        # activeView="hub" page, rendered by AppContent
  hub-page.test.tsx

src/components/hub/
  source-list.tsx
  source-card.tsx
  plugin-browser.tsx
  plugin-card.tsx
```

### Touched upstream files (kept minimal)

- `src-tauri/src/plugin_engine/mod.rs`: remove bundled-plugin copy from normal startup, expose a reusable load-from-install-dir helper, and add a reload function that only reloads `app_data_dir/plugins`.
- `src-tauri/src/lib.rs`: store `plugins_dir` in `AppState`, register new commands in `tauri::generate_handler!` (one-liner per command), and add optional `hubAutoCheck` boot hook in `setup()`.
- `src/components/side-nav.tsx`: add "Hub" nav item using a Hugeicons solid-rounded icon and `activeView="hub"`.
- `src/components/app/app-content.tsx`: render `HubPage` when `activeView === "hub"`.
- `src/stores/app-preferences-store.ts` and `src/lib/settings.ts`: add persisted `hubAutoCheck: boolean` (default `false`).
- `copy-bundled.cjs`, `package.json`, and any build config that invokes bundled-plugin copying: remove bundled plugin pipeline so builds do not embed upstream plugins.

### Untouched (upstream-owned, must stay clean for `git pull`)

- `src-tauri/src/plugin_engine/{manifest,runtime,host_api}.rs`
- `src-tauri/src/lib.rs` plugin command bodies
- `src/lib/plugin-types.ts`, `src/stores/app-plugin-store.ts`
- `vite.config.ts` unless it directly invokes the removed bundled-plugin copy path
- `src-tauri/Cargo.toml` — additive changes only:
  - `[dependencies] tokio`: add `"time"` feature (currently `["rt-multi-thread", "macros"]`) — needed for `tokio::time::timeout` in `git_ops.rs`
  - `[dev-dependencies]`: add `tempfile = "3"` for integration test fixture
  - No new crates. `reqwest`, `serde`, `serde_json`, `uuid`, and `tauri-plugin-store` already present.

### Decision: remove bundled plugin pipeline

Remove `copy-bundled.cjs` and any package/build wiring that copies bundled plugins. The default upstream repository is just a Hub source. First launch should create `app_data_dir/plugins/` as an empty install directory, then users install chosen plugins from the default source.

## Rust API

```rust
pub struct Source {
    pub id: String,
    pub label: String,
    pub url: String,
    pub kind: SourceKind,
    pub added_at: i64,
    pub last_refreshed_at: Option<i64>,
    pub auto_check: bool,
}

pub enum SourceKind { Github, GenericGit, LocalPath }

pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub brand_color: Option<String>,
    pub icon_data_url: Option<String>,
    pub source_id: String,
    pub installed: bool,
    pub installed_source_id: Option<String>,
    pub unmanaged: bool,
    pub installed_version: Option<String>,
    pub available_version: String,
    pub package_status: PackageStatus,
}

pub struct HubBrowseView {
    pub source: Source,
    pub available: Vec<PluginInfo>,
    pub skipped: Vec<SkippedPlugin>,  // { path, reason }
}

pub struct UpdateInfo { pub source_id: String; pub plugin_id: String; pub from: String; pub to: String; pub package_hash: String }

pub struct InstallMetadata {
    pub schema_version: u32,
    pub source_id: String,
    pub source_url: String,
    pub source_ref: Option<String>,
    pub source_commit_sha: Option<String>,
    pub plugin_id: String,
    pub installed_version: String,
    pub package_hash: String,
    pub installed_at: i64,
}

pub enum HubError {
    InvalidUrl,
    GitNotInstalled,
    CloneFailed(String),
    NotFound,
    Conflict(String),         // carries conflicting source_id, or "unmanaged"
    IoError(String),
    ManifestParse(String),
}

#[tauri::command]
pub async fn hub_list_sources(app: AppHandle) -> Result<Vec<Source>, HubError>;
#[tauri::command]
pub async fn hub_add_source(app: AppHandle, url: String, label: Option<String>) -> Result<Source, HubError>;
#[tauri::command]
pub async fn hub_remove_source(app: AppHandle, source_id: String) -> Result<(), HubError>;
#[tauri::command]
pub async fn hub_browse_source(app: AppHandle, source_id: String) -> Result<HubBrowseView, HubError>;
#[tauri::command]
pub async fn hub_install(app: AppHandle, source_id: String, plugin_id: String) -> Result<(), HubError>;
#[tauri::command]
pub async fn hub_uninstall(app: AppHandle, plugin_id: String) -> Result<(), HubError>;
#[tauri::command]
pub async fn hub_refresh_source(app: AppHandle, source_id: String) -> Result<HubBrowseView, HubError>;
#[tauri::command]
pub async fn hub_check_updates(app: AppHandle) -> Result<Vec<UpdateInfo>, HubError>;
```

### URL canonicalization (`source.rs`)

Input → output:

| Input | Output |
|---|---|
| `robinebers/openusage` | Github, `https://github.com/robinebers/openusage` |
| `https://github.com/foo/bar` | Github |
| `git@github.com:foo/bar.git` | Github, `https://github.com/foo/bar` |
| `https://gitlab.com/foo/bar` | GenericGit |
| `/Users/me/repos/foo` (exists) | LocalPath |
| `file:///tmp/foo` | LocalPath |
| `ftp://...` | InvalidUrl |
| `github.com/foo` | InvalidUrl (missing repo) |

### Git operations (`git_ops.rs`)

- First fetch: `git clone --depth=1 <url> <cache_dir>`
- Refresh: `git -C <cache_dir> fetch --depth=1 origin HEAD && git -C <cache_dir> reset --hard FETCH_HEAD`
- Discover: walk `cache_dir/plugins/*/plugin.json`
- LocalPath sources skip clone/fetch; discovery walks `<local_path>/plugins/*/plugin.json`, and install still copies from that path into `app_data_dir/plugins`
- All git calls wrapped in `tokio::time::timeout(HUB_*_TIMEOUT_SECS)`
- Boot-time detection: `Command::new("git").arg("--version").output()` once; failure → GitHub and GenericGit sources refused at add-time, LocalPath sources still work

### Install + hot reload (`install.rs`)

1. Verify `cache/<source>/plugins/<id>/plugin.json` exists, `id` matches dir name
2. Conflict check:
   - If `plugins/<id>/.openusage-install.json` exists with a different `source_id` → `HubError::Conflict(other_source_id)`
   - If `plugins/<id>/` exists without Hub metadata → `HubError::Conflict("unmanaged")` so local-dev or manually copied plugin dirs are not overwritten silently
   - Same source and same package hash -> idempotent success, no recopy
3. Copy entire dir `cache/<source>/plugins/<id>/` → `plugins/<id>/`
4. Write `plugins/<id>/.openusage-install.json` with `InstallMetadata`; this file is OpenUsage-owned and is not part of publisher `plugin.json`
5. Call `plugin_engine::reload(app_handle)` which:
   - locks `AppState.plugins` Mutex
   - reruns `load_active_plugins_from_dir(app_state.plugins_dir)`
   - drops lock
   - `app.emit("plugins-changed", new_meta)`
6. Frontend listener calls existing `app-plugin-store.setPluginsMeta`

### Hot reload event

Tauri event `plugins-changed` payload: `Vec<PluginMeta>` (existing type from `src/lib/plugin-types.ts`). Frontend listener registered once in `src/lib/hub/cache.ts` init; calls existing `setPluginsMeta` action — no store schema change.

## JS / UI

### TypeScript mirror (`src/lib/hub/types.ts`)

```ts
export type SourceKind = "Github" | "GenericGit" | "LocalPath"
export interface Source { id: string; label: string; url: string; kind: SourceKind; addedAt: number; lastRefreshedAt: number | null; autoCheck: boolean }
export type PackageStatus = "notInstalled" | "installed" | "updateAvailable" | "sourceChanged" | "installedNewerThanSource" | "samePackageFromOtherSource" | "differentPackageSamePluginId" | "unmanagedInstalled" | "orphanedSource"
export interface PluginInfo { id: string; name: string; brandColor: string | null; iconDataUrl: string | null; sourceId: string; installed: boolean; installedSourceId: string | null; unmanaged: boolean; installedVersion: string | null; availableVersion: string; packageHash: string; packageStatus: PackageStatus }
export interface HubBrowseView { source: Source; available: PluginInfo[]; skipped: SkippedPlugin[] }
export interface UpdateInfo { sourceId: string; pluginId: string; from: string; to: string }
export type HubErrorCode = "InvalidUrl" | "GitNotInstalled" | "CloneFailed" | "NotFound" | "Conflict" | "IoError" | "ManifestParse"
export interface HubError { code: HubErrorCode; message: string; context?: Record<string, unknown> }
export interface SkippedPlugin { path: string; reason: string }
```

### Zustand store (`src/lib/hub/cache.ts`)

State:
```ts
{
  sources: Source[]
  browseBySource: Record<string, HubBrowseView>
  loading: {
    sources: boolean
    perSource: Record<string, boolean>      // browsing/refreshing
    perPlugin: Record<string, "install" | "uninstall" | null>
  }
  error: HubError | null
}
```

Actions: `refreshSources()`, `browseSource(id, force?)`, `install(sourceId, pluginId)`, `uninstall(pluginId)`, `addSource(url, label?)`, `removeSource(id)`, `updateSource(id, patch)`. All wrap invoke() + manage loading + catch errors into `store.error` (drives toast).

### Page layout (`src/pages/hub-page.tsx`)

```
┌─────────────────────────────────────────────────┐
│ Plugin Hub                       [+ Add Source] │
├─────────────────────────────────────────────────┤
│ ▼ Upstream (default)         [↻] [⚙] [✕]        │
│   ✓ claude           v0.6.27  [installed]       │
│   ✓ codex            v0.6.27  [installed]       │
│   ☐ devin            v0.6.27  [Install]         │
│   ☐ minimax          v0.6.27  [Install]         │
│                                                 │
│ ▶ Another user source       [↻] [⚙] [✕]        │
│   (collapsed — click to browse)                 │
└─────────────────────────────────────────────────┘
```

Source card controls:
- Hugeicons refresh button (calls `hub_refresh_source`)
- Hugeicons settings button (edit label, toggle `autoCheck`, delete with confirm)
- Hugeicons close/delete button (with confirm)
- For `LocalPath`: badge "Local Source" next to label. Local sources are copied on install like git sources; editing the source path requires refresh + reinstall to update the installed copy.

Plugin card:
- Icon (from `iconDataUrl`), name, version
- State: not installed → `[Install]`; installed → `[Installed] [Uninstall]`; installed + update → `[Update to vN]`
- Orphan state: Hub metadata points to a removed source → `⚠ Source removed` gray, only `[Uninstall]` shown
- Unmanaged state: existing plugin dir has no Hub metadata → `Managed Outside Hub` gray, no overwrite; user can uninstall only if they choose to remove the existing plugin dir through Hub confirmation

Add Source modal:
- URL input
- Live detection: as user types, show derived `kind` + normalized URL preview
- Optional label field
- `[Add & Fetch]` button (closes modal, triggers add + initial browse)

### Navigation

- `Hub` added as a top-level item in `src/components/side-nav.tsx` (same level as Home / Settings)
- No app router is introduced in this iteration. `src/stores/app-ui-store.ts` keeps using `activeView`; Hub uses `activeView === "hub"`.
- Conflict error toast includes an action that sets `activeView` to `"hub"` and stores `highlightSourceId` in the Hub store so the page can scroll the conflicting source card into view.

### Auto-check at launch

- `hubAutoCheck: boolean` in `app-preferences-store.ts`, default `false`
- `setup()` in `lib.rs`: if `hubAutoCheck && !sources.is_empty()` → spawn background task `hub_check_updates()`, on success `app.emit("hub-updates-available", Vec<UpdateInfo>)`, frontend toast "N plugin updates available"
- Failure per-source → that source marked offline, others continue, toast summarizes "N sources unreachable"

## Persistence

### Disk layout

```
app_data_dir/
  hub/
    sources.json          # registry, owned by src-tauri/src/hub/registry.rs
    cache/
      <source_id>/        # cloned source
        plugins/<id>/...
  plugins/
    <id>/                 # installed plugins (shared with plugin_engine)
      plugin.json
      plugin.js
      icon.svg
      .openusage-install.json  # Hub install metadata, absent for unmanaged/manual plugin dirs
```

### `sources.json` shape

```json
{
  "version": 1,
  "sources": [
    {
      "id": "uuid-xxx",
      "label": "Upstream (default)",
      "url": "https://github.com/robinebers/openusage",
      "kind": "Github",
      "addedAt": 1234567890,
      "lastRefreshedAt": 1234567890,
      "autoCheck": false
    }
  ]
}
```

### Install metadata shape

```json
{
  "schemaVersion": 2,
  "sourceId": "uuid-xxx",
  "sourceUrl": "https://github.com/robinebers/openusage",
  "sourceRef": "main",
  "sourceCommitSha": "abcdef...",
  "pluginId": "codex",
  "installedVersion": "0.6.27",
  "packageHash": "sha256:...",
  "installedAt": 1234567890
}
```

Legacy metadata without `schemaVersion` is treated as v1 and remains readable.
Missing `packageHash` is backfilled by hashing the installed plugin directory.

### Write strategy

`registry.rs` owns `sources.json` directly instead of using `tauri-plugin-store`. Single `registry::persist()` writes to `sources.json.tmp` then renames — prevents partial writes on crash. `version` field guards future migrations: missing or `!= 1` → backup `.bak` + rebuild.

### Defaults

On first launch (`sources.json` missing or invalid): insert single default source `{ url: "https://github.com/robinebers/openusage", kind: Github, label: "Upstream (default)", autoCheck: false }`.

## Error Handling

### UI behavior matrix (`src/lib/hub/labels.ts`)

| code | UI behavior | Recovery |
|---|---|---|
| `InvalidUrl` | Add-source modal inline red text | Edit URL |
| `GitNotInstalled` | Toast + `[Open git-scm.com]` link for GitHub/GenericGit sources | Install git or use LocalPath |
| `CloneFailed(msg)` | Toast "Clone failed: {msg}", source card marked `offline` (gray) | Retry refresh |
| `NotFound` | Toast "Plugin missing in source" | Refresh source |
| `Conflict(otherId)` | Toast "Already installed from <label>. Uninstall first." with Hub action to highlight that source. For `"unmanaged"`, toast "Already installed outside Hub." | Uninstall, retry |
| `IoError(msg)` | Toast "Disk error: {msg}" | Check disk space |
| `ManifestParse(msg)` | Browse view shows skipped count; toast "Skipped N plugins with bad manifest", details in console | Report to source author |

### Network/offline policy

- Add source: clone failure → source not added to registry, no half-state
- Refresh source: failure → `browseBySource` cache preserved, card shows "offline (cached: <time>)", Install buttons still work against cached copy
- Launch auto-check: per-source isolation, summary toast on partial failure

### Timeouts (`src-tauri/src/hub/mod.rs`)

```rust
pub const HUB_CLONE_TIMEOUT_SECS: u64 = 60;
pub const HUB_REFRESH_TIMEOUT_SECS: u64 = 30;
pub const HUB_HTTP_TIMEOUT_SECS: u64 = 15;  // reserved for github.rs
```

### Manifest validation

`hub_install` / `hub_refresh_source` parse each `plugin.json` and skip with `SkippedPlugin { path, reason }` if any check fails:
- Required field missing
- `id` ≠ directory name
- `entry` resolves outside plugin dir (path traversal)
- `schemaVersion != 1` (deferred — silently skip until upstream adds v2)

## Concurrency

- **Concurrent refresh of same source**: second call awaits first's in-flight result via per-source `tokio::sync::Mutex<HashMap<SourceId, JoinHandle>>` (de-dupe)
- **Concurrent install/uninstall**: serialize through `AppState.plugins` Mutex in `reload()`
- **In-flight probe during reload**: probe holds `Arc<LoadedPlugin>` clone; `reload()` swaps `Vec<LoadedPlugin>` under lock; running probe completes against old data, next batch uses new

## Orphan handling

`install.rs::hub_startup_sweep()` runs at end of `setup()`, before plugin engine init:

1. Read registry → set of valid `source_id`s
2. Walk `cache/` → each subdir not in set → `fs::remove_dir_all` + `log::info!("removed orphan cache {id}")`
3. Walk `plugins/` → for each plugin with `.openusage-install.json`, if `source_id` is missing from registry, mark source removed; do not delete automatically
4. Plugins without `.openusage-install.json` are unmanaged; log only and do not show as Hub-owned
5. Frontend marks managed removed-source plugins with `⚠ Source removed` badge, but keeps Uninstall working

Boot cost: ~5-20ms (walk + stat), acceptable.

## Testing

- Rust unit tests cover source URL parsing, registry persistence, install
  metadata, path validation, conflicts, and orphan sweep.
- Rust integration coverage uses a LocalPath fixture with valid and malformed
  plugins.
- TypeScript tests cover command parameter shape, store loading/error states,
  labels, component interactions, conflict highlighting, and orphan/unmanaged
  rendering.
- Manual smoke covers GitHub install, LocalPath refresh/reinstall, offline
  refresh, missing git, source removal, and upstream sync conflict surface.
- Package hash, source ref, metadata v2 migration, and package status tests are
  specified in `docs/superpowers/specs/2026-06-17-plugin-package-identity-design.md`.

## Decisions Log

| # | Question | Decision |
|---|---|---|
| 1 | External plugin motivation | Hub model: user types source URL, picks plugins |
| 2 | Source host scope | GitHub + GenericGit + LocalPath |
| 3 | First-launch defaults | Pre-register default upstream source, no auto-install |
| 4 | Update mechanism | Manual refresh button + optional launch auto-check (toggle, default off) |
| 5 | Plugin manifest | Existing `plugin.json` schema only, no new `hub.json` |
| 6 | Architecture | Rust-led (Hub is supply-side to existing engine) |
| 7 | GitHub API optimization | Deferred; stub module, GitHub/GenericGit require local `git` |
| 8 | Conflict policy | Compare same IDs by package hash; require explicit replace for different packages |
| 9 | Hot reload | Tauri event `plugins-changed`, existing store listener pattern |
| 10 | Bundled plugins | Remove bundled plugin copy path; upstream repo is only a default source |
| 11 | Local path source UX | Badge "Local Source"; install still copies into `app_data_dir/plugins` |
| 12 | Schema versioning | Strict: `schemaVersion != 1` skipped silently |
| 13 | Default source autoCheck | false (consistent with global default off) |
| 14 | Conflict toast UX | Toast action opens Hub activeView and highlights the conflicting source |
| 15 | Orphan UI marker | `⚠ Source removed` badge; uninstall still works |
| 16 | Install ownership | Hub writes `.openusage-install.json`; publisher `plugin.json` stays unchanged |
| 17 | Package identity | `pluginId + version + packageHash`; source identity records URL, ref, and commit SHA |

## Out of Scope (future work)

- GitHub API-optimized fetch path (`github.rs` stub)
- Plugin signing / publisher key verification
- Marketplace / curated plugin list
- Per-plugin permissions UI (read filesystem, network, keychain — currently all-or-nothing per plugin via host API)
- Auto-update background polling (vs. launch-time check only)
- Plugin rollback / version pinning
- Cross-platform git binary auto-install prompt
- GitHub tarball/API fallback when `git` is missing
- i18n for Hub UI labels

## Compatibility Notes

- Upstream sync: all Hub files are in new directories (`src-tauri/src/hub/`, `src/lib/hub/`, `src/components/hub/`, `src/pages/hub-page.tsx`); only small wiring changes land in existing app/plugin files, plus removal of bundled-plugin copy wiring. `git pull upstream main` should produce zero or trivially-resolvable conflicts.
- Users without sources.json: first Hub open inserts default upstream source. Installed plugin set remains empty unless the user installs from a source.
- Existing plugin dirs without `.openusage-install.json` are treated as unmanaged, not Hub-owned. They remain available through the existing plugin engine and are never overwritten by Hub install.
- Removed upstream plugins (e.g. `windsurf`): if a user manually has them in `plugins/`, they remain unmanaged; otherwise they simply disappear from the default source browse results after refresh.

## Upstream Relationship Strategy

- Treat `https://github.com/robinebers/openusage` as a normal source record, not as bundled application content.
- Do not mirror upstream plugins into the app bundle during build. Source refresh is the only way Hub learns what upstream currently publishes.
- Keep publisher-facing contracts upstream-compatible: no `hub.json`, no extra required fields in `plugin.json`, and no upstream repo changes required.
- Keep fork-local metadata outside publisher files in `.openusage-install.json`.
- After each upstream sync, check only the narrow integration points: plugin manifest schema, plugin runtime load helper, `AppState` shape, command registration, and default source path convention.

## User Docs To Update

- `docs/plugins/schema.md`: mention Hub-discovered plugins still use the same `plugin.json` schema, and `.openusage-install.json` is app-owned metadata that publishers must not provide.
- `docs/plugins/api.md`: no runtime API contract change expected; review for wording that assumes only bundled plugins exist.
