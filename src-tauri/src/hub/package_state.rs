use std::path::Path;

use super::{InstalledLookup, InstalledLookupEntry, PackageStatus, install};

pub(super) fn classify_same_source_package(
    installed: &InstalledLookupEntry,
    available_version: &str,
    available_hash: &str,
) -> PackageStatus {
    match compare_versions(&installed.version, available_version) {
        Some(std::cmp::Ordering::Less) => PackageStatus::UpdateAvailable,
        Some(std::cmp::Ordering::Greater) => PackageStatus::InstalledNewerThanSource,
        Some(std::cmp::Ordering::Equal) => {
            if installed.package_hash.is_empty() || installed.package_hash == available_hash {
                PackageStatus::Installed
            } else {
                PackageStatus::SourceChanged
            }
        }
        None => {
            if installed.package_hash.is_empty() || installed.package_hash == available_hash {
                PackageStatus::Installed
            } else {
                PackageStatus::SourceChanged
            }
        }
    }
}

pub(super) fn classify_other_source_package(
    installed: &InstalledLookupEntry,
    available_hash: &str,
) -> PackageStatus {
    if !installed.package_hash.is_empty() && installed.package_hash == available_hash {
        PackageStatus::SamePackageFromOtherSource
    } else {
        PackageStatus::DifferentPackageSamePluginId
    }
}

fn compare_versions(installed: &str, available: &str) -> Option<std::cmp::Ordering> {
    if installed == available {
        return Some(std::cmp::Ordering::Equal);
    }
    let installed_parts = parse_numeric_version(installed)?;
    let available_parts = parse_numeric_version(available)?;
    Some(installed_parts.cmp(&available_parts))
}

fn parse_numeric_version(version: &str) -> Option<Vec<u64>> {
    let core = version
        .split_once('-')
        .map(|(core, _)| core)
        .unwrap_or(version);
    let mut parts = Vec::new();
    for part in core.split('.') {
        if part.is_empty() {
            return None;
        }
        parts.push(part.parse::<u64>().ok()?);
    }
    while parts.len() < 3 {
        parts.push(0);
    }
    Some(parts)
}

/// Build a Hub lookup map from the on-disk install directory.
pub fn build_installed_lookup(plugins_dir: &Path) -> InstalledLookup<'_> {
    let mut map = InstalledLookup::new();
    let entries = match std::fs::read_dir(plugins_dir) {
        Ok(e) => e,
        Err(_) => return map,
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
        if let Some(meta) = install::read_install_metadata(plugins_dir, &dir_name) {
            let package_hash = if meta.package_hash.is_empty() {
                match install::package_hash(&path) {
                    Ok(hash) => hash,
                    Err(err) => {
                        log::warn!(
                            "build_installed_lookup: cannot hash {}: {}",
                            path.display(),
                            err
                        );
                        String::new()
                    }
                }
            } else {
                meta.package_hash
            };
            map.insert(
                meta.plugin_id.clone(),
                InstalledLookupEntry {
                    source_id: meta.source_id,
                    source_url: meta.source_url,
                    version: meta.installed_version,
                    package_hash,
                },
            );
        }
    }
    map
}
