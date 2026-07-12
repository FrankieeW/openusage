use std::path::Path;

use super::error::InstallError;
use super::names::{is_internal_dir_name, plugin_id_from_install_dir_name};
use super::package::{package_hash, read_install_metadata};

pub fn validate_id_match(dir_name: &str, manifest_id: &str) -> Result<(), InstallError> {
    if dir_name == manifest_id {
        Ok(())
    } else {
        Err(InstallError::IdMismatch {
            dir_name: dir_name.to_string(),
            manifest_id: manifest_id.to_string(),
        })
    }
}

pub fn validate_entry_within_dir(plugin_dir: &Path, entry: &str) -> Result<(), InstallError> {
    if entry.is_empty() {
        return Err(InstallError::EntryOutsidePluginDir);
    }
    let entry_path = Path::new(entry);
    if entry_path.is_absolute() {
        return Err(InstallError::EntryOutsidePluginDir);
    }
    for component in entry_path.components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err(InstallError::EntryOutsidePluginDir);
        }
    }
    if !plugin_dir.join(entry_path).is_file() {
        return Err(InstallError::EntryOutsidePluginDir);
    }
    Ok(())
}

pub(super) fn validate_copied_plugin(
    plugin_dir: &Path,
    install_dir_name: &str,
) -> Result<(), InstallError> {
    let manifest_path = plugin_dir.join("plugin.json");
    let text = std::fs::read_to_string(&manifest_path).map_err(|error| {
        InstallError::ManifestParse(format!("read {}: {}", manifest_path.display(), error))
    })?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|error| InstallError::ManifestParse(error.to_string()))?;
    let schema_version = value
        .get("schemaVersion")
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    if schema_version != 1 {
        return Err(InstallError::ManifestParse(format!(
            "unsupported schemaVersion: {}",
            schema_version
        )));
    }
    let manifest_id = value
        .get("id")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    validate_id_match(
        plugin_id_from_install_dir_name(install_dir_name),
        manifest_id,
    )?;
    let entry_filename = value
        .get("entry")
        .and_then(|value| value.as_str())
        .unwrap_or("plugin.js");
    validate_entry_within_dir(plugin_dir, entry_filename)?;
    let _ = package_hash(plugin_dir)?;
    Ok(())
}

pub fn check_conflict(
    install_dir: &Path,
    plugin_id: &str,
    candidate_source_id: &str,
) -> Result<(), InstallError> {
    let candidate_plugin_id = plugin_id_from_install_dir_name(plugin_id);
    if install_dir.is_dir() {
        let entries =
            std::fs::read_dir(install_dir).map_err(|error| InstallError::Io(error.to_string()))?;
        for entry in entries {
            let entry = entry.map_err(|error| InstallError::Io(error.to_string()))?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let dir_name = entry.file_name().to_string_lossy().to_string();
            if is_internal_dir_name(&dir_name) {
                continue;
            }
            match read_install_metadata(install_dir, &dir_name) {
                Some(metadata) if metadata.plugin_id == candidate_plugin_id => {
                    if metadata.source_id == candidate_source_id {
                        return Ok(());
                    }
                    if metadata.source_id.is_empty() {
                        return Err(InstallError::ConflictUnmanaged);
                    }
                    return Err(InstallError::ConflictWithSource(metadata.source_id));
                }
                Some(_) => {}
                None if dir_name == plugin_id || dir_name == candidate_plugin_id => {
                    return Err(InstallError::ConflictUnmanaged);
                }
                None => {}
            }
        }
    } else if install_dir.exists() {
        return Err(InstallError::Io(format!(
            "{} is not a directory",
            install_dir.display()
        )));
    }
    Ok(())
}
