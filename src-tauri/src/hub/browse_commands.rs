use super::*;

pub(super) async fn hub_browse_source_impl(
    state: State<'_, Mutex<crate::AppState>>,
    source_id: String,
) -> Result<HubBrowseView, HubError> {
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

    let cache_path = cache_dir_for(&hub_dir, &source_id);
    if !cache_path.exists() {
        match source.kind {
            SourceKind::Github | SourceKind::GenericGit => {
                git_ops::clone(&source.url, &cache_path, source.branch.as_deref()).await?;
            }
            SourceKind::LocalPath => {
                return Err(HubError::not_found(
                    "local source path not found; re-add the source",
                ));
            }
        }
    }
    let commit_sha = match source.kind {
        SourceKind::Github | SourceKind::GenericGit => {
            match git_ops::head_commit(&cache_path).await {
                Ok(sha) => Some(sha),
                Err(err) => {
                    log::warn!(
                        "hub_browse_source: cannot read commit for {}: {}",
                        source_id,
                        err
                    );
                    None
                }
            }
        }
        SourceKind::LocalPath => None,
    };
    let installed = build_installed_lookup(&plugins_dir);
    let (available, skipped) = discover_cache_plugins_with_index(
        &cache_path,
        &source_id,
        &plugins_dir,
        &installed,
        source.plugin_filter.as_deref(),
        commit_sha.clone(),
    );
    let snapshot = source_snapshot(
        &source,
        commit_sha,
        available.len(),
        skipped.len(),
        now_millis(),
    );
    Ok(HubBrowseView {
        source,
        available,
        skipped,
        snapshot,
    })
}

pub(super) async fn hub_refresh_source_impl(
    state: State<'_, Mutex<crate::AppState>>,
    source_id: String,
) -> Result<HubBrowseView, HubError> {
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

    let cache_path = cache_dir_for(&hub_dir, &source_id);
    if !cache_path.exists() {
        return Err(HubError::not_found(format!(
            "no cache for source {}; re-add the source",
            source_id
        )));
    }

    match source.kind {
        SourceKind::LocalPath => {
            resync_local_cache(Path::new(&source.url), &cache_path)?;
        }
        SourceKind::Github | SourceKind::GenericGit => {
            git_ops::fetch_and_reset(&cache_path, source.branch.as_deref()).await?;
        }
    }
    let refreshed_at = now_millis();
    let source = {
        let mut s = lock_state(&state)?;
        let target = s
            .hub_registry
            .sources
            .iter_mut()
            .find(|src| src.id == source_id)
            .ok_or_else(|| HubError::not_found(format!("source {}", source_id)))?;
        target.last_refreshed_at = Some(refreshed_at);
        let updated = target.clone();
        registry::write(&s.hub_dir, &s.hub_registry)?;
        updated
    };

    let commit_sha = match source.kind {
        SourceKind::Github | SourceKind::GenericGit => {
            match git_ops::head_commit(&cache_path).await {
                Ok(sha) => Some(sha),
                Err(err) => {
                    log::warn!(
                        "hub_refresh_source: cannot read commit for {}: {}",
                        source_id,
                        err
                    );
                    None
                }
            }
        }
        SourceKind::LocalPath => None,
    };
    let installed = build_installed_lookup(&plugins_dir);
    let (available, skipped) = discover_cache_plugins_with_index(
        &cache_path,
        &source_id,
        &plugins_dir,
        &installed,
        source.plugin_filter.as_deref(),
        commit_sha.clone(),
    );
    let snapshot = source_snapshot(
        &source,
        commit_sha,
        available.len(),
        skipped.len(),
        refreshed_at,
    );
    Ok(HubBrowseView {
        source,
        available,
        skipped,
        snapshot,
    })
}

pub(super) async fn hub_check_updates_impl(
    app: AppHandle,
    state: State<'_, Mutex<crate::AppState>>,
) -> Result<Vec<UpdateInfo>, HubError> {
    let source_ids: Vec<(String, SourceKind, Option<String>)> = {
        let s = lock_state(&state)?;
        s.hub_registry
            .sources
            .iter()
            .map(|src| (src.id.clone(), src.kind, src.branch.clone()))
            .collect()
    };
    let (hub_dir, plugins_dir, sources) = {
        let s = lock_state(&state)?;
        (
            s.hub_dir.clone(),
            s.plugins_dir.clone(),
            s.hub_registry.sources.clone(),
        )
    };

    let mut updates = Vec::new();
    for (id, kind, branch) in &source_ids {
        let cache_path = cache_dir_for(&hub_dir, id);
        if !cache_path.exists() {
            continue;
        }
        if matches!(kind, SourceKind::Github | SourceKind::GenericGit)
            && let Err(err) = git_ops::fetch_and_reset(&cache_path, branch.as_deref()).await
        {
            log::warn!("hub_check_updates: refresh {} failed: {}", id, err);
            continue;
        }
        let commit_sha = if matches!(kind, SourceKind::Github | SourceKind::GenericGit) {
            match git_ops::head_commit(&cache_path).await {
                Ok(sha) => Some(sha),
                Err(err) => {
                    log::warn!("hub_check_updates: cannot read commit for {}: {}", id, err);
                    None
                }
            }
        } else {
            None
        };
        let installed = build_installed_lookup(&plugins_dir);
        let (available, _) = discover_cache_plugins_with_index(
            &cache_path,
            id,
            &plugins_dir,
            &installed,
            plugin_filter_lookup(id, &sources),
            commit_sha,
        );
        for plugin in available {
            if let (PackageStatus::UpdateAvailable, Some(from)) =
                (plugin.package_status, plugin.installed_version.clone())
            {
                updates.push(UpdateInfo {
                    source_id: id.clone(),
                    plugin_id: plugin.id.clone(),
                    from,
                    to: plugin.available_version,
                    package_hash: plugin.package_hash,
                });
            }
        }
    }

    if !updates.is_empty() {
        let _ = app.emit(
            "hub-updates-available",
            serde_json::json!({ "updates": &updates }),
        );
    }
    Ok(updates)
}
