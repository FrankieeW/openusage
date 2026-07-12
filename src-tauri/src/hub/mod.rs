mod browse_commands;
pub mod cache_index;
mod discovery;
mod error;
pub mod git_ops;
pub mod install;
mod package_state;
mod plugin_commands;
pub mod registry;
mod runtime_commands;
pub mod source;
mod source_commands;
mod source_support;
mod types;

use std::path::Path;
use std::sync::Mutex;

use tauri::{AppHandle, Emitter, State};

use discovery::{
    discover_cache_plugins_with_index, parse_manifest_updated_at, read_icon_data_url,
    validate_source_health,
};
use runtime_commands::{lock_state, reload_plugins_and_emit, report_orphans};
use source_support::{resync_local_cache, source_snapshot};

pub use error::HubError;
pub use package_state::build_installed_lookup;
pub use registry::Source;
pub use source::SourceKind;
pub use source_support::{
    cache_dir_for, derive_label_from_url, hub_dir, normalize_plugin_filter, now_millis,
    plugin_filter_lookup, sanitize_label,
};
pub use types::{
    HubBrowseView, InstalledLookup, InstalledLookupEntry, PackageStatus, PluginInfo, SkippedPlugin,
    SourceSnapshot, UpdateInfo,
};

#[cfg(test)]
pub type HubErrorCode = error::HubErrorCode;

#[cfg(test)]
/// Walks `cache_dir/plugins/<id>/plugin.json` and returns parsed plugin details
/// plus entries skipped during discovery.
pub fn discover_cache_plugins(
    cache_dir: &Path,
    source_id: &str,
    plugins_dir: &Path,
    installed: &InstalledLookup,
    plugin_filter: Option<&[String]>,
) -> (Vec<PluginInfo>, Vec<SkippedPlugin>) {
    discovery::discover_cache_plugins(cache_dir, source_id, plugins_dir, installed, plugin_filter)
}

#[tauri::command]
pub async fn hub_list_sources(
    state: State<'_, Mutex<crate::AppState>>,
) -> Result<Vec<Source>, HubError> {
    source_commands::hub_list_sources_impl(state).await
}

#[tauri::command]
pub async fn hub_add_source(
    _app: AppHandle,
    state: State<'_, Mutex<crate::AppState>>,
    url: String,
    label: Option<String>,
    branch: Option<String>,
    plugin_filter: Option<Vec<String>>,
) -> Result<Source, HubError> {
    source_commands::hub_add_source_impl(_app, state, url, label, branch, plugin_filter).await
}

/// Update mutable fields on an existing source. `None` for an option leaves the
/// field unchanged; `Some(value)` replaces it. `plugin_filter` is normalized
/// the same way as `hub_add_source`. After mutating, the source's cache is
/// cleared so the next browse re-fetches against the (possibly) new branch or
/// filter set.
#[tauri::command]
pub async fn hub_update_source(
    state: State<'_, Mutex<crate::AppState>>,
    source_id: String,
    label: Option<String>,
    branch: Option<String>,
    plugin_filter: Option<Vec<String>>,
) -> Result<Source, HubError> {
    source_commands::hub_update_source_impl(state, source_id, label, branch, plugin_filter).await
}

#[tauri::command]
pub async fn hub_remove_source(
    state: State<'_, Mutex<crate::AppState>>,
    source_id: String,
) -> Result<(), HubError> {
    source_commands::hub_remove_source_impl(state, source_id).await
}

#[tauri::command]
pub async fn hub_browse_source(
    state: State<'_, Mutex<crate::AppState>>,
    source_id: String,
) -> Result<HubBrowseView, HubError> {
    browse_commands::hub_browse_source_impl(state, source_id).await
}

#[tauri::command]
pub async fn hub_install(
    app: AppHandle,
    state: State<'_, Mutex<crate::AppState>>,
    source_id: String,
    plugin_id: String,
) -> Result<(), HubError> {
    plugin_commands::hub_install_impl(app, state, source_id, plugin_id).await
}

#[tauri::command]
pub async fn hub_switch_source(
    app: AppHandle,
    state: State<'_, Mutex<crate::AppState>>,
    source_id: String,
    plugin_id: String,
) -> Result<(), HubError> {
    plugin_commands::hub_switch_source_impl(app, state, source_id, plugin_id).await
}

#[tauri::command]
pub async fn hub_uninstall(
    app: AppHandle,
    state: State<'_, Mutex<crate::AppState>>,
    plugin_id: String,
    source_id: Option<String>,
) -> Result<(), HubError> {
    plugin_commands::hub_uninstall_impl(app, state, plugin_id, source_id).await
}

#[tauri::command]
pub async fn hub_refresh_source(
    state: State<'_, Mutex<crate::AppState>>,
    source_id: String,
) -> Result<HubBrowseView, HubError> {
    browse_commands::hub_refresh_source_impl(state, source_id).await
}

/// Public: re-walk the plugins directory and re-emit `plugins-changed`. Used by
/// the Settings page's manual "Reload Plugins" button. Same effect as the
/// auto-broadcast that fires after install/uninstall, but on demand.
#[tauri::command]
pub fn hub_reload_plugins(
    app: AppHandle,
    state: State<'_, Mutex<crate::AppState>>,
) -> Result<usize, HubError> {
    runtime_commands::hub_reload_plugins_impl(app, state)
}

#[tauri::command]
pub async fn hub_check_updates(
    app: AppHandle,
    state: State<'_, Mutex<crate::AppState>>,
) -> Result<Vec<UpdateInfo>, HubError> {
    browse_commands::hub_check_updates_impl(app, state).await
}

/// Return plugins present in the install directory that have no Hub metadata.
/// These are pre-existing plugins from an older OpenUsage version or manually
/// copied directories — the Hub shows them under a virtual "Local" source.
#[tauri::command]
pub async fn hub_list_local_plugins(
    state: State<'_, Mutex<crate::AppState>>,
) -> Result<Vec<PluginInfo>, HubError> {
    runtime_commands::hub_list_local_plugins_impl(state).await
}

#[cfg(test)]
mod tests;
