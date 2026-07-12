use super::super::*;
use super::support::{tempdir, write_fake_plugin};
use std::fs;

#[test]
fn discover_returns_empty_when_no_plugins_dir() {
    let cache = tempdir("cache-empty");
    let plugins = tempdir("plugins-empty");
    let lookup = InstalledLookup::new();
    let (available, skipped) = discover_cache_plugins(&cache, "src-1", &plugins, &lookup, None);
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
    let (available, skipped) = discover_cache_plugins(&cache, "src-1", &plugins, &lookup, None);
    assert_eq!(available.len(), 2);
    assert_eq!(available[0].id, "claude");
    assert_eq!(available[1].id, "codex");
    assert!(available.iter().all(|p| p.icon_data_url.is_some()));
    assert!(
        available
            .iter()
            .all(|p| p.brand_color.as_deref() == Some("#FF00FF"))
    );
    assert!(skipped.is_empty());
}

#[test]
fn discover_preserves_manifest_updated_at() {
    let cache = tempdir("cache-updated-at");
    let dir = cache.join("plugins").join("claude");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("plugin.json"),
        r##"{
  "schemaVersion": 1,
  "id": "claude",
  "name": "Claude",
  "version": "0.6.27",
  "updatedAt": 1781654400000,
  "entry": "plugin.js",
  "icon": "icon.svg",
  "brandColor": "#000000",
  "lines": []
}"##,
    )
    .unwrap();
    fs::write(dir.join("plugin.js"), "globalThis.__openusage_plugin={};").unwrap();
    fs::write(dir.join("icon.svg"), "<svg/>").unwrap();

    let plugins = tempdir("plugins-updated-at");
    let (available, skipped) =
        discover_cache_plugins(&cache, "src-1", &plugins, &InstalledLookup::new(), None);
    assert!(skipped.is_empty());
    let json = serde_json::to_value(&available[0]).unwrap();

    assert_eq!(json["updatedAt"], serde_json::json!(1781654400000_i64));
}
