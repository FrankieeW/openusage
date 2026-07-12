use std::path::Path;

use super::{
    HubError, InstalledLookup, PackageStatus, PluginInfo, SkippedPlugin, cache_index, install,
    now_millis,
    package_state::{classify_other_source_package, classify_same_source_package},
};

/// Walks `cache_dir/plugins/<id>/plugin.json` and returns parsed PluginInfo plus
/// any skipped entries. Pure function — testable with tempdir fixtures.
///
/// If `plugin_filter` is `Some(list)` and non-empty, only plugins whose id is in
/// the list are returned. `None` or empty list means "all plugins".
pub fn discover_cache_plugins(
    cache_dir: &Path,
    source_id: &str,
    plugins_dir: &Path,
    installed: &InstalledLookup,
    plugin_filter: Option<&[String]>,
) -> (Vec<PluginInfo>, Vec<SkippedPlugin>) {
    let plugins_subdir = cache_dir.join("plugins");
    let mut available = Vec::new();
    let mut skipped = Vec::new();
    let filter_set: Option<std::collections::HashSet<&str>> = plugin_filter
        .filter(|ids| !ids.is_empty())
        .map(|ids| ids.iter().map(|s| s.as_str()).collect());

    if !plugins_subdir.is_dir() {
        return (available, skipped);
    }

    let entries = match std::fs::read_dir(&plugins_subdir) {
        Ok(e) => e,
        Err(err) => {
            skipped.push(SkippedPlugin {
                path: plugins_subdir.display().to_string(),
                reason: format!("read_dir: {}", err),
            });
            return (available, skipped);
        }
    };

    for entry in entries.flatten() {
        let plugin_dir = entry.path();
        if !plugin_dir.is_dir() {
            continue;
        }
        let id = entry.file_name().to_string_lossy().to_string();
        if let Some(set) = filter_set.as_ref()
            && !set.contains(id.as_str())
        {
            continue;
        }
        let manifest_path = plugin_dir.join("plugin.json");
        let text = match std::fs::read_to_string(&manifest_path) {
            Ok(t) => t,
            Err(err) => {
                skipped.push(SkippedPlugin {
                    path: manifest_path.display().to_string(),
                    reason: format!("read: {}", err),
                });
                continue;
            }
        };
        let value: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(err) => {
                skipped.push(SkippedPlugin {
                    path: manifest_path.display().to_string(),
                    reason: format!("parse: {}", err),
                });
                continue;
            }
        };

        let schema_version = value
            .get("schemaVersion")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        if schema_version != 1 {
            skipped.push(SkippedPlugin {
                path: manifest_path.display().to_string(),
                reason: format!("unsupported schemaVersion: {}", schema_version),
            });
            continue;
        }

        let manifest_id = value.get("id").and_then(|v| v.as_str()).unwrap_or("");
        if let Err(err) = install::validate_id_match(&id, manifest_id) {
            skipped.push(SkippedPlugin {
                path: manifest_path.display().to_string(),
                reason: err.to_string(),
            });
            continue;
        }

        let entry_filename = value
            .get("entry")
            .and_then(|v| v.as_str())
            .unwrap_or("plugin.js");
        if let Err(err) = install::validate_entry_within_dir(&plugin_dir, entry_filename) {
            skipped.push(SkippedPlugin {
                path: manifest_path.display().to_string(),
                reason: err.to_string(),
            });
            continue;
        }

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
        let icon_data_url = read_icon_data_url(&plugin_dir, icon_filename);
        let package_hash = match install::package_hash(&plugin_dir) {
            Ok(hash) => hash,
            Err(err) => {
                skipped.push(SkippedPlugin {
                    path: plugin_dir.display().to_string(),
                    reason: format!("hash: {}", err),
                });
                continue;
            }
        };

        let (installed_flag, installed_source_id, installed_version, unmanaged, package_status) =
            match installed.get(&id) {
                Some(info) if info.source_id == source_id => (
                    true,
                    Some(info.source_id.clone()),
                    Some(info.version.clone()),
                    false,
                    classify_same_source_package(info, &version, &package_hash),
                ),
                Some(info) => (
                    false,
                    Some(info.source_id.clone()),
                    Some(info.version.clone()),
                    false,
                    classify_other_source_package(info, &package_hash),
                ),
                None => {
                    if plugins_dir.join(&id).is_dir() {
                        (true, None, None, true, PackageStatus::UnmanagedInstalled)
                    } else {
                        (false, None, None, false, PackageStatus::NotInstalled)
                    }
                }
            };

        let update_available = package_status == PackageStatus::UpdateAvailable;

        available.push(PluginInfo {
            id,
            name,
            brand_color,
            icon_data_url,
            source_id: source_id.to_string(),
            installed: installed_flag,
            installed_source_id,
            unmanaged,
            installed_version,
            available_version: version,
            updated_at,
            package_hash,
            package_status,
            update_available,
        });
    }

    available.sort_by(|a, b| a.id.cmp(&b.id));
    (available, skipped)
}

pub(super) fn discover_cache_plugins_with_index(
    cache_dir: &Path,
    source_id: &str,
    plugins_dir: &Path,
    installed: &InstalledLookup,
    plugin_filter: Option<&[String]>,
    commit_sha: Option<String>,
) -> (Vec<PluginInfo>, Vec<SkippedPlugin>) {
    if let Some(commit_sha) = commit_sha.as_deref()
        && let Some(index) = cache_index::load(cache_dir, source_id, commit_sha, plugin_filter)
    {
        let available = index
            .plugins
            .iter()
            .map(|plugin| cached_plugin_to_info(plugin, source_id, plugins_dir, installed))
            .collect::<Vec<_>>();
        return (available, index.skipped);
    }

    let (available, skipped) =
        discover_cache_plugins(cache_dir, source_id, plugins_dir, installed, plugin_filter);
    if let Some(commit_sha) = commit_sha.as_deref() {
        let index = cache_index::build(
            source_id,
            commit_sha,
            plugin_filter,
            now_millis(),
            &available,
            &skipped,
        );
        if let Err(err) = cache_index::write(cache_dir, &index) {
            log::warn!("cache index write failed for {}: {}", source_id, err);
        }
    }
    (available, skipped)
}

fn cached_plugin_to_info(
    plugin: &cache_index::CachedPluginSummary,
    source_id: &str,
    plugins_dir: &Path,
    installed: &InstalledLookup,
) -> PluginInfo {
    let (installed_flag, installed_source_id, installed_version, unmanaged, package_status) =
        match installed.get(&plugin.id) {
            Some(info) if info.source_id == source_id => (
                true,
                Some(info.source_id.clone()),
                Some(info.version.clone()),
                false,
                classify_same_source_package(info, &plugin.available_version, &plugin.package_hash),
            ),
            Some(info) => (
                false,
                Some(info.source_id.clone()),
                Some(info.version.clone()),
                false,
                classify_other_source_package(info, &plugin.package_hash),
            ),
            None => {
                if plugins_dir.join(&plugin.id).is_dir() {
                    (true, None, None, true, PackageStatus::UnmanagedInstalled)
                } else {
                    (false, None, None, false, PackageStatus::NotInstalled)
                }
            }
        };

    PluginInfo {
        id: plugin.id.clone(),
        name: plugin.name.clone(),
        brand_color: plugin.brand_color.clone(),
        icon_data_url: plugin.icon_data_url.clone(),
        source_id: source_id.to_string(),
        installed: installed_flag,
        installed_source_id,
        unmanaged,
        installed_version,
        available_version: plugin.available_version.clone(),
        updated_at: plugin.updated_at,
        package_hash: plugin.package_hash.clone(),
        package_status,
        update_available: package_status == PackageStatus::UpdateAvailable,
    }
}

pub(super) fn validate_source_health(
    cache_dir: &Path,
    source_id: &str,
    plugins_dir: &Path,
    plugin_filter: Option<&[String]>,
) -> Result<(Vec<PluginInfo>, Vec<SkippedPlugin>), HubError> {
    if !cache_dir.join("plugins").is_dir() {
        return Err(HubError::source_health_failed(
            "source has no plugins directory",
            0,
            &[],
        ));
    }

    let installed = InstalledLookup::new();
    let (available, skipped) =
        discover_cache_plugins(cache_dir, source_id, plugins_dir, &installed, plugin_filter);
    if available.is_empty() {
        return Err(HubError::source_health_failed(
            "source has no valid plugins",
            available.len(),
            &skipped,
        ));
    }
    Ok((available, skipped))
}

pub(super) fn read_icon_data_url(plugin_dir: &Path, icon_filename: &str) -> Option<String> {
    let path = plugin_dir.join(icon_filename);
    let bytes = std::fs::read(&path).ok()?;
    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Some(format!("data:image/svg+xml;base64,{}", encoded))
}

pub(super) fn parse_manifest_updated_at(value: &serde_json::Value) -> Option<i64> {
    let raw = value.get("updatedAt")?;
    if let Some(ms) = raw.as_i64() {
        return Some(normalize_epoch_millis(ms));
    }
    let text = raw.as_str()?.trim();
    if text.is_empty() {
        return None;
    }
    let parsed =
        time::OffsetDateTime::parse(text, &time::format_description::well_known::Rfc3339).ok()?;
    Some(parsed.unix_timestamp().saturating_mul(1_000))
}

fn normalize_epoch_millis(value: i64) -> i64 {
    if value > 0 && value < 10_000_000_000 {
        value.saturating_mul(1_000)
    } else {
        value
    }
}
