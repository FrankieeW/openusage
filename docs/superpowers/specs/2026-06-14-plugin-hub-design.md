# Plugin Hub — Design

Date: 2026-06-14
Status: Approved (pending written-spec user review)

## Summary

Replace the built-in-only plugin model with a Hub: a UI page where the user subscribes to one or more sources (GitHub repos, generic git hosts, local paths), browses available plugins from each, and installs/uninstalls them. Discovery/install stays data-driven via existing `plugin.json` schema and `__openusage_plugin.probe(ctx)` contract — Hub is the supply side, plugin engine is the runtime.

Default source `https://github.com/robinebers/openusage` is pre-registered on first launch with `autoCheck: false` and no auto-install. Existing upstream `plugin.json` schema is the sole contract; no new manifest required from publishers.

## Goals

- Add/install plugins without rebuilding the Tauri app
- Pull plugins from any source that follows the existing convention (`plugins/<id>/{plugin.json, plugin.js, icon.svg}`)
- Hot-reload installed plugin set without restarting the menubar app
- Survive `git pull upstream main` — all Hub code lives in new modules, zero edits to upstream-touched files beyond ~10 lines in `plugin_engine/mod.rs`

## Non-Goals (this iteration)

- Plugin signing/verification beyond trusting the source URL the user typed
- Plugin ratings, reviews, search, screenshots in browse UI
- Auto-update push notifications (manual refresh + optional launch-time check only)
- Marketplace/centralized registry
- Cross-source plugin ID reconciliation beyond refuse-on-conflict
- Generic plugin dependencies or version constraints
- GitHub-API-optimized fetch path (deferred — `github.rs` ships as TODO stub, all sources go through `git_ops.rs`)
- Symlink-based zero-copy install (rejected: Windows compat)

## Architecture

### Constraints driving layout

- `src-tauri/src/plugin_engine/*` (upstream-owned) must stay minimally modified
- JS bundle (`vite.config.ts`) must not statically import any plugin
- Existing runtime contract (`globalThis.__openusage_plugin.probe(ctx)` returning `{ plan?, lines: [...] }`) must stay valid — plugins from any source run identically to bundled ones

### Module layout (all new files)

```
src-tauri/src/hub/
  mod.rs              # public API + HubError + command entry points
  source.rs           # Source / SourceKind types + URL canonicalization
  git_ops.rs          # clone / fetch / reset / sparse checkout
  github.rs           # TODO stub — future GitHub REST API optimization
  registry.rs         # sources.json read/write via tauri-plugin-store
  install.rs          # copy from cache → plugins/, trigger hot reload, orphan sweep

src/lib/hub/
  commands.ts         # thin invoke() wrappers (Rust → typed HubError)
  types.ts            # Source / PluginInfo / HubBrowseView / UpdateInfo / HubError
  cache.ts            # zustand store, loading flags, error toast wiring
  labels.ts           # HubErrorCode → user-facing English strings

src/pages/
  hub-page.tsx        # single new route /hub
  hub-page.test.tsx

src/components/hub/
  source-list.tsx
  source-card.tsx
  plugin-browser.tsx
  plugin-card.tsx
```

### Touched upstream files (kept minimal)

- `src-tauri/src/plugin_engine/mod.rs`: extract `initialize_plugins` body into reusable `pub fn reload(app_handle: &AppHandle)` (~10 lines), call from both `setup()` and `install.rs::hub_install` / `hub_uninstall`.
- `src-tauri/src/lib.rs`: register new commands in `tauri::generate_handler!` (one-liner per command); optional `hubAutoCheck` boot hook in `setup()`.
- `src/components/side-nav.tsx`: add "Hub" nav item linking to `/hub`.
- `src/stores/app-preferences-store.ts`: add `hubAutoCheck: boolean` (default `false`).
- `src/App.tsx` (or router config): register `/hub` route.

### Untouched (upstream-owned, must stay clean for `git pull`)

- `src-tauri/src/plugin_engine/{manifest,runtime,host_api}.rs`
- `src-tauri/src/lib.rs` plugin command bodies
- `src/lib/plugin-types.ts`, `src/stores/app-plugin-store.ts`
- `vite.config.ts`, `copy-bundled.cjs`, `package.json`
- `src-tauri/Cargo.toml` — additive changes only:
  - `[dependencies] tokio`: add `"time"` feature (currently `["rt-multi-thread", "macros"]`) — needed for `tokio::time::timeout` in `git_ops.rs`
  - `[dev-dependencies]`: add `tempfile = "3"` for integration test fixture
  - No new crates. `reqwest`, `serde`, `serde_json`, `tauri-plugin-store` already present.

### Decision: keep `copy-bundled.cjs`?

Defer. If `0.6.27` ships no bundled plugins (per default-source-not-auto-install decision), the script becomes dead code. Remove in same PR or follow-up; tracked but not blocking.

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
    pub installed_version: Option<String>,
    pub available_version: String,
    pub update_available: bool,
}

pub struct HubBrowseView {
    pub source: Source,
    pub available: Vec<PluginInfo>,
    pub skipped: Vec<SkippedPlugin>,  // { path, reason }
}

pub struct UpdateInfo { pub source_id: String; pub plugin_id: String; pub from: String; pub to: String }

pub enum HubError {
    InvalidUrl,
    GitNotInstalled,
    CloneFailed(String),
    TarballFailed(String),    // reserved for github.rs
    RateLimited,
    NotFound,
    Conflict(String),         // carries conflicting source_id
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
- All git calls wrapped in `tokio::time::timeout(HUB_*_TIMEOUT_SECS)`
- Boot-time detection: `Command::new("git").arg("--version").output()` once; failure → GenericGit sources refused at add-time, LocalPath/Github still work

### Install + hot reload (`install.rs`)

1. Verify `cache/<source>/plugins/<id>/plugin.json` exists, `id` matches dir name
2. Conflict check: if `plugins/<id>/` exists with different `source_id` → `HubError::Conflict(other_source_id)`. Same source → idempotent success, no recopy.
3. `fs::copy` entire dir `cache/<source>/plugins/<id>/` → `plugins/<id>/`
4. Call `plugin_engine::reload(app_handle)` which:
   - locks `AppState.plugins` Mutex
   - reruns `load_plugins_from_dir` (same body as `setup()`)
   - drops lock
   - `app.emit("plugins-changed", new_meta)`
5. Frontend listener calls existing `app-plugin-store.setPluginsMeta`

### Hot reload event

Tauri event `plugins-changed` payload: `Vec<PluginMeta>` (existing type from `src/lib/plugin-types.ts`). Frontend listener registered once in `src/lib/hub/cache.ts` init; calls existing `setPluginsMeta` action — no store schema change.

## JS / UI

### TypeScript mirror (`src/lib/hub/types.ts`)

```ts
export type SourceKind = "Github" | "GenericGit" | "LocalPath"
export interface Source { id: string; label: string; url: string; kind: SourceKind; addedAt: number; lastRefreshedAt: number | null; autoCheck: boolean }
export interface PluginInfo { id: string; name: string; brandColor: string | null; iconDataUrl: string | null; sourceId: string; installed: boolean; installedVersion: string | null; availableVersion: string; updateAvailable: boolean }
export interface HubBrowseView { source: Source; available: PluginInfo[]; skipped: SkippedPlugin[] }
export interface UpdateInfo { sourceId: string; pluginId: string; from: string; to: string }
export type HubErrorCode = "InvalidUrl" | "GitNotInstalled" | "CloneFailed" | "TarballFailed" | "RateLimited" | "NotFound" | "Conflict" | "IoError" | "ManifestParse"
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
- `↻` refresh (calls `hub_refresh_source`)
- `⚙` settings popover (edit label, toggle `autoCheck`, delete with confirm)
- `✕` quick delete (with confirm)
- For `LocalPath`: badge "Local (no copy)" next to label

Plugin card:
- Icon (from `iconDataUrl`), name, version
- State: not installed → `[Install]`; installed → `[Installed] [Uninstall]`; installed + update → `[Update to vN]`
- Orphan state: source deleted → `⚠ Source removed` gray, only `[Uninstall]` shown

Add Source modal:
- URL input
- Live detection: as user types, show derived `kind` + normalized URL preview
- Optional label field
- `[Add & Fetch]` button (closes modal, triggers add + initial browse)

### Routing

- `Hub` added as a top-level item in `src/components/side-nav.tsx` (same level as Overview / Settings)
- Route `/hub` registered in app router
- Deep link: `/hub?source=<id>` — used by `Conflict` error toast to jump to the conflicting source's card and scroll into view

### Auto-check at launch

- `hubAutoCheck: boolean` in `app-preferences-store.ts`, default `false`
- `setup()` in `lib.rs`: if `hubAutoCheck && !sources.is_empty()` → spawn background task `hub_check_updates()`, on success `app.emit("hub-updates-available", Vec<UpdateInfo>)`, frontend toast "N plugin updates available"
- Failure per-source → that source marked offline, others continue, toast summarizes "N sources unreachable"

## Persistence

### Disk layout

```
app_data_dir/
  hub/
    sources.json          # registry (Tauri store, key="hub.sources")
    cache/
      <source_id>/        # cloned source
        plugins/<id>/...
  plugins/
    <id>/                 # installed plugins (shared with plugin_engine)
      plugin.json
      plugin.js
      icon.svg
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

### Write strategy

Single `registry::persist()` writes to `sources.json.tmp` then renames — prevents partial writes on crash. `version` field guards future migrations: missing or `!= 1` → backup `.bak` + rebuild.

### Defaults

On first launch (`sources.json` missing or invalid): insert single default source `{ url: "https://github.com/robinebers/openusage", kind: Github, label: "Upstream (default)", autoCheck: false }`.

## Error Handling

### UI behavior matrix (`src/lib/hub/labels.ts`)

| code | UI behavior | Recovery |
|---|---|---|
| `InvalidUrl` | Add-source modal inline red text | Edit URL |
| `GitNotInstalled` | Toast + `[Open git-scm.com]` link | Install git |
| `CloneFailed(msg)` | Toast "Clone failed: {msg}", source card marked `offline` (gray) | Retry refresh |
| `RateLimited` | Toast "GitHub rate limit, retry in ~{min}min" | Wait |
| `NotFound` | Toast "Plugin missing in source" | Refresh source |
| `Conflict(otherId)` | Toast "Already installed from <label>. Uninstall first." with deep-link to that source card | Uninstall, retry |
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
3. Walk `plugins/` → each plugin not in any source's latest browse view → log only, **do not delete** (could be local dev / bundled leftover)
4. Frontend marks such plugins with `⚠ Source removed` badge, but keeps Uninstall working

Boot cost: ~5-20ms (walk + stat), acceptable.

## Testing

### Rust unit tests (inline `#[cfg(test)]`)

- `source.rs` URL canonicalization table (12 cases)
- `registry.rs` round-trip + tmp-file recovery + version migration
- `install.rs` path validation (entry traversal, id mismatch), idempotent same-source install, Conflict on cross-source install
- `hub_startup_sweep` orphan detection (3 cases)

### Rust integration test (`src-tauri/tests/hub_e2e.rs`)

`tempfile::TempDir` simulating `app_data_dir`, fixture repo at `tests/fixtures/sample-source/`:
- 2 valid plugins + 1 malformed `plugin.json`
- Add LocalPath source → browse → 2 available + 1 skipped
- Install → file copied → listable via `plugin_engine` reload
- Uninstall → directory removed
- Remove source → cache cleared, installed plugin preserved

GitHub real-repo test marked `#[ignore]` (rate limits + network flakiness). Manual smoke covers.

### JS unit tests

- `src/lib/hub/cache.test.ts`: install/refresh/uninstall loading flip, Conflict error capture, concurrent refresh dedupe
- `src/lib/hub/labels.test.ts`: snapshot of error code → English string
- `src/lib/hub/commands.test.ts`: invoke param shape, HubError parsing

### JS component tests (`src/pages/hub-page.test.tsx`)

`@testing-library/react` + `vi.mock("@tauri-apps/api/core")`:
- Initial load → sources list rendered
- Source expand → lazy browse, plugin list rendered
- Install click → invoke called, loading spinner shown
- Conflict error → toast text shown, installed state unchanged
- Add-source modal → URL validation inline error, submit triggers invoke
- Delete source → confirm flow: cancel no invoke, confirm invokes + card disappears
- autoCheck toggle → invoke called
- Orphan plugin renders with `⚠ Source removed`

### Manual smoke

- macOS real GitHub clone → install → menubar shows new provider → probe data correct
- Offline refresh → toast + cached install works
- GitHub rate limit → minute-accurate message
- Local path source → edit `plugin.js` → restart app → change visible
- Remove source → orphan marker appears
- `git pull upstream main` → no conflicts in Hub-touched files

### Not testing

- Playwright E2E — internal app, single new page, component tests cover
- 80% coverage gate — new module, key functions (source parse, install validate, sweep) fully covered; commands/labels 100%; component coverage focused on interaction paths

## Decisions Log

| # | Question | Decision |
|---|---|---|
| 1 | External plugin motivation | Hub model: user types source URL, picks plugins |
| 2 | Source host scope | GitHub + GenericGit + LocalPath |
| 3 | First-launch defaults | Pre-register default upstream source, no auto-install |
| 4 | Update mechanism | Manual refresh button + optional launch auto-check (toggle, default off) |
| 5 | Plugin manifest | Existing `plugin.json` schema only, no new `hub.json` |
| 6 | Architecture | Rust-led (Hub is supply-side to existing engine) |
| 7 | GitHub API optimization | Deferred; stub module, all paths use `git_ops.rs` |
| 8 | Conflict policy | Refuse install if installed from different source; uninstall first |
| 9 | Hot reload | Tauri event `plugins-changed`, existing store listener pattern |
| 10 | Bundled `copy-bundled.cjs` | Remove in same PR or follow-up; track but not blocking |
| 11 | Local path source UX | Badge "Local (no copy)" on source card |
| 12 | Schema versioning | Strict: `schemaVersion != 1` skipped silently |
| 13 | Default source autoCheck | false (consistent with global default off) |
| 14 | Conflict toast UX | Deep-link to conflicting source card via `?source=<id>` |
| 15 | Orphan UI marker | `⚠ Source removed` badge; uninstall still works |

## Out of Scope (future work)

- GitHub API-optimized fetch path (`github.rs` stub)
- Plugin signing / publisher key verification
- Marketplace / curated plugin list
- Per-plugin permissions UI (read filesystem, network, keychain — currently all-or-nothing per plugin via host API)
- Auto-update background polling (vs. launch-time check only)
- Plugin rollback / version pinning
- Cross-platform git binary auto-install prompt
- i18n for Hub UI labels

## Compatibility Notes

- Upstream sync: all Hub files are in new directories (`src-tauri/src/hub/`, `src/lib/hub/`, `src/components/hub/`, `src/pages/hub-page.tsx`); only ~10 lines added to `plugin_engine/mod.rs`, one-line additions to `lib.rs` and `side-nav.tsx`, additive-only edits to `Cargo.toml`. `git pull upstream main` should produce zero or trivially-resolvable conflicts.
- Existing users without sources.json: first Hub open inserts default upstream source. Existing bundled plugins remain in `plugins/` until explicitly uninstalled via Hub.
- Removed upstream plugins (e.g. `windsurf`): if user has them installed from before, they remain; on first sweep they're marked orphan if not in default source.