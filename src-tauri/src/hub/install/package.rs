use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::error::InstallError;

pub const INSTALL_METADATA_SCHEMA_VERSION: u32 = 2;
pub const METADATA_FILENAME: &str = ".openusage-install.json";

fn legacy_metadata_schema_version() -> u32 {
    1
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallMetadata {
    #[serde(default = "legacy_metadata_schema_version", alias = "schema_version")]
    pub schema_version: u32,
    #[serde(alias = "source_id")]
    pub source_id: String,
    #[serde(alias = "source_url")]
    pub source_url: String,
    #[serde(default, alias = "source_label")]
    pub source_label: String,
    #[serde(default, alias = "source_kind")]
    pub source_kind: Option<crate::hub::source::SourceKind>,
    #[serde(default, alias = "source_ref")]
    pub source_ref: Option<String>,
    #[serde(default, alias = "source_commit_sha")]
    pub source_commit_sha: Option<String>,
    #[serde(alias = "plugin_id")]
    pub plugin_id: String,
    #[serde(alias = "installed_version")]
    pub installed_version: String,
    #[serde(default, alias = "package_hash")]
    pub package_hash: String,
    #[serde(alias = "installed_at")]
    pub installed_at: i64,
}

pub fn package_hash(plugin_dir: &Path) -> Result<String, InstallError> {
    let mut files = Vec::new();
    collect_package_files(plugin_dir, &mut files)?;
    files.sort_by(|a, b| {
        package_relative_path(plugin_dir, a).cmp(&package_relative_path(plugin_dir, b))
    });

    let mut hasher = Sha256::new();
    for path in files {
        let rel = package_relative_path(plugin_dir, &path);
        hasher.update(rel.as_bytes());
        hasher.update([0]);
        let bytes = std::fs::read(&path).map_err(|error| InstallError::Io(error.to_string()))?;
        hasher.update(bytes);
        hasher.update([0]);
    }
    let digest = hasher.finalize();
    let hex = digest
        .iter()
        .map(|byte| format!("{:02x}", byte))
        .collect::<String>();
    Ok(format!("sha256:{}", hex))
}

fn collect_package_files(
    dir: &Path,
    files: &mut Vec<std::path::PathBuf>,
) -> Result<(), InstallError> {
    let entries = std::fs::read_dir(dir).map_err(|error| InstallError::Io(error.to_string()))?;
    for entry in entries {
        let entry = entry.map_err(|error| InstallError::Io(error.to_string()))?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| InstallError::Io(error.to_string()))?;
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            if name_str == ".git" {
                continue;
            }
            collect_package_files(&path, files)?;
        } else if file_type.is_file() && !should_skip_package_file(&name_str) {
            files.push(path);
        }
    }
    Ok(())
}

fn package_relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

pub(super) fn should_skip_package_file(name: &str) -> bool {
    name == METADATA_FILENAME
        || name == ".DS_Store"
        || name == "test-helpers.js"
        || name.ends_with(".test.js")
        || name.ends_with(".test.ts")
        || name.ends_with(".test.tsx")
}

pub fn read_install_metadata(install_dir: &Path, plugin_id: &str) -> Option<InstallMetadata> {
    let path = install_dir.join(plugin_id).join(METADATA_FILENAME);
    let text = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn write_install_metadata(
    install_dir: &Path,
    dir_name: &str,
    metadata: &InstallMetadata,
) -> Result<(), InstallError> {
    let dir = install_dir.join(dir_name);
    std::fs::create_dir_all(&dir).map_err(|error| InstallError::Io(error.to_string()))?;
    write_install_metadata_file(&dir, metadata)
}

pub(super) fn write_install_metadata_file(
    plugin_dir: &Path,
    metadata: &InstallMetadata,
) -> Result<(), InstallError> {
    std::fs::create_dir_all(plugin_dir).map_err(|error| InstallError::Io(error.to_string()))?;
    let path = plugin_dir.join(METADATA_FILENAME);
    let text = serde_json::to_string_pretty(metadata)
        .map_err(|error| InstallError::ManifestParse(error.to_string()))?;
    std::fs::write(&path, text).map_err(|error| InstallError::Io(error.to_string()))?;
    Ok(())
}
