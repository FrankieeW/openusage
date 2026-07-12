use super::super::*;
use super::support::{tempdir, write_fake_plugin};
use std::fs;

#[test]
fn discover_marks_installed_and_update_available() {
    let cache = tempdir("cache-upd");
    write_fake_plugin(&cache, "claude", "0.7.0");
    let plugins = tempdir("plugins-upd");
    write_fake_plugin(&plugins, "claude", "0.6.27"); // older version installed
    let installed_hash = install::package_hash(&plugins.join("plugins").join("claude")).unwrap();
    let mut lookup = InstalledLookup::new();
    lookup.insert(
        "claude".into(),
        InstalledLookupEntry {
            source_id: "src-1".into(),
            source_url: "https://github.com/foo/bar".into(),
            version: "0.6.27".into(),
            package_hash: installed_hash,
        },
    );
    let (available, _skipped) = discover_cache_plugins(&cache, "src-1", &plugins, &lookup, None);
    let claude = available.iter().find(|p| p.id == "claude").unwrap();
    assert!(claude.installed);
    assert_eq!(claude.installed_source_id.as_deref(), Some("src-1"));
    assert_eq!(claude.installed_version.as_deref(), Some("0.6.27"));
    assert_eq!(claude.available_version, "0.7.0");
    assert!(claude.update_available);
    assert_eq!(claude.package_status, PackageStatus::UpdateAvailable);
    assert!(!claude.unmanaged);
}

#[test]
fn discover_marks_same_version_different_hash_from_same_source_as_source_changed() {
    let cache = tempdir("cache-source-changed");
    write_fake_plugin(&cache, "claude", "0.6.27");
    let plugins = tempdir("plugins-source-changed");
    let candidate_hash = install::package_hash(&cache.join("plugins").join("claude")).unwrap();
    let mut lookup = InstalledLookup::new();
    lookup.insert(
        "claude".into(),
        InstalledLookupEntry {
            source_id: "src-1".into(),
            source_url: "https://github.com/foo/bar".into(),
            version: "0.6.27".into(),
            package_hash: "sha256:previous".into(),
        },
    );

    let (available, _skipped) = discover_cache_plugins(&cache, "src-1", &plugins, &lookup, None);
    let claude = available.iter().find(|p| p.id == "claude").unwrap();

    assert!(claude.installed);
    assert!(!claude.update_available);
    assert_eq!(claude.package_hash, candidate_hash);
    assert_eq!(claude.package_status, PackageStatus::SourceChanged);
}

#[test]
fn discover_marks_installed_newer_than_source_without_update() {
    let cache = tempdir("cache-newer-installed");
    write_fake_plugin(&cache, "claude", "1.0.0");
    let plugins = tempdir("plugins-newer-installed");
    let mut lookup = InstalledLookup::new();
    lookup.insert(
        "claude".into(),
        InstalledLookupEntry {
            source_id: "src-1".into(),
            source_url: "https://github.com/foo/bar".into(),
            version: "2.0.0".into(),
            package_hash: "sha256:installed".into(),
        },
    );

    let (available, _skipped) = discover_cache_plugins(&cache, "src-1", &plugins, &lookup, None);
    let claude = available.iter().find(|p| p.id == "claude").unwrap();

    assert!(claude.installed);
    assert!(!claude.update_available);
    assert_eq!(
        claude.package_status,
        PackageStatus::InstalledNewerThanSource
    );
}

#[test]
fn discover_marks_same_package_from_other_source() {
    let cache = tempdir("cache-same-package-other-source");
    write_fake_plugin(&cache, "claude", "0.6.27");
    let plugins = tempdir("plugins-same-package-other-source");
    let candidate_hash = install::package_hash(&cache.join("plugins").join("claude")).unwrap();
    let mut lookup = InstalledLookup::new();
    lookup.insert(
        "claude".to_string(),
        InstalledLookupEntry {
            source_id: "src-1".to_string(),
            source_url: "https://github.com/example/a".to_string(),
            version: "0.6.27".to_string(),
            package_hash: candidate_hash,
        },
    );

    let (available, _skipped) = discover_cache_plugins(&cache, "src-2", &plugins, &lookup, None);
    let claude = available.iter().find(|p| p.id == "claude").unwrap();

    assert!(!claude.installed);
    assert_eq!(claude.installed_source_id.as_deref(), Some("src-1"));
    assert_eq!(claude.installed_version.as_deref(), Some("0.6.27"));
    assert_eq!(
        claude.package_status,
        PackageStatus::SamePackageFromOtherSource
    );
    assert!(!claude.update_available);
}

#[test]
fn discover_marks_different_package_same_plugin_id_from_other_source() {
    let cache = tempdir("cache-different-package-other-source");
    write_fake_plugin(&cache, "claude", "0.6.27");
    let plugins = tempdir("plugins-different-package-other-source");
    let mut lookup = InstalledLookup::new();
    lookup.insert(
        "claude".to_string(),
        InstalledLookupEntry {
            source_id: "src-1".to_string(),
            source_url: "https://github.com/example/a".to_string(),
            version: "0.6.27".to_string(),
            package_hash: "sha256:other".to_string(),
        },
    );

    let (available, _skipped) = discover_cache_plugins(&cache, "src-2", &plugins, &lookup, None);
    let claude = available.iter().find(|p| p.id == "claude").unwrap();

    assert!(!claude.installed);
    assert_eq!(claude.installed_source_id.as_deref(), Some("src-1"));
    assert_eq!(claude.installed_version.as_deref(), Some("0.6.27"));
    assert_eq!(
        claude.package_status,
        PackageStatus::DifferentPackageSamePluginId
    );
    assert!(!claude.update_available);
}

#[test]
fn discover_does_not_mark_installed_when_id_exists_in_another_source() {
    // Multi-source model: plugin id "claude" is installed from src-1, but we are
    // browsing src-2. The entry under src-2 must NOT show as installed or unmanaged.
    let cache = tempdir("cache-multisource");
    write_fake_plugin(&cache, "claude", "0.7.0");
    let plugins = tempdir("plugins-multisource");
    let installed_dir = plugins.join("claude");
    fs::create_dir_all(&installed_dir).unwrap();
    fs::write(
            installed_dir.join("plugin.json"),
            r##"{"schemaVersion":1,"id":"claude","name":"Claude","version":"0.6.27","entry":"plugin.js","icon":"icon.svg","brandColor":"#000000","lines":[]}"##,
        )
        .unwrap();
    // Simulate "installed from src-1" via the lookup map directly. The metadata
    // file format is verified by other tests; we just need the in-memory lookup.
    let mut lookup = InstalledLookup::new();
    lookup.insert(
        "claude".to_string(),
        InstalledLookupEntry {
            source_id: "src-1".to_string(),
            source_url: "https://github.com/example/a".to_string(),
            version: "0.6.27".to_string(),
            package_hash: "sha256:installed".to_string(),
        },
    );
    // Browse the OTHER source (src-2). Same id, different source.
    let (available, _skipped) = discover_cache_plugins(&cache, "src-2", &plugins, &lookup, None);
    let claude = available.iter().find(|p| p.id == "claude").unwrap();
    assert!(
        !claude.installed,
        "must not show as installed from a different source"
    );
    assert!(
        !claude.unmanaged,
        "must not show as unmanaged (it's managed by src-1)"
    );
    assert_eq!(claude.installed_source_id.as_deref(), Some("src-1"));
    assert_eq!(claude.installed_version.as_deref(), Some("0.6.27"));
    assert_eq!(
        claude.package_status,
        PackageStatus::DifferentPackageSamePluginId
    );
    assert!(!claude.update_available);
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
    let (available, _skipped) = discover_cache_plugins(&cache, "src-1", &plugins, &lookup, None);
    let claude = available.iter().find(|p| p.id == "claude").unwrap();
    assert!(claude.installed);
    assert!(claude.unmanaged);
    assert!(claude.installed_source_id.is_none());
}
