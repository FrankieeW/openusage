use super::super::*;
use super::support::{tempdir, write_fake_plugin};
use std::fs;

#[test]
fn local_cache_resync_replaces_the_complete_cached_snapshot() {
    // Given
    let root = tempdir("local-resync");
    let source = root.join("source");
    let cache = root.join("cache");
    write_fake_plugin(&source, "claude", "2.0.0");
    write_fake_plugin(&cache, "claude", "1.0.0");
    fs::write(cache.join("stale.txt"), "stale").unwrap();

    // When
    resync_local_cache(&source, &cache).unwrap();

    // Then
    let manifest = fs::read_to_string(cache.join("plugins/claude/plugin.json")).unwrap();
    assert!(manifest.contains("\"version\": \"2.0.0\""));
    assert!(!cache.join("stale.txt").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn local_cache_resync_preserves_existing_cache_when_copy_fails() {
    // Given
    let root = tempdir("local-resync-failure");
    let source = root.join("missing-source");
    let cache = root.join("cache");
    write_fake_plugin(&cache, "claude", "1.0.0");

    // When
    let result = resync_local_cache(&source, &cache);

    // Then
    assert!(result.is_err());
    let manifest = fs::read_to_string(cache.join("plugins/claude/plugin.json")).unwrap();
    assert!(manifest.contains("\"version\": \"1.0.0\""));
    fs::remove_dir_all(root).unwrap();
}
