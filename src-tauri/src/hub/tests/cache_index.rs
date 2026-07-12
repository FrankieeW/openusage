use super::super::*;
use super::support::{tempdir, write_fake_plugin};

#[test]
fn cache_index_reuses_plugin_summary_when_commit_matches() {
    let cache = tempdir("cache-index-reuse");
    write_fake_plugin(&cache, "claude", "1.0.0");
    let plugins = tempdir("cache-index-reuse-plugins");
    let installed = InstalledLookup::new();

    let (first, _) = discover_cache_plugins_with_index(
        &cache,
        "src-1",
        &plugins,
        &installed,
        None,
        Some("abc123".to_string()),
    );
    assert_eq!(first[0].available_version, "1.0.0");

    write_fake_plugin(&cache, "claude", "2.0.0");
    let (second, _) = discover_cache_plugins_with_index(
        &cache,
        "src-1",
        &plugins,
        &installed,
        None,
        Some("abc123".to_string()),
    );

    assert_eq!(second[0].available_version, "1.0.0");
    assert!(cache.join("hub-cache-index.json").exists());
}

#[test]
fn cache_index_refreshes_plugin_summary_when_commit_changes() {
    let cache = tempdir("cache-index-refresh");
    write_fake_plugin(&cache, "claude", "1.0.0");
    let plugins = tempdir("cache-index-refresh-plugins");
    let installed = InstalledLookup::new();

    let (first, _) = discover_cache_plugins_with_index(
        &cache,
        "src-1",
        &plugins,
        &installed,
        None,
        Some("abc123".to_string()),
    );
    assert_eq!(first[0].available_version, "1.0.0");

    write_fake_plugin(&cache, "claude", "2.0.0");
    let (second, _) = discover_cache_plugins_with_index(
        &cache,
        "src-1",
        &plugins,
        &installed,
        None,
        Some("def456".to_string()),
    );

    assert_eq!(second[0].available_version, "2.0.0");
}
