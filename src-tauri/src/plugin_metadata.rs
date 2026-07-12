use std::path::Path;
use std::sync::Mutex;

use serde::Serialize;

use crate::{AppState, hub, plugin_engine};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginMeta {
    pub id: String,
    pub name: String,
    pub icon_url: String,
    pub brand_color: Option<String>,
    pub lines: Vec<ManifestLineDto>,
    pub links: Vec<PluginLinkDto>,
    pub primary_candidates: Vec<String>,
    pub weekly_candidate: Option<String>,
    /// Human-readable source label (e.g. "Frankie's") from Hub metadata.
    /// None for unmanaged/local plugins.
    pub source_label: Option<String>,
    /// Installed version read from the Hub install metadata (`installed_version`).
    /// None for plugins installed outside the Hub (unmanaged / local).
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestLineDto {
    #[serde(rename = "type")]
    pub line_type: String,
    pub label: String,
    pub scope: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginLinkDto {
    pub label: String,
    pub url: String,
}

#[tauri::command]
pub(crate) fn list_plugins(state: tauri::State<'_, Mutex<AppState>>) -> Vec<PluginMeta> {
    let (plugins, plugins_dir) = {
        let locked = state.lock().expect("plugin state poisoned");
        (locked.plugins.clone(), locked.plugins_dir.clone())
    };
    log::debug!("list_plugins: {} plugins", plugins.len());
    plugins_to_meta(&plugins, &plugins_dir)
}

/// Read install metadata from the plugin directory that was actually loaded.
/// This preserves source-scoped install dirs while avoiding unrelated or
/// transient metadata that may exist elsewhere under plugins/.
fn read_install_metadata_for_plugin(
    plugins_dir: &Path,
    plugin: &plugin_engine::manifest::LoadedPlugin,
) -> Option<hub::install::InstallMetadata> {
    let dir_name = plugin.plugin_dir.file_name()?.to_str()?;
    hub::install::read_install_metadata(plugins_dir, dir_name)
        .filter(|metadata| metadata.plugin_id == plugin.manifest.id)
}

/// Build the JS-facing PluginMeta list from the loaded Rust plugins.
/// Shared by `list_plugins` and `hub::reload_plugins_and_emit` so hot-reload
/// stays byte-identical to the initial probe.
pub fn plugins_to_meta(
    plugins: &[plugin_engine::manifest::LoadedPlugin],
    plugins_dir: &Path,
) -> Vec<PluginMeta> {
    plugins
        .iter()
        .map(|plugin| {
            let metadata = read_install_metadata_for_plugin(plugins_dir, plugin);
            let mut candidates: Vec<_> = plugin
                .manifest
                .lines
                .iter()
                .filter(|line| line.line_type == "progress" && line.primary_order.is_some())
                .collect();
            candidates.sort_by_key(|line| line.primary_order.unwrap());
            let primary_candidates: Vec<String> =
                candidates.iter().map(|line| line.label.clone()).collect();

            let weekly_candidate =
                plugin_engine::manifest::weekly_candidate(&plugin.manifest.lines)
                    .map(str::to_string);

            PluginMeta {
                id: plugin.manifest.id.clone(),
                name: plugin.manifest.name.clone(),
                icon_url: plugin.icon_data_url.clone(),
                brand_color: plugin.manifest.brand_color.clone(),
                lines: plugin
                    .manifest
                    .lines
                    .iter()
                    .map(|line| ManifestLineDto {
                        line_type: line.line_type.clone(),
                        label: line.label.clone(),
                        scope: line.scope.clone(),
                    })
                    .collect(),
                links: plugin
                    .manifest
                    .links
                    .iter()
                    .map(|link| PluginLinkDto {
                        label: link.label.clone(),
                        url: link.url.clone(),
                    })
                    .collect(),
                primary_candidates,
                weekly_candidate,
                source_label: metadata
                    .as_ref()
                    .filter(|m| !m.source_label.is_empty())
                    .map(|m| m.source_label.clone()),
                version: metadata.map(|m| m.installed_version),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::plugins_to_meta;
    use crate::hub::install::{INSTALL_METADATA_SCHEMA_VERSION, InstallMetadata};
    use crate::plugin_engine::manifest::{LoadedPlugin, PluginManifest};
    use std::path::{Path, PathBuf};

    #[test]
    fn plugins_to_meta_uses_loaded_plugin_dir_for_install_metadata() {
        let plugins_dir = tempdir("plugins-to-meta-metadata-source");
        let loaded_dir = plugins_dir.join("claude__source-a");
        std::fs::create_dir_all(&loaded_dir).unwrap();
        crate::hub::install::write_install_metadata(
            &plugins_dir,
            "orphan-metadata",
            &install_metadata("claude", "Orphan Source", "9.9.9"),
        )
        .unwrap();

        let plugin = loaded_plugin("claude", loaded_dir);
        let meta = plugins_to_meta(&[plugin], &plugins_dir);

        assert_eq!(meta.len(), 1);
        assert_eq!(meta[0].source_label, None);
        assert_eq!(meta[0].version, None);

        crate::hub::install::write_install_metadata(
            &plugins_dir,
            "claude__source-a",
            &install_metadata("claude", "Loaded Source", "1.2.3"),
        )
        .unwrap();

        let plugin = loaded_plugin("claude", plugins_dir.join("claude__source-a"));
        let meta = plugins_to_meta(&[plugin], &plugins_dir);

        assert_eq!(meta[0].source_label.as_deref(), Some("Loaded Source"));
        assert_eq!(meta[0].version.as_deref(), Some("1.2.3"));
    }

    fn loaded_plugin(id: &str, plugin_dir: PathBuf) -> LoadedPlugin {
        LoadedPlugin {
            manifest: PluginManifest {
                schema_version: 1,
                id: id.to_string(),
                name: id.to_string(),
                version: "1.0.0".to_string(),
                entry: "plugin.js".to_string(),
                icon: "icon.svg".to_string(),
                brand_color: None,
                lines: Vec::new(),
                links: Vec::new(),
            },
            plugin_dir,
            entry_script: String::new(),
            icon_data_url: String::new(),
        }
    }

    fn install_metadata(plugin_id: &str, source_label: &str, version: &str) -> InstallMetadata {
        InstallMetadata {
            schema_version: INSTALL_METADATA_SCHEMA_VERSION,
            source_id: "source".to_string(),
            source_url: "https://example.com/source.git".to_string(),
            source_label: source_label.to_string(),
            source_kind: None,
            source_ref: None,
            source_commit_sha: None,
            plugin_id: plugin_id.to_string(),
            installed_version: version.to_string(),
            package_hash: "sha256:test".to_string(),
            installed_at: 1,
        }
    }

    fn tempdir(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "openusage-lib-{}-{}-{}",
            label,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        if path.exists() {
            remove_dir_all_best_effort(&path);
        }
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn remove_dir_all_best_effort(path: &Path) {
        if path.exists() {
            let _ = std::fs::remove_dir_all(path);
        }
    }
}
