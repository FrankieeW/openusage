use super::*;

pub(super) async fn hub_list_sources_impl(
    state: State<'_, Mutex<crate::AppState>>,
) -> Result<Vec<Source>, HubError> {
    let s = lock_state(&state)?;
    Ok(s.hub_registry.sources.clone())
}

pub(super) async fn hub_add_source_impl(
    _app: AppHandle,
    state: State<'_, Mutex<crate::AppState>>,
    url: String,
    label: Option<String>,
    branch: Option<String>,
    plugin_filter: Option<Vec<String>>,
) -> Result<Source, HubError> {
    let canonical = source::canonicalize(&url).map_err(|_| HubError::invalid_url())?;
    let kind = canonical.kind;
    let url = canonical.url.clone();
    let branch = branch.or_else(|| canonical.branch.clone());

    let id = format!("src-{}", uuid::Uuid::new_v4().simple());
    let label = label.unwrap_or_else(|| derive_label_from_url(&canonical.url));
    let plugin_filter = normalize_plugin_filter(plugin_filter);
    let now = now_millis();
    let mut new_source = Source {
        id: id.clone(),
        label,
        url: url.clone(),
        kind,
        branch: branch.clone(),
        plugin_filter,
        added_at: now,
        last_refreshed_at: None,
        auto_check: false,
    };

    // Clone first (network/disk I/O), then commit to registry only if clone
    // succeeds — avoids half-state if clone fails.
    let (cache_path, plugins_dir) = {
        let s = lock_state(&state)?;
        (cache_dir_for(&s.hub_dir, &id), s.plugins_dir.clone())
    };
    match kind {
        SourceKind::Github | SourceKind::GenericGit => {
            git_ops::clone(&url, &cache_path, branch.as_deref()).await?;
        }
        SourceKind::LocalPath => {
            let src = canonical
                .local_path
                .as_ref()
                .ok_or_else(HubError::invalid_url)?;
            install::copy_dir_to(src, &cache_path).map_err(HubError::from)?;
        }
    }
    if let Err(err) = validate_source_health(
        &cache_path,
        &id,
        &plugins_dir,
        new_source.plugin_filter.as_deref(),
    ) {
        let _ = std::fs::remove_dir_all(&cache_path);
        return Err(err);
    }
    new_source.last_refreshed_at = Some(now_millis());

    let mut s = lock_state(&state)?;
    s.hub_registry.sources.push(new_source.clone());
    registry::write(&s.hub_dir, &s.hub_registry)?;
    Ok(new_source)
}

/// Update mutable fields on an existing source. `None` for an option leaves the
/// field unchanged; `Some(value)` replaces it. `plugin_filter` is normalized
/// the same way as `hub_add_source`. After mutating, the source's cache is
/// cleared so the next browse re-fetches against the (possibly) new branch or
/// filter set.
pub(super) async fn hub_update_source_impl(
    state: State<'_, Mutex<crate::AppState>>,
    source_id: String,
    label: Option<String>,
    branch: Option<String>,
    plugin_filter: Option<Vec<String>>,
) -> Result<Source, HubError> {
    let hub_dir = {
        let mut s = lock_state(&state)?;
        let target = s
            .hub_registry
            .sources
            .iter_mut()
            .find(|src| src.id == source_id)
            .ok_or_else(|| HubError::not_found(format!("source {}", source_id)))?;
        if let Some(new_label) = label {
            target.label = new_label;
        }
        if let Some(new_branch) = branch {
            target.branch = if new_branch.trim().is_empty() {
                None
            } else {
                Some(new_branch)
            };
        }
        if let Some(new_filter) = plugin_filter {
            target.plugin_filter = normalize_plugin_filter(Some(new_filter));
        }
        // Drop the cached `last_refreshed_at` so the UI knows the source has
        // pending re-fetch.
        target.last_refreshed_at = None;
        registry::write(&s.hub_dir, &s.hub_registry)?;
        s.hub_dir.clone()
    };

    // Clear the cache so the next browse re-fetches against the (possibly) new
    // branch or filter set.
    let cache_path = cache_dir_for(&hub_dir, &source_id);
    if cache_path.exists() {
        let _ = std::fs::remove_dir_all(&cache_path);
    }

    let s = lock_state(&state)?;
    s.hub_registry
        .sources
        .iter()
        .find(|src| src.id == source_id)
        .cloned()
        .ok_or_else(|| HubError::not_found(format!("source {}", source_id)))
}

pub(super) async fn hub_remove_source_impl(
    state: State<'_, Mutex<crate::AppState>>,
    source_id: String,
) -> Result<(), HubError> {
    let mut s = lock_state(&state)?;
    let cache_path = cache_dir_for(&s.hub_dir, &source_id);
    if cache_path.exists() {
        let _ = std::fs::remove_dir_all(&cache_path);
    }
    // Reclassify installed plugins from this source as local (unmanaged)
    let plugins_dir = s.plugins_dir.clone();
    if plugins_dir.is_dir()
        && let Ok(entries) = std::fs::read_dir(&plugins_dir)
    {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let dir_name = entry.file_name().to_string_lossy().to_string();
            if install::is_internal_dir_name(&dir_name) {
                continue;
            }
            if let Some(mut meta) = install::read_install_metadata(&plugins_dir, &dir_name)
                && meta.source_id == source_id
            {
                meta.source_id = String::new();
                let _ = install::write_install_metadata(&plugins_dir, &dir_name, &meta);
            }
        }
    }
    s.hub_registry.sources.retain(|src| src.id != source_id);
    registry::write(&s.hub_dir, &s.hub_registry)?;
    Ok(())
}
