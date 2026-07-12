use std::fs;

use crate::hub::install::{
    INSTALL_METADATA_SCHEMA_VERSION, InstallMetadata, METADATA_FILENAME, package_hash,
    read_install_metadata, write_install_metadata,
};

use super::{tempdir, write_plugin};

#[test]
fn metadata_round_trip() {
    let dir = tempdir("meta");
    let metadata = InstallMetadata {
        schema_version: INSTALL_METADATA_SCHEMA_VERSION,
        source_id: "src-1".into(),
        source_url: "https://github.com/foo/bar".into(),
        source_label: "".into(),
        source_kind: Some(crate::hub::source::SourceKind::Github),
        source_ref: Some("main".into()),
        source_commit_sha: Some("abc123".into()),
        plugin_id: "claude".into(),
        installed_version: "0.6.27".into(),
        package_hash: "sha256:fixture".into(),
        installed_at: 1234567890,
    };
    write_install_metadata(&dir, "claude", &metadata).unwrap();
    let loaded = read_install_metadata(&dir, "claude").unwrap();
    assert_eq!(loaded, metadata);
    // Sidecar file is hidden-named and inside plugin dir
    assert!(dir.join("claude").join(METADATA_FILENAME).exists());
}

#[test]
fn legacy_metadata_defaults_missing_v2_fields() {
    let dir = tempdir("legacy-meta");
    fs::create_dir_all(dir.join("claude")).unwrap();
    fs::write(
        dir.join("claude").join(METADATA_FILENAME),
        r##"{
  "source_id": "src-1",
  "source_url": "https://github.com/foo/bar",
  "source_label": "Foo",
  "plugin_id": "claude",
  "installed_version": "0.6.27",
  "installed_at": 123
}"##,
    )
    .unwrap();

    let loaded = read_install_metadata(&dir, "claude").unwrap();

    assert_eq!(loaded.schema_version, 1);
    assert_eq!(loaded.package_hash, "");
    assert_eq!(loaded.source_kind, None);
    assert_eq!(loaded.source_ref, None);
    assert_eq!(loaded.source_commit_sha, None);
}

#[test]
fn package_hash_is_stable_and_ignores_install_metadata() {
    let root = tempdir("hash-stable");
    let plugin_dir = write_plugin(&root, "claude", "0.6.27");

    let before = package_hash(&plugin_dir).unwrap();
    fs::write(plugin_dir.join(METADATA_FILENAME), "{}").unwrap();
    let after = package_hash(&plugin_dir).unwrap();

    assert_eq!(before, after);
    assert!(before.starts_with("sha256:"));
}

#[test]
fn package_hash_changes_when_plugin_file_changes() {
    let root = tempdir("hash-change");
    let plugin_dir = write_plugin(&root, "claude", "0.6.27");

    let before = package_hash(&plugin_dir).unwrap();
    fs::write(plugin_dir.join("plugin.js"), "// changed").unwrap();
    let after = package_hash(&plugin_dir).unwrap();

    assert_ne!(before, after);
}

#[test]
fn package_hash_ignores_files_excluded_from_install_copy() {
    let root = tempdir("hash-copy-excludes");
    let plugin_dir = write_plugin(&root, "claude", "0.6.27");

    let before = package_hash(&plugin_dir).unwrap();
    fs::write(plugin_dir.join("plugin.test.ts"), "throw new Error('test')").unwrap();
    fs::write(
        plugin_dir.join("test-helpers.js"),
        "export const helper = true",
    )
    .unwrap();
    let after = package_hash(&plugin_dir).unwrap();

    assert_eq!(before, after);
}
