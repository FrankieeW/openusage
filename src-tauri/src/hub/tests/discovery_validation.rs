use super::super::*;
use super::support::{tempdir, write_fake_plugin};
use std::fs;

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
    let (available, skipped) =
        discover_cache_plugins(&cache, "src-1", &plugins, &InstalledLookup::new(), None);
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
    let (available, skipped) =
        discover_cache_plugins(&cache, "src-1", &plugins, &InstalledLookup::new(), None);
    assert!(available.is_empty());
    assert_eq!(skipped.len(), 1);
    assert!(skipped[0].reason.contains("schemaVersion"));
}

#[test]
fn discover_filters_to_plugin_filter_list() {
    let cache = tempdir("cache-filter");
    write_fake_plugin(&cache, "claude", "0.6.27");
    write_fake_plugin(&cache, "codex", "0.6.27");
    let plugins = tempdir("plugins-filter");
    let lookup = InstalledLookup::new();
    let filter = vec!["claude".to_string()];
    let (available, skipped) =
        discover_cache_plugins(&cache, "src-1", &plugins, &lookup, Some(&filter));
    assert_eq!(skipped.len(), 0);
    assert_eq!(available.len(), 1);
    assert_eq!(available[0].id, "claude");
}

#[test]
fn discover_shows_all_when_filter_is_empty() {
    let cache = tempdir("cache-emptyfilter");
    write_fake_plugin(&cache, "claude", "0.6.27");
    write_fake_plugin(&cache, "codex", "0.6.27");
    let plugins = tempdir("plugins-emptyfilter");
    let lookup = InstalledLookup::new();
    let empty: Vec<String> = vec![];
    let (available, _skipped) =
        discover_cache_plugins(&cache, "src-1", &plugins, &lookup, Some(&empty));
    assert_eq!(available.len(), 2);
}

#[test]
fn discover_filters_skip_nonexistent_ids() {
    let cache = tempdir("cache-missing");
    write_fake_plugin(&cache, "claude", "0.6.27");
    let plugins = tempdir("plugins-missing");
    let lookup = InstalledLookup::new();
    let filter = vec!["openrouter".to_string(), "claude".to_string()];
    let (available, _skipped) =
        discover_cache_plugins(&cache, "src-1", &plugins, &lookup, Some(&filter));
    assert_eq!(available.len(), 1);
    assert_eq!(available[0].id, "claude");
}

#[test]
fn source_snapshot_records_branch_counts_and_checked_time() {
    let source = Source {
        id: "src-1".to_string(),
        label: "Source".to_string(),
        url: "https://github.com/foo/bar".to_string(),
        kind: SourceKind::Github,
        branch: Some("feat/plugins".to_string()),
        plugin_filter: None,
        added_at: 1,
        last_refreshed_at: Some(2),
        auto_check: false,
    };

    let snapshot = source_snapshot(&source, Some("abcdef".to_string()), 3, 1, 1234);

    assert_eq!(snapshot.branch.as_deref(), Some("feat/plugins"));
    assert_eq!(snapshot.commit_sha.as_deref(), Some("abcdef"));
    assert_eq!(snapshot.checked_at, 1234);
    assert_eq!(snapshot.discovered_count, 3);
    assert_eq!(snapshot.skipped_count, 1);
}

#[test]
fn validate_source_health_rejects_source_without_valid_plugins() {
    let cache = tempdir("health-empty");
    fs::create_dir_all(cache.join("plugins")).unwrap();
    let plugins = tempdir("health-plugins");

    let err = validate_source_health(&cache, "src-1", &plugins, None).unwrap_err();

    assert_eq!(err.code, HubErrorCode::SourceHealthFailed);
    assert!(err.message.contains("no valid plugins"));
}

#[test]
fn validate_source_health_returns_invalid_plugin_reasons() {
    let cache = tempdir("health-invalid");
    let invalid = cache.join("plugins").join("broken");
    fs::create_dir_all(&invalid).unwrap();
    fs::write(
            invalid.join("plugin.json"),
            r##"{"schemaVersion":99,"id":"broken","name":"Broken","version":"1.0.0","entry":"plugin.js","icon":"icon.svg","brandColor":"#000000","lines":[]}"##,
        )
        .unwrap();
    let plugins = tempdir("health-invalid-plugins");

    let err = validate_source_health(&cache, "src-1", &plugins, None).unwrap_err();

    assert_eq!(err.code, HubErrorCode::SourceHealthFailed);
    let skipped = err
        .context
        .as_ref()
        .and_then(|context| context.get("skipped"))
        .and_then(|value| value.as_array())
        .expect("skipped context");
    assert_eq!(skipped.len(), 1);
}
