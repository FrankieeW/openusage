#![allow(dead_code)] // many helpers are wired by the Tauri command layer in a later commit

pub mod git_ops;
pub mod install;
pub mod registry;
pub mod source;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub use registry::{
    RegistryFile, Source, DEFAULT_UPSTREAM_ID, DEFAULT_UPSTREAM_LABEL, DEFAULT_UPSTREAM_URL,
};
pub use source::SourceKind;

/// Error type returned to the JS bridge. Always carries a stable `code` and a
/// human-readable `message`. Optional `context` carries structured details
/// (e.g. the conflicting source id for `Conflict`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubError {
    pub code: HubErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum HubErrorCode {
    InvalidUrl,
    GitNotInstalled,
    CloneFailed,
    NotFound,
    Conflict,
    IoError,
    ManifestParse,
}

impl std::fmt::Display for HubError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for HubError {}

impl HubError {
    pub fn new(code: HubErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            context: None,
        }
    }

    pub fn with_context(code: HubErrorCode, message: impl Into<String>, ctx: serde_json::Value) -> Self {
        Self {
            code,
            message: message.into(),
            context: Some(ctx),
        }
    }

    pub fn invalid_url() -> Self {
        Self::new(HubErrorCode::InvalidUrl, "invalid source URL")
    }
    pub fn git_not_installed() -> Self {
        Self::new(HubErrorCode::GitNotInstalled, "git binary not found on PATH")
    }
    pub fn clone_failed(msg: impl Into<String>) -> Self {
        Self::new(HubErrorCode::CloneFailed, msg)
    }
    pub fn not_found(what: impl Into<String>) -> Self {
        Self::new(HubErrorCode::NotFound, what)
    }
    pub fn conflict(other_source_id: &str) -> Self {
        Self::with_context(
            HubErrorCode::Conflict,
            format!("already installed from {}", other_source_id),
            serde_json::json!({ "otherSourceId": other_source_id }),
        )
    }
    pub fn conflict_unmanaged() -> Self {
        Self::new(HubErrorCode::Conflict, "already installed outside Hub")
    }
    pub fn io(msg: impl Into<String>) -> Self {
        Self::new(HubErrorCode::IoError, msg)
    }
    pub fn manifest_parse(msg: impl Into<String>) -> Self {
        Self::new(HubErrorCode::ManifestParse, msg)
    }
}

impl From<install::InstallError> for HubError {
    fn from(e: install::InstallError) -> Self {
        match e {
            install::InstallError::ConflictWithSource(s) => Self::conflict(&s),
            install::InstallError::ConflictUnmanaged => Self::conflict_unmanaged(),
            install::InstallError::ManifestParse(m) => Self::manifest_parse(m),
            install::InstallError::Io(m) => Self::io(m),
            install::InstallError::IdMismatch { dir_name, manifest_id } => {
                Self::manifest_parse(format!("id mismatch: dir={} manifest={}", dir_name, manifest_id))
            }
            install::InstallError::EntryOutsidePluginDir => {
                Self::manifest_parse("entry path escapes plugin dir")
            }
        }
    }
}

impl From<registry::RegistryError> for HubError {
    fn from(e: registry::RegistryError) -> Self {
        match e {
            registry::RegistryError::Io(m) => Self::io(m),
            registry::RegistryError::Json(m) => Self::manifest_parse(m),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub brand_color: Option<String>,
    pub icon_data_url: Option<String>,
    pub source_id: String,
    pub installed: bool,
    pub installed_source_id: Option<String>,
    pub unmanaged: bool,
    pub installed_version: Option<String>,
    pub available_version: String,
    pub update_available: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkippedPlugin {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct HubBrowseView {
    pub source: Source,
    pub available: Vec<PluginInfo>,
    pub skipped: Vec<SkippedPlugin>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    pub source_id: String,
    pub plugin_id: String,
    pub from: String,
    pub to: String,
}

/// What the JS Hub install side knows about an already-installed plugin.
#[derive(Debug, Clone)]
pub struct InstalledLookupEntry {
    pub source_id: String,
    pub source_url: String,
    pub version: String,
}

pub type InstalledLookup<'a> = std::collections::HashMap<String, InstalledLookupEntry>;

/// Walks `cache_dir/plugins/<id>/plugin.json` and returns parsed PluginInfo plus
/// any skipped entries. Pure function — testable with tempdir fixtures.
pub fn discover_cache_plugins(
    cache_dir: &Path,
    source_id: &str,
    plugins_dir: &Path,
    installed: &InstalledLookup,
) -> (Vec<PluginInfo>, Vec<SkippedPlugin>) {
    let plugins_subdir = cache_dir.join("plugins");
    let mut available = Vec::new();
    let mut skipped = Vec::new();

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

        let schema_version = value.get("schemaVersion").and_then(|v| v.as_u64()).unwrap_or(0);
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
        let brand_color = value
            .get("brandColor")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let icon_filename = value
            .get("icon")
            .and_then(|v| v.as_str())
            .unwrap_or("icon.svg");
        let icon_data_url = read_icon_data_url(&plugin_dir, icon_filename);

        let (installed_flag, installed_source_id, installed_version, unmanaged) =
            match installed.get(&id) {
                Some(info) => (true, Some(info.source_id.clone()), Some(info.version.clone()), false),
                None => {
                    if plugins_dir.join(&id).is_dir() {
                        (true, None, None, true)
                    } else {
                        (false, None, None, false)
                    }
                }
            };

        let update_available = installed_flag && installed_version.as_deref() != Some(version.as_str());

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
            update_available,
        });
    }

    available.sort_by(|a, b| a.id.cmp(&b.id));
    (available, skipped)
}

fn read_icon_data_url(plugin_dir: &Path, icon_filename: &str) -> Option<String> {
    let path = plugin_dir.join(icon_filename);
    let bytes = std::fs::read(&path).ok()?;
    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Some(format!("data:image/svg+xml;base64,{}", encoded))
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
        let id = entry.file_name().to_string_lossy().to_string();
        if let Some(meta) = install::read_install_metadata(plugins_dir, &id) {
            map.insert(
                id,
                InstalledLookupEntry {
                    source_id: meta.source_id,
                    source_url: meta.source_url,
                    version: meta.installed_version,
                },
            );
        }
    }
    map
}

/// Directory layout helpers.
pub fn hub_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("hub")
}
pub fn cache_dir_for(hub_dir: &Path, source_id: &str) -> PathBuf {
    hub_dir.join("cache").join(source_id)
}

pub fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
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
            "openusage-hub-mod-{}-{}-{}",
            label,
            std::process::id(),
            suffix
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn write_fake_plugin(parent: &Path, id: &str, version: &str) {
        let dir = parent.join("plugins").join(id);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("plugin.json"),
            format!(
                r##"{{
  "schemaVersion": 1,
  "id": "{}",
  "name": "{}",
  "version": "{}",
  "entry": "plugin.js",
  "icon": "icon.svg",
  "brandColor": "#FF00FF",
  "lines": []
}}"##,
                id, id, version
            ),
        )
        .unwrap();
        fs::write(dir.join("plugin.js"), "globalThis.__openusage_plugin={};")
            .unwrap();
        fs::write(dir.join("icon.svg"), "<svg/>").unwrap();
    }

    #[test]
    fn discover_returns_empty_when_no_plugins_dir() {
        let cache = tempdir("cache-empty");
        let plugins = tempdir("plugins-empty");
        let lookup = InstalledLookup::new();
        let (available, skipped) = discover_cache_plugins(&cache, "src-1", &plugins, &lookup);
        assert!(available.is_empty());
        assert!(skipped.is_empty());
    }

    #[test]
    fn discover_returns_one_plugin_per_subdir() {
        let cache = tempdir("cache-one");
        write_fake_plugin(&cache, "claude", "0.6.27");
        write_fake_plugin(&cache, "codex", "0.6.27");
        let plugins = tempdir("plugins-1");
        let lookup = InstalledLookup::new();
        let (available, skipped) = discover_cache_plugins(&cache, "src-1", &plugins, &lookup);
        assert_eq!(available.len(), 2);
        assert_eq!(available[0].id, "claude");
        assert_eq!(available[1].id, "codex");
        assert!(available.iter().all(|p| p.icon_data_url.is_some()));
        assert!(available.iter().all(|p| p.brand_color.as_deref() == Some("#FF00FF")));
        assert!(skipped.is_empty());
    }

    #[test]
    fn discover_marks_installed_and_update_available() {
        let cache = tempdir("cache-upd");
        write_fake_plugin(&cache, "claude", "0.7.0");
        let plugins = tempdir("plugins-upd");
        write_fake_plugin(&plugins, "claude", "0.6.27"); // older version installed
        let mut lookup = InstalledLookup::new();
        lookup.insert(
            "claude".into(),
            InstalledLookupEntry {
                source_id: "src-1".into(),
                source_url: "https://github.com/foo/bar".into(),
                version: "0.6.27".into(),
            },
        );
        let (available, _skipped) =
            discover_cache_plugins(&cache, "src-1", &plugins, &lookup);
        let claude = available.iter().find(|p| p.id == "claude").unwrap();
        assert!(claude.installed);
        assert_eq!(claude.installed_source_id.as_deref(), Some("src-1"));
        assert_eq!(claude.installed_version.as_deref(), Some("0.6.27"));
        assert_eq!(claude.available_version, "0.7.0");
        assert!(claude.update_available);
        assert!(!claude.unmanaged);
    }

    #[test]
    fn discover_marks_unmanaged_when_dir_exists_but_no_metadata() {
        let cache = tempdir("cache-unmgd");
        write_fake_plugin(&cache, "claude", "0.6.27");
        let plugins = tempdir("plugins-unmgd");
        // Installed plugin lives directly under plugins_dir, NOT under a `plugins/` subdir.
        let installed_dir = plugins.join("claude");
        fs::create_dir_all(&installed_dir).unwrap();
        fs::write(
            installed_dir.join("plugin.json"),
            r##"{"schemaVersion":1,"id":"claude","name":"Claude","version":"0.6.27","entry":"plugin.js","icon":"icon.svg","brandColor":"#000000","lines":[]}"##,
        )
        .unwrap();
        // No metadata sidecar in plugins/claude/.openusage-install.json
        let lookup = InstalledLookup::new();
        let (available, _skipped) = discover_cache_plugins(&cache, "src-1", &plugins, &lookup);
        let claude = available.iter().find(|p| p.id == "claude").unwrap();
        assert!(claude.installed);
        assert!(claude.unmanaged);
        assert!(claude.installed_source_id.is_none());
    }

    #[test]
    fn discover_skips_plugin_with_id_mismatch() {
        let cache = tempdir("cache-mismatch");
        let dir = cache.join("plugins").join("legacy-name");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("plugin.json"),
            r##"{"schemaVersion":1,"id":"claude","name":"Claude","version":"0.6.27","entry":"plugin.js","icon":"icon.svg","brandColor":"#000000","lines":[]}"##,
        )
        .unwrap();
        let plugins = tempdir("plugins-mismatch");
        let (available, skipped) = discover_cache_plugins(&cache, "src-1", &plugins, &InstalledLookup::new());
        assert_eq!(available.len(), 0);
        assert_eq!(skipped.len(), 1);
        assert!(skipped[0].reason.contains("id mismatch"));
    }

    #[test]
    fn discover_skips_plugin_with_unsupported_schema() {
        let cache = tempdir("cache-schema");
        let dir = cache.join("plugins").join("claude");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("plugin.json"),
            r##"{"schemaVersion":99,"id":"claude","name":"Claude","version":"0.6.27","entry":"plugin.js","icon":"icon.svg","brandColor":"#000000","lines":[]}"##,
        )
        .unwrap();
        let plugins = tempdir("plugins-schema");
        let (available, skipped) = discover_cache_plugins(&cache, "src-1", &plugins, &InstalledLookup::new());
        assert!(available.is_empty());
        assert_eq!(skipped.len(), 1);
        assert!(skipped[0].reason.contains("schemaVersion"));
    }

    #[test]
    fn derive_label_extracts_owner_repo() {
        assert_eq!(
            derive_label_from_url("https://github.com/robinebers/openusage"),
            "robinebers/openusage"
        );
        assert_eq!(
            derive_label_from_url("https://gitlab.com/foo/bar.git"),
            "foo/bar"
        );
    }

    #[test]
    fn hub_error_conflict_carries_other_source_id_in_context() {
        let err = HubError::conflict("src-other");
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["code"], "Conflict");
        assert_eq!(json["context"]["otherSourceId"], "src-other");
    }

    #[test]
    fn hub_error_invalid_url_omits_context() {
        let err = HubError::invalid_url();
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["code"], "InvalidUrl");
        assert!(json.get("context").is_none() || json["context"].is_null());
    }
}