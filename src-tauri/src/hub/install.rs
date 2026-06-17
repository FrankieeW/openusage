use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const INSTALL_METADATA_SCHEMA_VERSION: u32 = 2;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallError {
    IdMismatch {
        dir_name: String,
        manifest_id: String,
    },
    EntryOutsidePluginDir,
    ConflictWithSource(String),
    ConflictUnmanaged,
    ManifestParse(String),
    Io(String),
}

impl std::fmt::Display for InstallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstallError::IdMismatch {
                dir_name,
                manifest_id,
            } => {
                write!(f, "id mismatch: dir={} manifest={}", dir_name, manifest_id)
            }
            InstallError::EntryOutsidePluginDir => write!(f, "entry path escapes plugin dir"),
            InstallError::ConflictWithSource(s) => write!(f, "already installed from {}", s),
            InstallError::ConflictUnmanaged => write!(f, "already installed outside Hub"),
            InstallError::ManifestParse(m) => write!(f, "manifest parse: {}", m),
            InstallError::Io(m) => write!(f, "io: {}", m),
        }
    }
}

impl std::error::Error for InstallError {}

pub const METADATA_FILENAME: &str = ".openusage-install.json";
pub const HUB_TRASH_DIRNAME: &str = ".openusage-trash";
const HUB_INSTALL_TMP_PREFIX: &str = ".openusage-installing";

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
        let bytes = std::fs::read(&path).map_err(|e| InstallError::Io(e.to_string()))?;
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

pub fn is_internal_dir_name(name: &str) -> bool {
    name == HUB_TRASH_DIRNAME || name.starts_with(HUB_INSTALL_TMP_PREFIX)
}

fn collect_package_files(
    dir: &Path,
    files: &mut Vec<std::path::PathBuf>,
) -> Result<(), InstallError> {
    let entries = std::fs::read_dir(dir).map_err(|e| InstallError::Io(e.to_string()))?;
    for entry in entries {
        let entry = entry.map_err(|e| InstallError::Io(e.to_string()))?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|e| InstallError::Io(e.to_string()))?;
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

fn should_skip_package_file(name: &str) -> bool {
    name == METADATA_FILENAME
        || name == ".DS_Store"
        || name == "test-helpers.js"
        || name.ends_with(".test.js")
        || name.ends_with(".test.ts")
        || name.ends_with(".test.tsx")
}

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
    std::fs::create_dir_all(&dir).map_err(|e| InstallError::Io(e.to_string()))?;
    write_install_metadata_file(&dir, metadata)
}

fn write_install_metadata_file(
    plugin_dir: &Path,
    metadata: &InstallMetadata,
) -> Result<(), InstallError> {
    std::fs::create_dir_all(plugin_dir).map_err(|e| InstallError::Io(e.to_string()))?;
    let path = plugin_dir.join(METADATA_FILENAME);
    let text = serde_json::to_string_pretty(metadata)
        .map_err(|e| InstallError::ManifestParse(e.to_string()))?;
    std::fs::write(&path, text).map_err(|e| InstallError::Io(e.to_string()))?;
    Ok(())
}

pub fn switch_plugin_install_dir_with_metadata(
    source_plugin_dir: &Path,
    install_dir: &Path,
    old_dir_name: &str,
    new_dir_name: &str,
    metadata: &InstallMetadata,
) -> Result<(), InstallError> {
    std::fs::create_dir_all(install_dir).map_err(|e| InstallError::Io(e.to_string()))?;

    let temp = unique_install_temp_dir(install_dir, new_dir_name);
    copy_dir_recursive(source_plugin_dir, &temp).map_err(|e| {
        cleanup_dir_best_effort(&temp);
        InstallError::Io(e.to_string())
    })?;
    if let Err(error) = validate_copied_plugin(&temp, new_dir_name) {
        cleanup_dir_best_effort(&temp);
        return Err(error);
    }
    if let Err(error) = write_install_metadata_file(&temp, metadata) {
        cleanup_dir_best_effort(&temp);
        return Err(error);
    }

    finish_switch_plugin_install_dir(temp, install_dir, old_dir_name, new_dir_name)
}

pub fn copy_plugin_to_install_dir(
    source_plugin_dir: &Path,
    install_dir: &Path,
    plugin_id: &str,
) -> Result<(), InstallError> {
    switch_plugin_install_dir(source_plugin_dir, install_dir, plugin_id, plugin_id)
}

pub fn switch_plugin_install_dir(
    source_plugin_dir: &Path,
    install_dir: &Path,
    old_dir_name: &str,
    new_dir_name: &str,
) -> Result<(), InstallError> {
    std::fs::create_dir_all(install_dir).map_err(|e| InstallError::Io(e.to_string()))?;

    let temp = unique_install_temp_dir(install_dir, new_dir_name);
    copy_dir_recursive(source_plugin_dir, &temp).map_err(|e| {
        cleanup_dir_best_effort(&temp);
        InstallError::Io(e.to_string())
    })?;
    if let Err(error) = validate_copied_plugin(&temp, new_dir_name) {
        cleanup_dir_best_effort(&temp);
        return Err(error);
    }

    finish_switch_plugin_install_dir(temp, install_dir, old_dir_name, new_dir_name)
}

fn finish_switch_plugin_install_dir(
    temp: std::path::PathBuf,
    install_dir: &Path,
    old_dir_name: &str,
    new_dir_name: &str,
) -> Result<(), InstallError> {
    let mut moved_dirs: Vec<(std::path::PathBuf, std::path::PathBuf)> = Vec::new();
    let old_path = install_dir.join(old_dir_name);
    if old_path.exists() {
        let backup = unique_trash_dir(install_dir, old_dir_name);
        move_dir_for_replace(&old_path, &backup)?;
        moved_dirs.push((old_path, backup));
    }

    let dest = install_dir.join(new_dir_name);
    if new_dir_name != old_dir_name && dest.exists() {
        let backup = unique_trash_dir(install_dir, new_dir_name);
        move_dir_for_replace(&dest, &backup)?;
        moved_dirs.push((dest.clone(), backup));
    }

    if let Err(error) = std::fs::rename(&temp, &dest) {
        for (original, backup) in moved_dirs.iter().rev() {
            let _ = std::fs::rename(backup, original);
        }
        cleanup_dir_best_effort(&temp);
        return Err(InstallError::Io(error.to_string()));
    }

    Ok(())
}

pub fn remove_installed_plugin(install_dir: &Path, plugin_id: &str) -> Result<(), InstallError> {
    let path = install_dir.join(plugin_id);
    if path.exists() {
        let backup = unique_trash_dir(install_dir, plugin_id);
        if let Some(parent) = backup.parent() {
            std::fs::create_dir_all(parent).map_err(|e| InstallError::Io(e.to_string()))?;
        }
        std::fs::rename(&path, &backup).map_err(|e| InstallError::Io(e.to_string()))?;
    }
    Ok(())
}

pub fn hub_trash_dir(install_dir: &Path) -> std::path::PathBuf {
    install_dir.join(HUB_TRASH_DIRNAME)
}

fn validate_copied_plugin(plugin_dir: &Path, install_dir_name: &str) -> Result<(), InstallError> {
    let manifest_path = plugin_dir.join("plugin.json");
    let text = std::fs::read_to_string(&manifest_path).map_err(|e| {
        InstallError::ManifestParse(format!("read {}: {}", manifest_path.display(), e))
    })?;
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| InstallError::ManifestParse(e.to_string()))?;
    let schema_version = value
        .get("schemaVersion")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    if schema_version != 1 {
        return Err(InstallError::ManifestParse(format!(
            "unsupported schemaVersion: {}",
            schema_version
        )));
    }
    let manifest_id = value.get("id").and_then(|v| v.as_str()).unwrap_or("");
    validate_id_match(
        plugin_id_from_install_dir_name(install_dir_name),
        manifest_id,
    )?;
    let entry_filename = value
        .get("entry")
        .and_then(|v| v.as_str())
        .unwrap_or("plugin.js");
    validate_entry_within_dir(plugin_dir, entry_filename)?;
    let _ = package_hash(plugin_dir)?;
    Ok(())
}

fn unique_install_temp_dir(install_dir: &Path, plugin_id: &str) -> std::path::PathBuf {
    unique_child_path(
        install_dir,
        &format!(
            "{}-{}",
            HUB_INSTALL_TMP_PREFIX,
            sanitize_backup_component(plugin_id)
        ),
    )
}

fn unique_trash_dir(install_dir: &Path, plugin_id: &str) -> std::path::PathBuf {
    unique_child_path(
        &hub_trash_dir(install_dir),
        &sanitize_backup_component(plugin_id),
    )
}

fn move_dir_for_replace(from: &Path, to: &Path) -> Result<(), InstallError> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).map_err(|e| InstallError::Io(e.to_string()))?;
    }
    std::fs::rename(from, to).map_err(|e| InstallError::Io(e.to_string()))
}

fn unique_child_path(parent: &Path, prefix: &str) -> std::path::PathBuf {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let counter = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    parent.join(format!(
        "{}-{}-{}-{}",
        prefix,
        std::process::id(),
        nanos,
        counter
    ))
}

fn sanitize_backup_component(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

fn cleanup_dir_best_effort(path: &Path) {
    if path.exists() {
        let Some(parent) = path.parent() else {
            return;
        };
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("install-temp");
        let backup = unique_child_path(&hub_trash_dir(parent), &sanitize_backup_component(name));
        if let Some(trash_parent) = backup.parent() {
            let _ = std::fs::create_dir_all(trash_parent);
        }
        let _ = std::fs::rename(path, backup);
    }
}

pub fn check_conflict(
    install_dir: &Path,
    plugin_id: &str,
    candidate_source_id: &str,
) -> Result<(), InstallError> {
    let candidate_plugin_id = plugin_id_from_install_dir_name(plugin_id);
    if install_dir.is_dir() {
        let entries =
            std::fs::read_dir(install_dir).map_err(|e| InstallError::Io(e.to_string()))?;
        for entry in entries {
            let entry = entry.map_err(|e| InstallError::Io(e.to_string()))?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let dir_name = entry.file_name().to_string_lossy().to_string();
            if is_internal_dir_name(&dir_name) {
                continue;
            }
            match read_install_metadata(install_dir, &dir_name) {
                Some(m) if m.plugin_id == candidate_plugin_id => {
                    if m.source_id == candidate_source_id {
                        return Ok(());
                    }
                    if m.source_id.is_empty() {
                        return Err(InstallError::ConflictUnmanaged);
                    }
                    return Err(InstallError::ConflictWithSource(m.source_id));
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

fn plugin_id_from_install_dir_name(dir_name: &str) -> &str {
    dir_name
        .split_once("__")
        .map(|(id, _)| id)
        .unwrap_or(dir_name)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let entry_path = entry.path();
        let dest_path = dst.join(&name);
        let ft = entry.file_type()?;
        if ft.is_symlink() {
            continue;
        }
        if ft.is_dir() {
            copy_dir_recursive(&entry_path, &dest_path)?;
        } else if ft.is_file() {
            if should_skip_package_file(&name_str) {
                continue;
            }
            std::fs::copy(&entry_path, &dest_path)?;
        }
    }
    Ok(())
}

/// Public wrapper used by the Tauri command layer (e.g. when seeding a local
/// path source's cache).
pub fn copy_dir_to(src: &Path, dst: &Path) -> Result<(), InstallError> {
    copy_dir_recursive(src, dst).map_err(|e| InstallError::Io(e.to_string()))
}

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
    use std::collections::HashSet;
    let mut report = OrphanReport::default();
    let valid_ids: HashSet<&str> = registry.sources.iter().map(|s| s.id.as_str()).collect();

    let cache_dir = hub_dir.join("cache");
    if cache_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&cache_dir) {
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
    }

    if plugins_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(plugins_dir) {
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
                    Some(m) => {
                        if !valid_ids.contains(m.source_id.as_str()) {
                            report.orphan_source_plugins.push(name_str);
                        }
                    }
                    None => {
                        report.unmanaged_plugins.push(name_str);
                    }
                }
            }
        }
    }

    report
}

#[cfg(test)]
mod sweep_tests {
    use super::*;
    use crate::hub::registry::{CURRENT_VERSION, RegistryFile, Source};
    use crate::hub::source::SourceKind;
    use std::fs;

    fn tempdir(label: &str) -> PathBuf {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let counter = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "openusage-hub-sweep-{}-{}-{}-{}",
            label,
            std::process::id(),
            suffix,
            counter
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn source(id: &str) -> Source {
        Source {
            id: id.into(),
            label: id.into(),
            url: "https://github.com/foo/bar".into(),
            kind: SourceKind::Github,
            added_at: 0,
            last_refreshed_at: None,
            branch: None,
            plugin_filter: None,
            auto_check: false,
        }
    }

    fn write_metadata(install_dir: &Path, plugin_id: &str, source_id: &str) {
        let m = InstallMetadata {
            schema_version: INSTALL_METADATA_SCHEMA_VERSION,
            source_id: source_id.into(),
            source_url: "https://github.com/foo/bar".into(),
            source_label: "".into(),
            source_kind: Some(SourceKind::Github),
            source_ref: Some("main".into()),
            source_commit_sha: Some("abc123".into()),
            plugin_id: plugin_id.into(),
            installed_version: "0.6.27".into(),
            package_hash: "sha256:fixture".into(),
            installed_at: 0,
        };
        write_install_metadata(install_dir, plugin_id, &m).unwrap();
    }

    #[test]
    fn sweep_removes_orphan_cache_dirs() {
        let hub = tempdir("hub");
        let plugins = tempdir("plugins");
        let cache = hub.join("cache");
        fs::create_dir_all(cache.join("valid-source")).unwrap();
        fs::create_dir_all(cache.join("removed-source")).unwrap();
        let registry = RegistryFile {
            version: CURRENT_VERSION,
            sources: vec![source("valid-source")],
        };
        let report = startup_sweep(&hub, &plugins, &registry);
        assert_eq!(
            report.removed_cache_dirs,
            vec!["removed-source".to_string()]
        );
        assert!(cache.join("valid-source").exists());
        assert!(!cache.join("removed-source").exists());
    }

    #[test]
    fn sweep_identifies_unmanaged_plugins() {
        let hub = tempdir("hub");
        let plugins = tempdir("plugins");
        fs::create_dir_all(plugins.join("manual")).unwrap();
        let registry = RegistryFile {
            version: CURRENT_VERSION,
            sources: vec![],
        };
        let report = startup_sweep(&hub, &plugins, &registry);
        assert_eq!(report.unmanaged_plugins, vec!["manual".to_string()]);
        assert!(plugins.join("manual").exists());
    }

    #[test]
    fn sweep_identifies_plugins_with_removed_source() {
        let hub = tempdir("hub");
        let plugins = tempdir("plugins");
        fs::create_dir_all(plugins.join("orphan")).unwrap();
        write_metadata(&plugins, "orphan", "removed-source");
        let registry = RegistryFile {
            version: CURRENT_VERSION,
            sources: vec![source("other-source")],
        };
        let report = startup_sweep(&hub, &plugins, &registry);
        assert_eq!(report.orphan_source_plugins, vec!["orphan".to_string()]);
        assert!(plugins.join("orphan").exists());
    }

    #[test]
    fn sweep_keeps_plugins_with_valid_source() {
        let hub = tempdir("hub");
        let plugins = tempdir("plugins");
        fs::create_dir_all(plugins.join("claude")).unwrap();
        write_metadata(&plugins, "claude", "valid-source");
        let registry = RegistryFile {
            version: CURRENT_VERSION,
            sources: vec![source("valid-source")],
        };
        let report = startup_sweep(&hub, &plugins, &registry);
        assert_eq!(report.orphan_source_plugins, Vec::<String>::new());
        assert_eq!(report.unmanaged_plugins, Vec::<String>::new());
        assert_eq!(report.removed_cache_dirs, Vec::<String>::new());
    }

    #[test]
    fn sweep_empty_dirs_reports_nothing() {
        let hub = tempdir("hub");
        let plugins = tempdir("plugins");
        let registry = RegistryFile {
            version: CURRENT_VERSION,
            sources: vec![],
        };
        let report = startup_sweep(&hub, &plugins, &registry);
        assert_eq!(report, OrphanReport::default());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tempdir(label: &str) -> PathBuf {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "openusage-hub-install-{}-{}-{}",
            label,
            std::process::id(),
            suffix
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn write_plugin(parent: &Path, id: &str, version: &str) -> PathBuf {
        let plugin_dir = parent.join(id);
        fs::create_dir_all(&plugin_dir).unwrap();
        fs::write(
            plugin_dir.join("plugin.json"),
            format!(
                r##"{{
  "schemaVersion": 1,
  "id": "{}",
  "name": "{}",
  "version": "{}",
  "entry": "plugin.js",
  "icon": "icon.svg",
  "brandColor": "#000000",
  "lines": []
}}"##,
                id, id, version
            ),
        )
        .unwrap();
        fs::write(plugin_dir.join("plugin.js"), "// stub").unwrap();
        fs::write(plugin_dir.join("icon.svg"), "<svg/>").unwrap();
        plugin_dir
    }

    fn trash_entries_for(install_dir: &Path, plugin_id: &str) -> Vec<PathBuf> {
        let trash = hub_trash_dir(install_dir);
        if !trash.is_dir() {
            return Vec::new();
        }
        let mut entries = fs::read_dir(trash)
            .unwrap()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.starts_with(plugin_id))
                    .unwrap_or(false)
            })
            .collect::<Vec<_>>();
        entries.sort();
        entries
    }

    #[test]
    fn validate_id_match_accepts_matching() {
        assert!(validate_id_match("claude", "claude").is_ok());
    }

    #[test]
    fn validate_id_match_rejects_mismatch() {
        assert!(matches!(
            validate_id_match("legacy-name", "claude"),
            Err(InstallError::IdMismatch { .. })
        ));
    }

    #[test]
    fn validate_entry_accepts_simple_relative() {
        let dir = tempdir("entry-exists");
        fs::write(dir.join("plugin.js"), "// entry").unwrap();
        assert!(validate_entry_within_dir(&dir, "plugin.js").is_ok());
    }

    #[test]
    fn validate_entry_rejects_missing_file() {
        let dir = tempdir("entry-missing");
        assert_eq!(
            validate_entry_within_dir(&dir, "plugin.js"),
            Err(InstallError::EntryOutsidePluginDir),
        );
    }

    #[test]
    fn validate_entry_rejects_parent_traversal() {
        assert_eq!(
            validate_entry_within_dir(Path::new("/tmp/x"), "../foo.js"),
            Err(InstallError::EntryOutsidePluginDir),
        );
    }

    #[test]
    fn validate_entry_rejects_absolute() {
        assert_eq!(
            validate_entry_within_dir(Path::new("/tmp/x"), "/etc/passwd"),
            Err(InstallError::EntryOutsidePluginDir),
        );
    }

    #[test]
    fn metadata_round_trip() {
        let dir = tempdir("meta");
        let m = InstallMetadata {
            schema_version: INSTALL_METADATA_SCHEMA_VERSION,
            source_id: "src-1".into(),
            source_url: "https://github.com/foo/bar".into(),
            source_label: "".into(),
            source_kind: Some(crate::hub::source::SourceKind::Github),
            source_ref: Some("main".into()),
            source_commit_sha: Some("abc123".into()),
            plugin_id: "claude".into(),
            installed_version: "0.6.27".into(),
            package_hash: "sha256:fixture".into(),
            installed_at: 1234567890,
        };
        write_install_metadata(&dir, "claude", &m).unwrap();
        let loaded = read_install_metadata(&dir, "claude").unwrap();
        assert_eq!(loaded, m);
        // Sidecar file is hidden-named and inside plugin dir
        assert!(dir.join("claude").join(METADATA_FILENAME).exists());
    }

    #[test]
    fn legacy_metadata_defaults_missing_v2_fields() {
        let dir = tempdir("legacy-meta");
        fs::create_dir_all(dir.join("claude")).unwrap();
        fs::write(
            dir.join("claude").join(METADATA_FILENAME),
            r##"{
  "source_id": "src-1",
  "source_url": "https://github.com/foo/bar",
  "source_label": "Foo",
  "plugin_id": "claude",
  "installed_version": "0.6.27",
  "installed_at": 123
}"##,
        )
        .unwrap();

        let loaded = read_install_metadata(&dir, "claude").unwrap();

        assert_eq!(loaded.schema_version, 1);
        assert_eq!(loaded.package_hash, "");
        assert_eq!(loaded.source_kind, None);
        assert_eq!(loaded.source_ref, None);
        assert_eq!(loaded.source_commit_sha, None);
    }

    #[test]
    fn package_hash_is_stable_and_ignores_install_metadata() {
        let root = tempdir("hash-stable");
        let plugin_dir = write_plugin(&root, "claude", "0.6.27");

        let before = package_hash(&plugin_dir).unwrap();
        fs::write(plugin_dir.join(METADATA_FILENAME), "{}").unwrap();
        let after = package_hash(&plugin_dir).unwrap();

        assert_eq!(before, after);
        assert!(before.starts_with("sha256:"));
    }

    #[test]
    fn package_hash_changes_when_plugin_file_changes() {
        let root = tempdir("hash-change");
        let plugin_dir = write_plugin(&root, "claude", "0.6.27");

        let before = package_hash(&plugin_dir).unwrap();
        fs::write(plugin_dir.join("plugin.js"), "// changed").unwrap();
        let after = package_hash(&plugin_dir).unwrap();

        assert_ne!(before, after);
    }

    #[test]
    fn package_hash_ignores_files_excluded_from_install_copy() {
        let root = tempdir("hash-copy-excludes");
        let plugin_dir = write_plugin(&root, "claude", "0.6.27");

        let before = package_hash(&plugin_dir).unwrap();
        fs::write(plugin_dir.join("plugin.test.ts"), "throw new Error('test')").unwrap();
        fs::write(
            plugin_dir.join("test-helpers.js"),
            "export const helper = true",
        )
        .unwrap();
        let after = package_hash(&plugin_dir).unwrap();

        assert_eq!(before, after);
    }

    #[test]
    fn copy_installs_plugin_and_writes_metadata() {
        let src = tempdir("src");
        let dst = tempdir("dst");
        let plugin_dir = write_plugin(&src, "claude", "0.6.27");

        copy_plugin_to_install_dir(&plugin_dir, &dst, "claude").unwrap();

        let installed = dst.join("claude");
        assert!(installed.join("plugin.json").exists());
        assert!(installed.join("plugin.js").exists());
        assert!(installed.join("icon.svg").exists());
    }

    #[test]
    fn copy_keeps_existing_install_when_candidate_is_invalid() {
        let src = tempdir("invalid-src");
        let dst = tempdir("invalid-dst");
        let existing = write_plugin(&dst, "claude", "0.6.27");
        fs::write(existing.join("plugin.js"), "// existing").unwrap();

        let candidate = write_plugin(&src, "claude", "0.6.28");
        fs::write(
            candidate.join("plugin.json"),
            r##"{
  "schemaVersion": 1,
  "id": "wrong-id",
  "name": "Claude",
  "version": "0.6.28",
  "entry": "plugin.js",
  "icon": "icon.svg",
  "brandColor": "#000000",
  "lines": []
}"##,
        )
        .unwrap();

        assert!(copy_plugin_to_install_dir(&candidate, &dst, "claude").is_err());

        assert_eq!(
            fs::read_to_string(dst.join("claude").join("plugin.js")).unwrap(),
            "// existing"
        );
        assert!(trash_entries_for(&dst, "claude").is_empty());
    }

    #[test]
    fn copy_replaces_existing_install_after_moving_old_dir_to_trash() {
        let src = tempdir("replace-src");
        let dst = tempdir("replace-dst");
        let existing = write_plugin(&dst, "claude", "0.6.27");
        fs::write(existing.join("plugin.js"), "// old").unwrap();
        let candidate = write_plugin(&src, "claude", "0.6.28");
        fs::write(candidate.join("plugin.js"), "// new").unwrap();

        copy_plugin_to_install_dir(&candidate, &dst, "claude").unwrap();

        assert_eq!(
            fs::read_to_string(dst.join("claude").join("plugin.js")).unwrap(),
            "// new"
        );
        let trashed = trash_entries_for(&dst, "claude");
        assert_eq!(trashed.len(), 1);
        assert_eq!(
            fs::read_to_string(trashed[0].join("plugin.js")).unwrap(),
            "// old"
        );
    }

    #[test]
    fn remove_installed_plugin_moves_dir_to_trash() {
        let dst = tempdir("remove");
        write_plugin(&dst, "claude", "0.6.27");
        assert!(dst.join("claude").exists());
        remove_installed_plugin(&dst, "claude").unwrap();
        assert!(!dst.join("claude").exists());
        let trashed = trash_entries_for(&dst, "claude");
        assert_eq!(trashed.len(), 1);
        assert!(trashed[0].join("plugin.json").exists());
    }

    #[test]
    fn switch_plugin_install_dir_moves_old_source_to_trash_and_installs_new_source_dir() {
        let src = tempdir("switch-src");
        let dst = tempdir("switch-dst");
        let old = write_plugin(&dst, "claude__old", "0.6.27");
        fs::write(old.join("plugin.js"), "// old").unwrap();
        let candidate = write_plugin(&src, "claude", "0.6.28");
        fs::write(candidate.join("plugin.js"), "// new").unwrap();

        switch_plugin_install_dir(&candidate, &dst, "claude__old", "claude__new").unwrap();

        assert!(!dst.join("claude__old").exists());
        assert_eq!(
            fs::read_to_string(dst.join("claude__new").join("plugin.js")).unwrap(),
            "// new"
        );
        let trashed = trash_entries_for(&dst, "claude__old");
        assert_eq!(trashed.len(), 1);
        assert_eq!(
            fs::read_to_string(trashed[0].join("plugin.js")).unwrap(),
            "// old"
        );
    }

    #[test]
    fn switch_plugin_install_dir_with_metadata_installs_metadata_atomically() {
        let src = tempdir("switch-meta-src");
        let dst = tempdir("switch-meta-dst");
        let candidate = write_plugin(&src, "claude", "0.6.28");
        let metadata = InstallMetadata {
            schema_version: INSTALL_METADATA_SCHEMA_VERSION,
            source_id: "src-new".into(),
            source_url: "https://github.com/foo/bar".into(),
            source_label: "Foo".into(),
            source_kind: Some(crate::hub::source::SourceKind::Github),
            source_ref: Some("main".into()),
            source_commit_sha: Some("abc123".into()),
            plugin_id: "claude".into(),
            installed_version: "0.6.28".into(),
            package_hash: "sha256:new".into(),
            installed_at: 123,
        };

        switch_plugin_install_dir_with_metadata(&candidate, &dst, "claude", "claude", &metadata)
            .unwrap();

        assert_eq!(read_install_metadata(&dst, "claude").unwrap(), metadata);
    }

    #[test]
    fn remove_installed_plugin_missing_is_ok() {
        let dst = tempdir("remove-missing");
        assert!(remove_installed_plugin(&dst, "claude").is_ok());
    }

    #[test]
    fn check_conflict_unmanaged_when_no_metadata() {
        let dst = tempdir("conflict-unmanaged");
        write_plugin(&dst, "claude", "0.6.27");
        // No metadata sidecar — should report unmanaged
        assert_eq!(
            check_conflict(&dst, "claude", "src-new"),
            Err(InstallError::ConflictUnmanaged),
        );
    }

    #[test]
    fn check_conflict_when_metadata_matches_candidate() {
        let dst = tempdir("conflict-match");
        write_plugin(&dst, "claude", "0.6.27");
        let m = InstallMetadata {
            schema_version: INSTALL_METADATA_SCHEMA_VERSION,
            source_id: "src-existing".into(),
            source_url: "https://github.com/foo/bar".into(),
            source_label: "".into(),
            source_kind: Some(crate::hub::source::SourceKind::Github),
            source_ref: Some("main".into()),
            source_commit_sha: Some("abc123".into()),
            plugin_id: "claude".into(),
            installed_version: "0.6.27".into(),
            package_hash: "sha256:fixture".into(),
            installed_at: 0,
        };
        write_install_metadata(&dst, "claude", &m).unwrap();
        assert!(check_conflict(&dst, "claude", "src-existing").is_ok());
    }

    #[test]
    fn check_conflict_when_metadata_differs() {
        let dst = tempdir("conflict-diff");
        write_plugin(&dst, "claude", "0.6.27");
        let m = InstallMetadata {
            schema_version: INSTALL_METADATA_SCHEMA_VERSION,
            source_id: "src-existing".into(),
            source_url: "https://github.com/foo/bar".into(),
            source_label: "".into(),
            source_kind: Some(crate::hub::source::SourceKind::Github),
            source_ref: Some("main".into()),
            source_commit_sha: Some("abc123".into()),
            plugin_id: "claude".into(),
            installed_version: "0.6.27".into(),
            package_hash: "sha256:fixture".into(),
            installed_at: 0,
        };
        write_install_metadata(&dst, "claude", &m).unwrap();
        assert_eq!(
            check_conflict(&dst, "claude", "src-new"),
            Err(InstallError::ConflictWithSource("src-existing".into())),
        );
    }

    #[test]
    fn check_conflict_when_same_plugin_id_exists_in_source_scoped_dir() {
        let dst = tempdir("conflict-scoped-dir");
        write_plugin(&dst, "claude__source-a", "0.6.27");
        let m = InstallMetadata {
            schema_version: INSTALL_METADATA_SCHEMA_VERSION,
            source_id: "src-existing".into(),
            source_url: "https://github.com/foo/bar".into(),
            source_label: "".into(),
            source_kind: Some(crate::hub::source::SourceKind::Github),
            source_ref: Some("main".into()),
            source_commit_sha: Some("abc123".into()),
            plugin_id: "claude".into(),
            installed_version: "0.6.27".into(),
            package_hash: "sha256:fixture".into(),
            installed_at: 0,
        };
        write_install_metadata(&dst, "claude__source-a", &m).unwrap();

        assert_eq!(
            check_conflict(&dst, "claude__source-b", "src-new"),
            Err(InstallError::ConflictWithSource("src-existing".into())),
        );
    }
}
