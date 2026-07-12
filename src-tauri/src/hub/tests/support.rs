use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn tempdir(label: &str) -> PathBuf {
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

pub(super) fn write_fake_plugin(parent: &Path, id: &str, version: &str) {
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
    fs::write(dir.join("plugin.js"), "globalThis.__openusage_plugin={};").unwrap();
    fs::write(dir.join("icon.svg"), "<svg/>").unwrap();
}
