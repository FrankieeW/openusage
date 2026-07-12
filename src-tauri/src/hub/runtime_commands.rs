use super::*;

pub(super) fn lock_state<'a>(
    state: &'a State<'_, Mutex<crate::AppState>>,
) -> Result<std::sync::MutexGuard<'a, crate::AppState>, HubError> {
    state
        .lock()
        .map_err(|e| HubError::io(format!("state poisoned: {}", e)))
}

/// Reload the installed plugins list and emit `plugins-changed` so the JS side
/// can refresh. Errors are logged; not propagated (best-effort).
pub(super) fn reload_plugins_and_emit(
    app: &AppHandle,
    state: &State<'_, Mutex<crate::AppState>>,
) -> Result<(), HubError> {
    let plugins_dir = {
        let s = lock_state(state)?;
        s.plugins_dir.clone()
    };
    let fresh = crate::plugin_engine::reload_from_install_dir(&plugins_dir);
    let meta = crate::plugins_to_meta(&fresh, &plugins_dir);
    log::info!(
        "reload_plugins_and_emit: {} plugins loaded, emitting plugins-changed",
        meta.len()
    );
    {
        let mut s = lock_state(state)?;
        s.plugins = fresh;
    }
    app.emit("plugins-changed", meta)
        .map_err(|e| HubError::io(format!("emit plugins-changed: {}", e)))?;
    Ok(())
}

/// Re-classify all installed plugins against the current registry/source map
/// and report orphan-source plugins back to JS via `hub-orphans-detected`.
pub(super) fn report_orphans(app: &AppHandle, state: &State<'_, Mutex<crate::AppState>>) {
    let (hub_dir, plugins_dir, registry) = match lock_state(state) {
        Ok(s) => (
            s.hub_dir.clone(),
            s.plugins_dir.clone(),
            s.hub_registry.clone(),
        ),
        Err(_) => return,
    };
    let report = install::startup_sweep(&hub_dir, &plugins_dir, &registry);
    if !report.orphan_source_plugins.is_empty() || !report.unmanaged_plugins.is_empty() {
        let _ = app.emit(
            "hub-orphans-detected",
            serde_json::json!({
                "orphanSourcePlugins": report.orphan_source_plugins,
                "unmanagedPlugins": report.unmanaged_plugins,
            }),
        );
    }
}

pub(super) fn hub_reload_plugins_impl(
    app: AppHandle,
    state: State<'_, Mutex<crate::AppState>>,
) -> Result<usize, HubError> {
    reload_plugins_and_emit(&app, &state)?;
    let count = {
        let s = lock_state(&state)?;
        s.plugins.len()
    };
    Ok(count)
}

pub(super) async fn hub_list_local_plugins_impl(
    state: State<'_, Mutex<crate::AppState>>,
) -> Result<Vec<PluginInfo>, HubError> {
    let plugins_dir = {
        let s = lock_state(&state)?;
        s.plugins_dir.clone()
    };
    let mut locals = Vec::new();
    if !plugins_dir.is_dir() {
        return Ok(locals);
    }
    let entries = std::fs::read_dir(&plugins_dir).map_err(|e| HubError::io(e.to_string()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let id = entry.file_name().to_string_lossy().to_string();
        if install::is_internal_dir_name(&id) {
            continue;
        }
        // Skip anything that already has Hub metadata — those belong to a Hub source
        if install::read_install_metadata(&plugins_dir, &id).is_some() {
            continue;
        }
        let manifest_path = path.join("plugin.json");
        let Ok(text) = std::fs::read_to_string(&manifest_path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        let name = value
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(&id)
            .to_string();
        let version = value
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("0.0.0")
            .to_string();
        let updated_at = parse_manifest_updated_at(&value);
        let brand_color = value
            .get("brandColor")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let icon_filename = value
            .get("icon")
            .and_then(|v| v.as_str())
            .unwrap_or("icon.svg");
        let icon_data_url = read_icon_data_url(&path, icon_filename);

        locals.push(PluginInfo {
            id,
            name,
            brand_color,
            icon_data_url,
            source_id: String::new(),
            installed: true,
            installed_source_id: None,
            unmanaged: true,
            installed_version: Some(version.clone()),
            available_version: version,
            updated_at,
            package_hash: install::package_hash(&path)?,
            package_status: PackageStatus::UnmanagedInstalled,
            update_available: false,
        });
    }
    locals.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(locals)
}
