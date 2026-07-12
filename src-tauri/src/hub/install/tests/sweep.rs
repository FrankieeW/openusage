use std::fs;
use std::path::{Path, PathBuf};

use crate::hub::install::{
    INSTALL_METADATA_SCHEMA_VERSION, InstallMetadata, OrphanReport, startup_sweep,
    write_install_metadata,
};
use crate::hub::registry::{CURRENT_VERSION, RegistryFile, Source};
use crate::hub::source::SourceKind;

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
        branch: None,
        plugin_filter: None,
        auto_check: false,
    }
}

fn write_metadata(install_dir: &Path, plugin_id: &str, source_id: &str) {
    let metadata = InstallMetadata {
        schema_version: INSTALL_METADATA_SCHEMA_VERSION,
        source_id: source_id.into(),
        source_url: "https://github.com/foo/bar".into(),
        source_label: "".into(),
        source_kind: Some(SourceKind::Github),
        source_ref: Some("main".into()),
        source_commit_sha: Some("abc123".into()),
        plugin_id: plugin_id.into(),
        installed_version: "0.6.27".into(),
        package_hash: "sha256:fixture".into(),
        installed_at: 0,
    };
    write_install_metadata(install_dir, plugin_id, &metadata).unwrap();
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
    assert_eq!(
        report.removed_cache_dirs,
        vec!["removed-source".to_string()]
    );
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
    assert_eq!(report.orphan_source_plugins, vec!["orphan".to_string()]);
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
