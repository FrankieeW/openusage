use std::path::{Path, PathBuf};

use super::{HubError, Source, SourceSnapshot, install};

/// Directory layout helpers.
pub fn hub_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("hub")
}
pub fn cache_dir_for(hub_dir: &Path, source_id: &str) -> PathBuf {
    hub_dir.join("cache").join(source_id)
}

pub(super) fn resync_local_cache(source_path: &Path, cache_path: &Path) -> Result<(), HubError> {
    let parent = cache_path
        .parent()
        .ok_or_else(|| HubError::io("local source cache has no parent directory"))?;
    std::fs::create_dir_all(parent).map_err(|error| HubError::io(error.to_string()))?;

    let cache_name = cache_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("source");
    let suffix = uuid::Uuid::new_v4().simple();
    let staged_path = parent.join(format!(".{cache_name}.refresh-{suffix}"));
    let backup_path = parent.join(format!(".{cache_name}.backup-{suffix}"));

    if let Err(error) = install::copy_dir_to(source_path, &staged_path) {
        let _ = std::fs::remove_dir_all(&staged_path);
        return Err(error.into());
    }

    if cache_path.exists()
        && let Err(error) = std::fs::rename(cache_path, &backup_path)
    {
        let _ = std::fs::remove_dir_all(&staged_path);
        return Err(HubError::io(format!(
            "move existing local cache aside: {error}"
        )));
    }

    if let Err(error) = std::fs::rename(&staged_path, cache_path) {
        let restore_error = if backup_path.exists() {
            std::fs::rename(&backup_path, cache_path).err()
        } else {
            None
        };
        let _ = std::fs::remove_dir_all(&staged_path);
        return Err(match restore_error {
            Some(restore_error) => HubError::io(format!(
                "replace local cache: {error}; restore previous cache: {restore_error}"
            )),
            None => HubError::io(format!("replace local cache: {error}")),
        });
    }

    if backup_path.exists()
        && let Err(error) = std::fs::remove_dir_all(&backup_path)
    {
        log::warn!(
            "resync_local_cache: cannot remove backup {}: {}",
            backup_path.display(),
            error
        );
    }
    Ok(())
}

pub fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Look up a source's plugin_filter from the registry by id.
/// Returns `None` for unknown sources (treated as "no filter").
pub fn plugin_filter_lookup<'a>(source_id: &str, sources: &'a [Source]) -> Option<&'a [String]> {
    sources
        .iter()
        .find(|s| s.id == source_id)
        .and_then(|s| s.plugin_filter.as_deref())
}

/// Normalize a list of plugin ids: trim, drop empties, preserve case, dedupe.
pub fn normalize_plugin_filter(filter: Option<Vec<String>>) -> Option<Vec<String>> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for raw in filter.unwrap_or_default() {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        if seen.insert(trimmed.to_string()) {
            out.push(trimmed.to_string());
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

pub(super) fn source_snapshot(
    source: &Source,
    commit_sha: Option<String>,
    discovered_count: usize,
    skipped_count: usize,
    checked_at: i64,
) -> SourceSnapshot {
    SourceSnapshot {
        branch: source.branch.clone(),
        commit_sha,
        checked_at,
        discovered_count,
        skipped_count,
    }
}

pub fn derive_label_from_url(url: &str) -> String {
    // e.g. "https://github.com/robinebers/openusage" -> "robinebers/openusage"
    url.trim_end_matches('/')
        .trim_end_matches(".git")
        .rsplit('/')
        .take(2)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("/")
}

/// Turn a source label into a safe directory-name component.
pub fn sanitize_label(label: &str) -> String {
    let mut out = String::with_capacity(label.len());
    for c in label.chars() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
            out.push(c);
        } else if c == ' ' || c == '\'' {
            // collapse to nothing — "Frankie's" → "Frankies"
        } else {
            out.push('-');
        }
    }
    out.to_lowercase()
}
