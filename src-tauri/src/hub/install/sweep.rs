use std::collections::HashSet;
use std::path::Path;

use super::names::is_internal_dir_name;
use super::package::read_install_metadata;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct OrphanReport {
    pub removed_cache_dirs: Vec<String>,
    pub unmanaged_plugins: Vec<String>,
    pub orphan_source_plugins: Vec<String>,
}

pub fn startup_sweep(
    hub_dir: &Path,
    plugins_dir: &Path,
    registry: &crate::hub::registry::RegistryFile,
) -> OrphanReport {
    let mut report = OrphanReport::default();
    let valid_ids: HashSet<&str> = registry
        .sources
        .iter()
        .map(|source| source.id.as_str())
        .collect();

    let cache_dir = hub_dir.join("cache");
    if cache_dir.is_dir()
        && let Ok(entries) = std::fs::read_dir(&cache_dir)
    {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy().to_string();
            if is_internal_dir_name(&name_str) {
                continue;
            }
            if !valid_ids.contains(name_str.as_str()) {
                let path = entry.path();
                if path.is_dir() && std::fs::remove_dir_all(&path).is_ok() {
                    log::info!("hub sweep: removed orphan cache {}", name_str);
                    report.removed_cache_dirs.push(name_str);
                }
            }
        }
    }

    if plugins_dir.is_dir()
        && let Ok(entries) = std::fs::read_dir(plugins_dir)
    {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name_str = name.to_string_lossy().to_string();
            if is_internal_dir_name(&name_str) {
                continue;
            }
            if !path.is_dir() {
                continue;
            }
            match read_install_metadata(plugins_dir, &name_str) {
                Some(metadata) => {
                    if !valid_ids.contains(metadata.source_id.as_str()) {
                        report.orphan_source_plugins.push(name_str);
                    }
                }
                None => {
                    report.unmanaged_plugins.push(name_str);
                }
            }
        }
    }

    report
}
