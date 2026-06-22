#[cfg(target_os = "macos")]
mod app_nap;
mod config;
mod local_http_api;
mod log_path;
mod panel;
mod plugin_engine;
mod tray;
mod hub;
#[cfg(target_os = "macos")]
mod webkit_config;

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use serde::Serialize;
use tauri::Emitter;
use tauri_plugin_aptabase::EventTracker;
use tauri_plugin_log::{Target, TargetKind};
use uuid::Uuid;

#[cfg(desktop)]
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

const GLOBAL_SHORTCUT_STORE_KEY: &str = "globalShortcut";
const DAILY_ACTIVE_TRACKED_DAY_KEY: &str = "analytics.daily_active_day";
const DAILY_ACTIVE_EVENT_NAME: &str = "app_started";
const MAX_CONCURRENT_PROBES: usize = 4;

fn probe_worker_count(plugin_count: usize) -> usize {
    plugin_count.min(MAX_CONCURRENT_PROBES)
}

fn today_utc_ymd() -> String {
    let date = time::OffsetDateTime::now_utc().date();
    format!(
        "{:04}-{:02}-{:02}",
        date.year(),
        date.month() as u8,
        date.day()
    )
}

fn should_track_daily_active(last_tracked_day: Option<&str>, today: &str) -> bool {
    match last_tracked_day {
        Some(day) => day != today,
        None => true,
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct EnvOverrideDto {
    name: String,
    kind: String,
    value: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct EnvGroupDto {
    #[allow(dead_code)]
    id: String,
    #[allow(dead_code)]
    name: String,
    enabled: bool,
    overrides: Vec<EnvGroupOverrideDto>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct EnvGroupOverrideDto {
    name: String,
    value: String,
}

/// Flatten groups (frontend-compatible logic). Active groups only;
/// `$REF` → reference, `$$X` → literal `$X`, everything else → literal.
/// Conflicts (same name in >1 active group) become `[CONFLICT: NAME]`.
fn flatten_env_groups(groups: &[EnvGroupDto]) -> Vec<EnvOverrideDto> {
    flatten_selected_env_groups(groups.iter().filter(|group| group.enabled))
}

fn flatten_legacy_env_groups(groups: &[EnvGroupDto], active_ids: &[String]) -> Vec<EnvOverrideDto> {
    let active_set: HashSet<&str> = active_ids.iter().map(|id| id.as_str()).collect();
    flatten_selected_env_groups(
        groups
            .iter()
            .filter(|group| active_set.contains(group.id.as_str())),
    )
}

fn flatten_selected_env_groups<'a>(
    groups: impl Iterator<Item = &'a EnvGroupDto>,
) -> Vec<EnvOverrideDto> {
    // name → (kind, value); None marker means conflict.
    let mut map: HashMap<String, Option<(String, String)>> = HashMap::new();

    for group in groups {
        for o in &group.overrides {
            if o.name.is_empty() || o.value.is_empty() {
                continue;
            }
            let (kind, val) = if o.value.starts_with("$$") {
                ("literal".to_string(), o.value[1..].to_string())
            } else if o.value.starts_with('$') && o.value.len() > 1 {
                ("reference".to_string(), o.value[1..].to_string())
            } else {
                ("literal".to_string(), o.value.clone())
            };
            if kind == "literal" && val.is_empty() {
                continue;
            }
            if map.contains_key(&o.name) {
                // Conflict — mark for replacement.
                map.insert(o.name.clone(), None);
            } else {
                map.insert(o.name.clone(), Some((kind, val)));
            }
        }
    }

    map.into_iter()
        .filter_map(|(name, entry)| match entry {
            Some((kind, value)) => Some(EnvOverrideDto { name, kind, value }),
            None => Some(EnvOverrideDto {
                name: name.clone(),
                kind: "literal".to_string(),
                value: format!("[CONFLICT: {}]", name),
            }),
        })
        .collect()
}

fn map_env_overrides(dtos: Vec<EnvOverrideDto>) -> Vec<plugin_engine::host_api::EnvOverrideInput> {
    use plugin_engine::host_api::{EnvOverrideInput, EnvOverrideKind};
    dtos.into_iter()
        .filter_map(|dto| {
            let kind = match dto.kind.as_str() {
                "literal" => EnvOverrideKind::Literal,
                "reference" => EnvOverrideKind::Reference,
                other => {
                    log::warn!("Ignoring env override with unknown kind: {}", other);
                    return None;
                }
            };
            Some(EnvOverrideInput { name: dto.name, kind, value: dto.value })
        })
        .collect()
}

/// Read the persisted `unsafeAllowAllEnv` flag and sync it into the plugin
/// engine so the setting takes effect even before the frontend boots.
fn apply_unsafe_env_setting(app_handle: &tauri::AppHandle) {
    use tauri_plugin_store::StoreExt;

    let enabled = match app_handle.store("settings.json") {
        Ok(store) => store
            .get("unsafeAllowAllEnv")
            .and_then(|value| value.as_bool())
            .unwrap_or(false),
        Err(error) => {
            log::warn!("Failed to read unsafeAllowAllEnv from settings: {}", error);
            false
        }
    };
    plugin_engine::host_api::set_allow_all_env(enabled);
}

/// Read persisted env groups from env.json and sync them into the plugin
/// engine so overrides take effect even before the frontend boots.
fn apply_env_overrides(app_handle: &tauri::AppHandle) {
    use tauri_plugin_store::StoreExt;

    let store = match app_handle.store("env.json") {
        Ok(store) => store,
        Err(error) => {
            log::warn!("Failed to open env.json: {}", error);
            return;
        }
    };

    let raw_groups = store.get("groups");
    let schema_version = store
        .get("envSchemaVersion")
        .and_then(|value| value.as_u64())
        .unwrap_or(0);

    let groups: Vec<EnvGroupDto> = match raw_groups.and_then(|v| serde_json::from_value(v).ok()) {
        Some(g) => g,
        None => return, // No groups yet — nothing to apply.
    };

    let dtos = if schema_version >= 2 {
        flatten_env_groups(&groups)
    } else {
        match store
            .get("activeGroupIds")
            .and_then(|v| serde_json::from_value::<Vec<String>>(v).ok())
        {
            Some(active_ids) => flatten_legacy_env_groups(&groups, &active_ids),
            None => flatten_env_groups(&groups),
        }
    };
    plugin_engine::host_api::set_env_overrides(map_env_overrides(dtos));
}

#[cfg(desktop)]
fn track_daily_active_if_needed(app_handle: &tauri::AppHandle) {
    use tauri_plugin_store::StoreExt;

    let today = today_utc_ymd();

    let store = match app_handle.store("settings.json") {
        Ok(store) => store,
        Err(error) => {
            log::warn!(
                "Failed to access settings store for daily analytics gate: {}",
                error
            );
            return;
        }
    };

    let last_tracked_day = store
        .get(DAILY_ACTIVE_TRACKED_DAY_KEY)
        .and_then(|value| value.as_str().map(|value| value.to_string()));

    if !should_track_daily_active(last_tracked_day.as_deref(), &today) {
        return;
    }

    if let Err(error) = app_handle.track_event(DAILY_ACTIVE_EVENT_NAME, None) {
        log::warn!("Failed to track daily analytics event: {}", error);
        return;
    }

    store.set(
        DAILY_ACTIVE_TRACKED_DAY_KEY,
        serde_json::Value::String(today),
    );
    if let Err(error) = store.save() {
        log::warn!("Failed to save daily analytics tracked day: {}", error);
    }
}

#[cfg(not(desktop))]
fn track_daily_active_if_needed(app_handle: &tauri::AppHandle) {
    let _ = app_handle.track_event(DAILY_ACTIVE_EVENT_NAME, None);
}

#[cfg(desktop)]
fn seconds_until_next_utc_day(now: time::OffsetDateTime) -> u64 {
    let now_time = now.time();
    let seconds_since_midnight = u64::from(now_time.hour()) * 60 * 60
        + u64::from(now_time.minute()) * 60
        + u64::from(now_time.second());
    let seconds_until_next_day = 86_400_u64.saturating_sub(seconds_since_midnight);
    if seconds_until_next_day == 0 {
        86_400
    } else {
        seconds_until_next_day
    }
}

#[cfg(desktop)]
fn spawn_daily_active_rollover_tracker(app_handle: tauri::AppHandle) {
    std::thread::spawn(move || {
        loop {
            let sleep_for = std::time::Duration::from_secs(seconds_until_next_utc_day(
                time::OffsetDateTime::now_utc(),
            ));
            std::thread::sleep(sleep_for);
            track_daily_active_if_needed(&app_handle);
        }
    });
}

#[cfg(desktop)]
fn managed_shortcut_slot() -> &'static Mutex<Option<String>> {
    static SLOT: OnceLock<Mutex<Option<String>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

/// Shared shortcut handler that toggles the panel when the shortcut is pressed.
#[cfg(desktop)]
fn handle_global_shortcut(
    app: &tauri::AppHandle,
    event: tauri_plugin_global_shortcut::ShortcutEvent,
) {
    if event.state == ShortcutState::Pressed {
        log::debug!("Global shortcut triggered");
        panel::toggle_panel(app);
    }
}

pub struct AppState {
    pub plugins: Vec<plugin_engine::manifest::LoadedPlugin>,
    pub app_data_dir: PathBuf,
    pub app_version: String,
    pub hub_dir: PathBuf,
    pub hub_registry: hub::registry::RegistryFile,
    pub plugins_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginMeta {
    pub id: String,
    pub name: String,
    pub icon_url: String,
    pub brand_color: Option<String>,
    pub lines: Vec<ManifestLineDto>,
    pub links: Vec<PluginLinkDto>,
    pub primary_candidates: Vec<String>,
    pub weekly_candidate: Option<String>,
    /// Human-readable source label (e.g. "Frankie's") from Hub metadata.
    /// None for unmanaged/local plugins.
    pub source_label: Option<String>,
    /// Installed version read from the Hub install metadata (`installed_version`).
    /// None for plugins installed outside the Hub (unmanaged / local).
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestLineDto {
    #[serde(rename = "type")]
    pub line_type: String,
    pub label: String,
    pub scope: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginLinkDto {
    pub label: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeBatchStarted {
    pub batch_id: String,
    pub plugin_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeResult {
    pub batch_id: String,
    pub output: plugin_engine::runtime::PluginOutput,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeBatchComplete {
    pub batch_id: String,
}

#[tauri::command]
fn init_panel(app_handle: tauri::AppHandle) {
    panel::init(&app_handle).expect("Failed to initialize panel");
}

#[tauri::command]
fn hide_panel(app_handle: tauri::AppHandle) {
    use tauri_nspanel::ManagerExt;
    if let Ok(panel) = app_handle.get_webview_panel("main") {
        panel.hide();
    }
}

/// Unsafe escape hatch toggled from Settings: when enabled, plugins may read
/// any environment variable instead of just the whitelisted ones.
#[tauri::command]
fn set_allow_all_env(enabled: bool) {
    plugin_engine::host_api::set_allow_all_env(enabled);
}

#[tauri::command]
fn load_log_level(app_handle: tauri::AppHandle) -> String {
    tray::get_stored_log_level_value(&app_handle).to_string()
}

#[tauri::command]
fn save_log_level(app_handle: tauri::AppHandle, level: String) -> Result<(), String> {
    let Some(parsed) = tray::parse_log_level(&level) else {
        log::warn!("Rejected invalid debug level from frontend: {}", level);
        return Err("Invalid debug level".to_string());
    };
    tray::save_log_level(&app_handle, parsed).map_err(|error| {
        log::error!("Failed to save debug level: {}", error);
        "Failed to save debug level".to_string()
    })
}

#[tauri::command]
fn set_env_overrides(overrides: Vec<EnvOverrideDto>) {
    plugin_engine::host_api::set_env_overrides(map_env_overrides(overrides));
}

#[tauri::command]
fn open_devtools(#[allow(unused)] app_handle: tauri::AppHandle) {
    #[cfg(debug_assertions)]
    {
        use tauri::Manager;
        if let Some(window) = app_handle.get_webview_window("main") {
            window.open_devtools();
        }
    }
}

#[tauri::command]
async fn start_probe_batch(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Mutex<AppState>>,
    batch_id: Option<String>,
    plugin_ids: Option<Vec<String>>,
) -> Result<ProbeBatchStarted, String> {
    let batch_id = batch_id
        .and_then(|id| {
            let trimmed = id.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        })
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let (plugins, app_data_dir, app_version) = {
        let locked = state.lock().map_err(|e| e.to_string())?;
        (
            locked.plugins.clone(),
            locked.app_data_dir.clone(),
            locked.app_version.clone(),
        )
    };

    let selected_plugins = match plugin_ids {
        Some(ids) => {
            let mut by_id: HashMap<String, plugin_engine::manifest::LoadedPlugin> = plugins
                .into_iter()
                .map(|plugin| (plugin.manifest.id.clone(), plugin))
                .collect();
            let mut seen = HashSet::new();
            ids.into_iter()
                .filter_map(|id| {
                    if !seen.insert(id.clone()) {
                        return None;
                    }
                    by_id.remove(&id)
                })
                .collect()
        }
        None => plugins,
    };

    let response_plugin_ids: Vec<String> = selected_plugins
        .iter()
        .map(|plugin| plugin.manifest.id.clone())
        .collect();

    log::info!(
        "probe batch {} starting: {:?}",
        batch_id,
        response_plugin_ids
    );

    if selected_plugins.is_empty() {
        let _ = app_handle.emit(
            "probe:batch-complete",
            ProbeBatchComplete {
                batch_id: batch_id.clone(),
            },
        );
        return Ok(ProbeBatchStarted {
            batch_id,
            plugin_ids: response_plugin_ids,
        });
    }

    let selected_count = selected_plugins.len();
    let worker_count = probe_worker_count(selected_count);
    if worker_count < selected_count {
        log::info!(
            "probe batch {} using {} workers for {} plugins",
            batch_id,
            worker_count,
            selected_count
        );
    }

    let remaining = Arc::new(AtomicUsize::new(selected_count));
    let probe_queue = Arc::new(Mutex::new(
        selected_plugins.into_iter().collect::<VecDeque<_>>(),
    ));

    for _ in 0..worker_count {
        let handle = app_handle.clone();
        let completion_handle = app_handle.clone();
        let bid = batch_id.clone();
        let completion_bid = batch_id.clone();
        let data_dir = app_data_dir.clone();
        let version = app_version.clone();
        let counter = Arc::clone(&remaining);
        let queue = Arc::clone(&probe_queue);

        tauri::async_runtime::spawn_blocking(move || {
            loop {
                let plugin = {
                    let mut queue = queue
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    queue.pop_front()
                };

                let Some(plugin) = plugin else {
                    break;
                };

                let plugin_id = plugin.manifest.id.clone();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    plugin_engine::runtime::run_probe(&plugin, &data_dir, &version)
                }));

                match result {
                    Ok(output) => {
                        let has_error = output.lines.iter().any(|line| {
                            matches!(line, plugin_engine::runtime::MetricLine::Badge { label, .. } if label == "Error")
                        });
                        if has_error {
                            log::warn!("probe {} completed with error", plugin_id);
                        } else {
                            log::info!(
                                "probe {} completed ok ({} lines)",
                                plugin_id,
                                output.lines.len()
                            );
                            local_http_api::cache_successful_output(&output);
                        }
                        let _ = handle.emit(
                            "probe:result",
                            ProbeResult {
                                batch_id: bid.clone(),
                                output,
                            },
                        );
                    }
                    Err(_) => {
                        log::error!("probe {} panicked", plugin_id);
                    }
                }

                if counter.fetch_sub(1, Ordering::SeqCst) == 1 {
                    log::info!("probe batch {} complete", completion_bid);
                    let _ = completion_handle.emit(
                        "probe:batch-complete",
                        ProbeBatchComplete {
                            batch_id: completion_bid.clone(),
                        },
                    );
                }
            }
        });
    }

    Ok(ProbeBatchStarted {
        batch_id,
        plugin_ids: response_plugin_ids,
    })
}

#[tauri::command]
fn get_log_path(app_handle: tauri::AppHandle) -> Result<String, String> {
    log_path::for_app(&app_handle).map(|path| path.to_string_lossy().to_string())
}

#[tauri::command]
fn copy_log_path(app_handle: tauri::AppHandle) -> Result<(), String> {
    log_path::copy_to_clipboard(&app_handle).map_err(|error| {
        log::error!("Failed to copy log path: {}", error);
        "Failed to copy log path".to_string()
    })
}

/// Update the global shortcut registration.
/// Pass `null` to disable the shortcut, or a shortcut string like "CommandOrControl+Shift+U".
#[cfg(desktop)]
#[tauri::command]
fn update_global_shortcut(
    app_handle: tauri::AppHandle,
    shortcut: Option<String>,
) -> Result<(), String> {
    let global_shortcut = app_handle.global_shortcut();
    let normalized_shortcut = shortcut.and_then(|value| {
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    });
    let mut managed_shortcut = managed_shortcut_slot()
        .lock()
        .map_err(|e| format!("failed to lock managed shortcut state: {}", e))?;

    if *managed_shortcut == normalized_shortcut {
        log::debug!("Global shortcut unchanged");
        return Ok(());
    }

    let previous_shortcut = managed_shortcut.clone();
    if let Some(existing) = previous_shortcut.as_deref() {
        match global_shortcut.unregister(existing) {
            Ok(()) => {
                // Keep in-memory state aligned with actual registration state.
                *managed_shortcut = None;
            }
            Err(e) => {
                log::warn!(
                    "Failed to unregister existing shortcut '{}': {}",
                    existing,
                    e
                );
            }
        }
    }

    if let Some(shortcut) = normalized_shortcut {
        log::info!("Registering global shortcut: {}", shortcut);
        global_shortcut
            .on_shortcut(shortcut.as_str(), |app, _shortcut, event| {
                handle_global_shortcut(app, event);
            })
            .map_err(|e| format!("Failed to register shortcut '{}': {}", shortcut, e))?;
        *managed_shortcut = Some(shortcut);
    } else {
        log::info!("Global shortcut disabled");
        *managed_shortcut = None;
    }

    Ok(())
}

#[tauri::command]
fn list_plugins(state: tauri::State<'_, Mutex<AppState>>) -> Vec<PluginMeta> {
    let (plugins, plugins_dir) = {
        let locked = state.lock().expect("plugin state poisoned");
        (locked.plugins.clone(), locked.plugins_dir.clone())
    };
    log::debug!("list_plugins: {} plugins", plugins.len());
    plugins_to_meta(&plugins, &plugins_dir)
}

/// Build the JS-facing PluginMeta list from the loaded Rust plugins.
/// Shared by `list_plugins` and `hub::reload_plugins_and_emit` so hot-reload
/// stays byte-identical to the initial probe.
/// Read install metadata from the plugin directory that was actually loaded.
/// This preserves source-scoped install dirs while avoiding unrelated or
/// transient metadata that may exist elsewhere under plugins/.
fn read_install_metadata_for_plugin(
    plugins_dir: &std::path::Path,
    plugin: &plugin_engine::manifest::LoadedPlugin,
) -> Option<hub::install::InstallMetadata> {
    let dir_name = plugin.plugin_dir.file_name()?.to_str()?;
    hub::install::read_install_metadata(plugins_dir, dir_name)
        .filter(|metadata| metadata.plugin_id == plugin.manifest.id)
}

pub fn plugins_to_meta(
    plugins: &[plugin_engine::manifest::LoadedPlugin],
    plugins_dir: &std::path::Path,
) -> Vec<PluginMeta> {
    plugins
        .iter()
        .map(|plugin| {
            let metadata = read_install_metadata_for_plugin(plugins_dir, plugin);
            // Extract primary candidates: progress lines with primary_order, sorted by order
            let mut candidates: Vec<_> = plugin
                .manifest
                .lines
                .iter()
                .filter(|line| line.line_type == "progress" && line.primary_order.is_some())
                .collect();
            candidates.sort_by_key(|line| line.primary_order.unwrap());
            let primary_candidates: Vec<String> =
                candidates.iter().map(|line| line.label.clone()).collect();

            // The weekly metric is the progress line declared `"period": "weekly"`.
            let weekly_candidate: Option<String> =
                plugin_engine::manifest::weekly_candidate(&plugin.manifest.lines)
                    .map(str::to_string);

            PluginMeta {
                id: plugin.manifest.id.clone(),
                name: plugin.manifest.name.clone(),
                icon_url: plugin.icon_data_url.clone(),
                brand_color: plugin.manifest.brand_color.clone(),
                lines: plugin
                    .manifest
                    .lines
                    .iter()
                    .map(|line| ManifestLineDto {
                        line_type: line.line_type.clone(),
                        label: line.label.clone(),
                        scope: line.scope.clone(),
                    })
                    .collect(),
                links: plugin
                    .manifest
                    .links
                    .iter()
                    .map(|link| PluginLinkDto {
                        label: link.label.clone(),
                        url: link.url.clone(),
                    })
                    .collect(),
                primary_candidates,
                weekly_candidate,
                source_label: metadata
                    .as_ref()
                    .filter(|m| !m.source_label.is_empty())
                    .map(|m| m.source_label.clone()),
                version: metadata.map(|m| m.installed_version),
            }
        })
        .collect()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    let _guard = runtime.enter();

    tauri::Builder::default()
        .plugin(tauri_plugin_aptabase::Builder::new("A-US-6435241436").build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_nspanel::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::LogDir { file_name: None }),
                ])
                .max_file_size(10_000_000) // 10 MB
                .level(log::LevelFilter::Trace) // Allow all levels; runtime filter via tray or Settings
                .level_for("hyper", log::LevelFilter::Warn)
                .level_for("reqwest", log::LevelFilter::Warn)
                .level_for("tao", log::LevelFilter::Info)
                .level_for("tauri_plugin_updater", log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            init_panel,
            hide_panel,
            open_devtools,
            set_allow_all_env,
            load_log_level,
            save_log_level,
            set_env_overrides,
            start_probe_batch,
            list_plugins,
            get_log_path,
            copy_log_path,
            update_global_shortcut,
            hub::hub_list_sources,
            hub::hub_add_source,
            hub::hub_update_source,
            hub::hub_remove_source,
            hub::hub_browse_source,
            hub::hub_install,
            hub::hub_switch_source,
            hub::hub_uninstall,
            hub::hub_refresh_source,
            hub::hub_reload_plugins,
            hub::hub_check_updates,
            hub::hub_list_local_plugins
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            #[cfg(target_os = "macos")]
            {
                app_nap::disable_app_nap();
                webkit_config::disable_webview_suspension(app.handle());
            }

            use tauri::Manager;

            let version = app.package_info().version.to_string();
            log::info!("OpenUsage v{} starting", version);

            // Load config early (lazy init via OnceLock, zero-cost after)
            let _proxy = config::get_resolved_proxy();

            apply_unsafe_env_setting(app.handle());
            apply_env_overrides(app.handle());

            track_daily_active_if_needed(app.handle());
            #[cfg(desktop)]
            spawn_daily_active_rollover_tracker(app.handle().clone());

            let app_data_dir = app.path().app_data_dir().expect("no app data dir");
            let resource_dir = app.path().resource_dir().expect("no resource dir");
            let app_data_dir_tail = app_data_dir
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("unknown");
            let redacted_app_data_dir =
                plugin_engine::host_api::redact_log_message(&app_data_dir.display().to_string());
            log::debug!(
                "app_data_dir: tail={}, path={}",
                app_data_dir_tail,
                redacted_app_data_dir
            );

            let (plugins_dir, plugins) = plugin_engine::initialize_plugins(&app_data_dir, &resource_dir);
            let known_plugin_ids: Vec<String> =
                plugins.iter().map(|p| p.manifest.id.clone()).collect();

            let hub_dir = hub::hub_dir(&app_data_dir);
            let mut hub_registry = hub::registry::read(&hub_dir).unwrap_or_else(|err| {
                log::warn!(
                    "hub registry load failed: {}, starting from default",
                    err
                );
                hub::registry::default_registry()
            });
            // Persist default to disk so sources.json exists before the first
            // Browse (which triggers auto-clone into cache/).
            if let Err(err) = hub::registry::write(&hub_dir, &hub_registry) {
                log::warn!("hub registry write failed (non-fatal): {}", err);
                // Reset to in-memory default so the UI still shows the upstream source.
                hub_registry = hub::registry::default_registry();
            }

            app.manage(Mutex::new(AppState {
                plugins,
                app_data_dir: app_data_dir.clone(),
                app_version: app.package_info().version.to_string(),
                hub_dir,
                hub_registry,
                plugins_dir,
            }));

            local_http_api::init(&app_data_dir, known_plugin_ids);
            local_http_api::start_server();

            tray::create(app.handle())?;

            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;

            // Register global shortcut from stored settings
            #[cfg(desktop)]
            {
                use tauri_plugin_store::StoreExt;

                if let Ok(store) = app.handle().store("settings.json") {
                    if let Some(shortcut_value) = store.get(GLOBAL_SHORTCUT_STORE_KEY) {
                        if let Some(shortcut) = shortcut_value.as_str() {
                            let shortcut = shortcut.trim();
                            if !shortcut.is_empty() {
                                let handle = app.handle().clone();
                                log::info!("Registering initial global shortcut: {}", shortcut);
                                if let Err(e) = handle.global_shortcut().on_shortcut(
                                    shortcut,
                                    |app, _shortcut, event| {
                                        handle_global_shortcut(app, event);
                                    },
                                ) {
                                    log::warn!("Failed to register initial global shortcut: {}", e);
                                } else if let Ok(mut managed_shortcut) =
                                    managed_shortcut_slot().lock()
                                {
                                    *managed_shortcut = Some(shortcut.to_string());
                                } else {
                                    log::warn!("Failed to store managed shortcut in memory");
                                }
                            }
                        }
                    }
                }
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_, event| match event {
            tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit => {
                local_http_api::flush_cache();
            }
            _ => {}
        });
}

#[cfg(test)]
mod tests {
    use super::{
        DAILY_ACTIVE_TRACKED_DAY_KEY, EnvGroupDto, EnvGroupOverrideDto, MAX_CONCURRENT_PROBES,
        flatten_env_groups, flatten_legacy_env_groups, plugins_to_meta, probe_worker_count,
        seconds_until_next_utc_day, should_track_daily_active,
    };
    use crate::hub::install::{INSTALL_METADATA_SCHEMA_VERSION, InstallMetadata};
    use crate::plugin_engine::manifest::{LoadedPlugin, PluginManifest};
    use std::path::{Path, PathBuf};
    use time::{Date, Month, PrimitiveDateTime, Time};

    #[test]
    fn should_track_when_no_previous_day() {
        assert!(should_track_daily_active(None, "2026-02-12"));
    }

    #[test]
    fn should_not_track_when_same_day() {
        assert!(!should_track_daily_active(Some("2026-02-12"), "2026-02-12"));
    }

    #[test]
    fn should_track_when_day_changes() {
        assert!(should_track_daily_active(Some("2026-02-11"), "2026-02-12"));
    }

    #[test]
    fn daily_active_key_is_not_version_scoped() {
        assert_eq!(DAILY_ACTIVE_TRACKED_DAY_KEY, "analytics.daily_active_day");
        assert!(!DAILY_ACTIVE_TRACKED_DAY_KEY.contains("0.6.2"));
        assert!(!DAILY_ACTIVE_TRACKED_DAY_KEY.contains("0.6.3"));
    }

    #[test]
    fn rollover_sleep_waits_for_next_utc_day_boundary() {
        let now = PrimitiveDateTime::new(
            Date::from_calendar_date(2026, Month::February, 12).unwrap(),
            Time::from_hms(23, 59, 50).unwrap(),
        )
        .assume_utc();

        assert_eq!(seconds_until_next_utc_day(now), 10);
    }

    #[test]
    fn probe_worker_count_is_bounded() {
        assert_eq!(probe_worker_count(0), 0);
        assert_eq!(probe_worker_count(1), 1);
        assert_eq!(
            probe_worker_count(MAX_CONCURRENT_PROBES),
            MAX_CONCURRENT_PROBES
        );
        assert_eq!(
            probe_worker_count(MAX_CONCURRENT_PROBES + 1),
            MAX_CONCURRENT_PROBES
        );
    }

    #[test]
    fn plugins_to_meta_uses_loaded_plugin_dir_for_install_metadata() {
        let plugins_dir = tempdir("plugins-to-meta-metadata-source");
        let loaded_dir = plugins_dir.join("claude__source-a");
        std::fs::create_dir_all(&loaded_dir).unwrap();
        crate::hub::install::write_install_metadata(
            &plugins_dir,
            "orphan-metadata",
            &install_metadata("claude", "Orphan Source", "9.9.9"),
        )
        .unwrap();

        let plugin = loaded_plugin("claude", loaded_dir);
        let meta = plugins_to_meta(&[plugin], &plugins_dir);

        assert_eq!(meta.len(), 1);
        assert_eq!(meta[0].source_label, None);
        assert_eq!(meta[0].version, None);

        crate::hub::install::write_install_metadata(
            &plugins_dir,
            "claude__source-a",
            &install_metadata("claude", "Loaded Source", "1.2.3"),
        )
        .unwrap();

        let plugin = loaded_plugin("claude", plugins_dir.join("claude__source-a"));
        let meta = plugins_to_meta(&[plugin], &plugins_dir);

        assert_eq!(meta[0].source_label.as_deref(), Some("Loaded Source"));
        assert_eq!(meta[0].version.as_deref(), Some("1.2.3"));
    }

    #[test]
    fn flatten_env_groups_uses_enabled_as_source_of_truth() {
        let groups = vec![
            EnvGroupDto {
                id: "enabled".to_string(),
                name: "Enabled".to_string(),
                enabled: true,
                overrides: vec![EnvGroupOverrideDto {
                    name: "OPENUSAGE_ENABLED".to_string(),
                    value: "yes".to_string(),
                }],
            },
            EnvGroupDto {
                id: "disabled".to_string(),
                name: "Disabled".to_string(),
                enabled: false,
                overrides: vec![EnvGroupOverrideDto {
                    name: "OPENUSAGE_DISABLED".to_string(),
                    value: "no".to_string(),
                }],
            },
        ];

        let flattened = flatten_env_groups(&groups);

        assert_eq!(flattened.len(), 1);
        assert_eq!(flattened[0].name, "OPENUSAGE_ENABLED");
    }

    #[test]
    fn flatten_legacy_env_groups_uses_active_ids() {
        let groups = vec![EnvGroupDto {
            id: "legacy-active".to_string(),
            name: "Legacy Active".to_string(),
            enabled: false,
            overrides: vec![EnvGroupOverrideDto {
                name: "OPENUSAGE_LEGACY".to_string(),
                value: "yes".to_string(),
            }],
        }];

        let flattened = flatten_legacy_env_groups(&groups, &["legacy-active".to_string()]);

        assert_eq!(flattened.len(), 1);
        assert_eq!(flattened[0].name, "OPENUSAGE_LEGACY");
    }

    fn loaded_plugin(id: &str, plugin_dir: PathBuf) -> LoadedPlugin {
        LoadedPlugin {
            manifest: PluginManifest {
                schema_version: 1,
                id: id.to_string(),
                name: id.to_string(),
                version: "1.0.0".to_string(),
                entry: "plugin.js".to_string(),
                icon: "icon.svg".to_string(),
                brand_color: None,
                lines: Vec::new(),
                links: Vec::new(),
            },
            plugin_dir,
            entry_script: String::new(),
            icon_data_url: String::new(),
        }
    }

    fn install_metadata(plugin_id: &str, source_label: &str, version: &str) -> InstallMetadata {
        InstallMetadata {
            schema_version: INSTALL_METADATA_SCHEMA_VERSION,
            source_id: "source".to_string(),
            source_url: "https://example.com/source.git".to_string(),
            source_label: source_label.to_string(),
            source_kind: None,
            source_ref: None,
            source_commit_sha: None,
            plugin_id: plugin_id.to_string(),
            installed_version: version.to_string(),
            package_hash: "sha256:test".to_string(),
            installed_at: 1,
        }
    }

    fn tempdir(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "openusage-lib-{}-{}-{}",
            label,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        if path.exists() {
            remove_dir_all_best_effort(&path);
        }
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn remove_dir_all_best_effort(path: &Path) {
        if path.exists() {
            let _ = std::fs::remove_dir_all(path);
        }
    }
}
