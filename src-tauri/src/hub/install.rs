// RED phase: install validation + metadata write stubs.
// `validate_id_match`, `validate_entry_within_dir`, `read_install_metadata`,
// `write_install_metadata`, `copy_plugin_to_install_dir`, and the public
// `hub_install` are all stubs that return errors or empty values.

use std::path::Path;
#[cfg(test)] use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallMetadata {
    pub source_id: String,
    pub source_url: String,
    pub plugin_id: String,
    pub installed_version: String,
    pub installed_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallError {
    IdMismatch { dir_name: String, manifest_id: String },
    EntryOutsidePluginDir,
    ConflictWithSource(String),
    ConflictUnmanaged,
    ManifestParse(String),
    Io(String),
}

impl std::fmt::Display for InstallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstallError::IdMismatch { dir_name, manifest_id } => {
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
    let _ = plugin_dir;
    Ok(())
}

pub fn read_install_metadata(install_dir: &Path, plugin_id: &str) -> Option<InstallMetadata> {
    let path = install_dir.join(plugin_id).join(METADATA_FILENAME);
    let text = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn write_install_metadata(
    install_dir: &Path,
    metadata: &InstallMetadata,
) -> Result<(), InstallError> {
    let dir = install_dir.join(&metadata.plugin_id);
    std::fs::create_dir_all(&dir).map_err(|e| InstallError::Io(e.to_string()))?;
    let path = dir.join(METADATA_FILENAME);
    let text = serde_json::to_string_pretty(metadata)
        .map_err(|e| InstallError::ManifestParse(e.to_string()))?;
    std::fs::write(&path, text).map_err(|e| InstallError::Io(e.to_string()))?;
    Ok(())
}

pub fn copy_plugin_to_install_dir(
    source_plugin_dir: &Path,
    install_dir: &Path,
    plugin_id: &str,
) -> Result<(), InstallError> {
    let dest = install_dir.join(plugin_id);
    if dest.exists() {
        std::fs::remove_dir_all(&dest).map_err(|e| InstallError::Io(e.to_string()))?;
    }
    copy_dir_recursive(source_plugin_dir, &dest)
        .map_err(|e| InstallError::Io(e.to_string()))?;
    Ok(())
}

pub fn remove_installed_plugin(install_dir: &Path, plugin_id: &str) -> Result<(), InstallError> {
    let path = install_dir.join(plugin_id);
    if path.exists() {
        std::fs::remove_dir_all(&path).map_err(|e| InstallError::Io(e.to_string()))?;
    }
    Ok(())
}

pub fn check_conflict(
    install_dir: &Path,
    plugin_id: &str,
    candidate_source_id: &str,
) -> Result<(), InstallError> {
    let plugin_dir = install_dir.join(plugin_id);
    if !plugin_dir.exists() {
        return Ok(());
    }
    match read_install_metadata(install_dir, plugin_id) {
        Some(m) if m.source_id == candidate_source_id => Ok(()),
        Some(m) => Err(InstallError::ConflictWithSource(m.source_id)),
        None => Err(InstallError::ConflictUnmanaged),
    }
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
            if name_str.ends_with(".test.js")
                || name_str.ends_with(".test.ts")
                || name_str.ends_with(".test.tsx")
                || name_str == "test-helpers.js"
            {
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
    let valid_ids: HashSet<&str> = registry
        .sources
        .iter()
        .map(|s| s.id.as_str())
        .collect();

    let cache_dir = hub_dir.join("cache");
    if cache_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&cache_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy().to_string();
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
    use crate::hub::registry::{RegistryFile, Source, CURRENT_VERSION};
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
            auto_check: false,
        }
    }

    fn write_metadata(install_dir: &Path, plugin_id: &str, source_id: &str) {
        let m = InstallMetadata {
            source_id: source_id.into(),
            source_url: "https://github.com/foo/bar".into(),
            plugin_id: plugin_id.into(),
            installed_version: "0.6.27".into(),
            installed_at: 0,
        };
        write_install_metadata(install_dir, &m).unwrap();
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
        assert_eq!(report.removed_cache_dirs, vec!["removed-source".to_string()]);
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
        assert_eq!(
            report.orphan_source_plugins,
            vec!["orphan".to_string()]
        );
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
        assert!(validate_entry_within_dir(Path::new("/tmp/x"), "plugin.js").is_ok());
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
            source_id: "src-1".into(),
            source_url: "https://github.com/foo/bar".into(),
            plugin_id: "claude".into(),
            installed_version: "0.6.27".into(),
            installed_at: 1234567890,
        };
        write_install_metadata(&dir, &m).unwrap();
        let loaded = read_install_metadata(&dir, "claude").unwrap();
        assert_eq!(loaded, m);
        // Sidecar file is hidden-named and inside plugin dir
        assert!(dir.join("claude").join(METADATA_FILENAME).exists());
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
    fn remove_installed_plugin_deletes_dir() {
        let dst = tempdir("remove");
        write_plugin(&dst, "claude", "0.6.27");
        assert!(dst.join("claude").exists());
        remove_installed_plugin(&dst, "claude").unwrap();
        assert!(!dst.join("claude").exists());
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
            source_id: "src-existing".into(),
            source_url: "https://github.com/foo/bar".into(),
            plugin_id: "claude".into(),
            installed_version: "0.6.27".into(),
            installed_at: 0,
        };
        write_install_metadata(&dst, &m).unwrap();
        assert!(check_conflict(&dst, "claude", "src-existing").is_ok());
    }

    #[test]
    fn check_conflict_when_metadata_differs() {
        let dst = tempdir("conflict-diff");
        write_plugin(&dst, "claude", "0.6.27");
        let m = InstallMetadata {
            source_id: "src-existing".into(),
            source_url: "https://github.com/foo/bar".into(),
            plugin_id: "claude".into(),
            installed_version: "0.6.27".into(),
            installed_at: 0,
        };
        write_install_metadata(&dst, &m).unwrap();
        assert_eq!(
            check_conflict(&dst, "claude", "src-new"),
            Err(InstallError::ConflictWithSource("src-existing".into())),
        );
    }
}