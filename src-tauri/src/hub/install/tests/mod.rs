mod filesystem;
mod package;
mod sweep;
mod validation;

use std::fs;
use std::path::{Path, PathBuf};

use crate::hub::install::hub_trash_dir;

pub(super) fn tempdir(label: &str) -> PathBuf {
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

pub(super) fn write_plugin(parent: &Path, id: &str, version: &str) -> PathBuf {
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

pub(super) fn trash_entries_for(install_dir: &Path, plugin_id: &str) -> Vec<PathBuf> {
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
