use super::*;

pub(super) async fn hub_install_impl(
    app: AppHandle,
    state: State<'_, Mutex<crate::AppState>>,
    source_id: String,
    plugin_id: String,
) -> Result<(), HubError> {
    let (hub_dir, plugins_dir, source) = {
        let s = lock_state(&state)?;
        let source = s
            .hub_registry
            .sources
            .iter()
            .find(|src| src.id == source_id)
            .cloned()
            .ok_or_else(|| HubError::not_found(format!("source {}", source_id)))?;
        (s.hub_dir.clone(), s.plugins_dir.clone(), source)
    };

    let safe_label = sanitize_label(&source.label);
    let install_dir_name = if safe_label.is_empty() || safe_label == "local" {
        plugin_id.clone()
    } else {
        format!("{}__{}", plugin_id, safe_label)
    };
    let install_dir_name =
        find_install_dir(&plugins_dir, &plugin_id, &source_id).unwrap_or(install_dir_name);

    install::check_conflict(&plugins_dir, &install_dir_name, &source_id)?;

    let source_plugin_dir = cache_dir_for(&hub_dir, &source_id)
        .join("plugins")
        .join(&plugin_id);
    if !source_plugin_dir.is_dir() {
        return Err(HubError::not_found(format!(
            "plugin {} in source {}",
            plugin_id, source_id
        )));
    }
    let manifest_text = std::fs::read_to_string(source_plugin_dir.join("plugin.json"))
        .map_err(|e| HubError::io(e.to_string()))?;
    let manifest: serde_json::Value = serde_json::from_str(&manifest_text)
        .map_err(|e| HubError::manifest_parse(e.to_string()))?;
    let version = manifest
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("0.0.0")
        .to_string();
    let package_hash = install::package_hash(&source_plugin_dir)?;
    let source_commit_sha = if matches!(source.kind, SourceKind::Github | SourceKind::GenericGit) {
        match git_ops::head_commit(&cache_dir_for(&hub_dir, &source_id)).await {
            Ok(sha) => Some(sha),
            Err(err) => {
                log::warn!(
                    "hub_install: cannot read source commit for {}: {}",
                    source_id,
                    err
                );
                None
            }
        }
    } else {
        None
    };

    let metadata = install::InstallMetadata {
        schema_version: install::INSTALL_METADATA_SCHEMA_VERSION,
        source_id: source.id.clone(),
        source_url: source.url.clone(),
        source_label: source.label.clone(),
        source_kind: Some(source.kind),
        source_ref: source.branch.clone(),
        source_commit_sha,
        plugin_id: plugin_id.clone(),
        installed_version: version,
        package_hash,
        installed_at: now_millis(),
    };
    install::switch_plugin_install_dir_with_metadata(
        &source_plugin_dir,
        &plugins_dir,
        &install_dir_name,
        &install_dir_name,
        &metadata,
    )?;

    reload_plugins_and_emit(&app, &state)?;
    report_orphans(&app, &state);
    Ok(())
}

pub(super) async fn hub_switch_source_impl(
    app: AppHandle,
    state: State<'_, Mutex<crate::AppState>>,
    source_id: String,
    plugin_id: String,
) -> Result<(), HubError> {
    let (hub_dir, plugins_dir, source) = {
        let s = lock_state(&state)?;
        let source = s
            .hub_registry
            .sources
            .iter()
            .find(|src| src.id == source_id)
            .cloned()
            .ok_or_else(|| HubError::not_found(format!("source {}", source_id)))?;
        (s.hub_dir.clone(), s.plugins_dir.clone(), source)
    };

    let old_install_dir_name = find_install_dir(&plugins_dir, &plugin_id, "")
        .ok_or_else(|| HubError::not_found(format!("installed plugin {}", plugin_id)))?;
    let safe_label = sanitize_label(&source.label);
    let new_install_dir_name = if safe_label.is_empty() || safe_label == "local" {
        plugin_id.clone()
    } else {
        format!("{}__{}", plugin_id, safe_label)
    };

    let source_plugin_dir = cache_dir_for(&hub_dir, &source_id)
        .join("plugins")
        .join(&plugin_id);
    if !source_plugin_dir.is_dir() {
        return Err(HubError::not_found(format!(
            "plugin {} in source {}",
            plugin_id, source_id
        )));
    }
    let manifest_text = std::fs::read_to_string(source_plugin_dir.join("plugin.json"))
        .map_err(|e| HubError::io(e.to_string()))?;
    let manifest: serde_json::Value = serde_json::from_str(&manifest_text)
        .map_err(|e| HubError::manifest_parse(e.to_string()))?;
    let version = manifest
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("0.0.0")
        .to_string();
    let package_hash = install::package_hash(&source_plugin_dir)?;
    let source_commit_sha = if matches!(source.kind, SourceKind::Github | SourceKind::GenericGit) {
        match git_ops::head_commit(&cache_dir_for(&hub_dir, &source_id)).await {
            Ok(sha) => Some(sha),
            Err(err) => {
                log::warn!(
                    "hub_switch_source: cannot read source commit for {}: {}",
                    source_id,
                    err
                );
                None
            }
        }
    } else {
        None
    };

    let metadata = install::InstallMetadata {
        schema_version: install::INSTALL_METADATA_SCHEMA_VERSION,
        source_id: source.id.clone(),
        source_url: source.url.clone(),
        source_label: source.label.clone(),
        source_kind: Some(source.kind),
        source_ref: source.branch.clone(),
        source_commit_sha,
        plugin_id: plugin_id.clone(),
        installed_version: version,
        package_hash,
        installed_at: now_millis(),
    };
    install::switch_plugin_install_dir_with_metadata(
        &source_plugin_dir,
        &plugins_dir,
        &old_install_dir_name,
        &new_install_dir_name,
        &metadata,
    )?;

    reload_plugins_and_emit(&app, &state)?;
    report_orphans(&app, &state);
    Ok(())
}

pub(super) async fn hub_uninstall_impl(
    app: AppHandle,
    state: State<'_, Mutex<crate::AppState>>,
    plugin_id: String,
    source_id: Option<String>,
) -> Result<(), HubError> {
    log::info!(
        "hub_uninstall ENTER: plugin_id={} source_id={:?}",
        plugin_id,
        source_id
    );
    {
        let s = lock_state(&state)?;
        let plugins_dir = s.plugins_dir.clone();
        let dir_name = {
            // Try metadata-based lookup first (handles per-source dir naming)
            let found =
                find_install_dir(&plugins_dir, &plugin_id, source_id.as_deref().unwrap_or(""));
            if let Some(d) = found {
                log::info!("hub_uninstall: found dir {} for {}", d, plugin_id);
                d
            } else if plugins_dir.join(&plugin_id).is_dir() {
                // Plain directory (local / unmanaged / pre-Hub install)
                log::info!("hub_uninstall: using plain dir {}", plugin_id);
                plugin_id.clone()
            } else {
                log::warn!(
                    "hub_uninstall: no dir found for {}, removing anyway",
                    plugin_id
                );
                plugin_id.clone()
            }
        };
        install::remove_installed_plugin(&plugins_dir, &dir_name)?;
    }
    reload_plugins_and_emit(&app, &state)?;
    Ok(())
}

/// Walk plugins/ and return the directory name whose .openusage-install.json
/// matches the given plugin_id + source_id.
fn find_install_dir(plugins_dir: &Path, plugin_id: &str, source_id: &str) -> Option<String> {
    let entries = match std::fs::read_dir(plugins_dir) {
        Ok(e) => e,
        Err(e) => {
            log::warn!(
                "find_install_dir: cannot read {}: {}",
                plugins_dir.display(),
                e
            );
            return None;
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let dir_name = entry.file_name().to_string_lossy().to_string();
        if install::is_internal_dir_name(&dir_name) {
            continue;
        }
        match install::read_install_metadata(plugins_dir, &dir_name) {
            Some(meta) => {
                log::debug!(
                    "find_install_dir: dir={} meta.plugin_id={} meta.source_id={}",
                    dir_name,
                    meta.plugin_id,
                    meta.source_id
                );
                let source_matches = source_id.is_empty() || meta.source_id == source_id;
                if meta.plugin_id == plugin_id && source_matches {
                    return Some(dir_name);
                }
            }
            None => {
                log::debug!("find_install_dir: dir={} has no metadata", dir_name);
            }
        }
    }
    log::warn!(
        "find_install_dir: no match for plugin_id={} source_id={}",
        plugin_id,
        source_id
    );
    None
}
