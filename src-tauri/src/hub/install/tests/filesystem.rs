use std::fs;

use crate::hub::install::{
    INSTALL_METADATA_SCHEMA_VERSION, InstallMetadata, copy_plugin_to_install_dir,
    read_install_metadata, remove_installed_plugin, switch_plugin_install_dir,
    switch_plugin_install_dir_with_metadata,
};

use super::{tempdir, trash_entries_for, write_plugin};

#[test]
fn copy_installs_plugin_and_writes_metadata() {
    let src = tempdir("src");
    let dst = tempdir("dst");
    let plugin_dir = write_plugin(&src, "claude", "0.6.27");

    copy_plugin_to_install_dir(&plugin_dir, &dst, "claude").unwrap();

    let installed = dst.join("claude");
    assert!(installed.join("plugin.json").exists());
    assert!(installed.join("plugin.js").exists());
    assert!(installed.join("icon.svg").exists());
}

#[test]
fn copy_keeps_existing_install_when_candidate_is_invalid() {
    let src = tempdir("invalid-src");
    let dst = tempdir("invalid-dst");
    let existing = write_plugin(&dst, "claude", "0.6.27");
    fs::write(existing.join("plugin.js"), "// existing").unwrap();

    let candidate = write_plugin(&src, "claude", "0.6.28");
    fs::write(
        candidate.join("plugin.json"),
        r##"{
  "schemaVersion": 1,
  "id": "wrong-id",
  "name": "Claude",
  "version": "0.6.28",
  "entry": "plugin.js",
  "icon": "icon.svg",
  "brandColor": "#000000",
  "lines": []
}"##,
    )
    .unwrap();

    assert!(copy_plugin_to_install_dir(&candidate, &dst, "claude").is_err());

    assert_eq!(
        fs::read_to_string(dst.join("claude").join("plugin.js")).unwrap(),
        "// existing"
    );
    assert!(trash_entries_for(&dst, "claude").is_empty());
}

#[test]
fn copy_replaces_existing_install_after_moving_old_dir_to_trash() {
    let src = tempdir("replace-src");
    let dst = tempdir("replace-dst");
    let existing = write_plugin(&dst, "claude", "0.6.27");
    fs::write(existing.join("plugin.js"), "// old").unwrap();
    let candidate = write_plugin(&src, "claude", "0.6.28");
    fs::write(candidate.join("plugin.js"), "// new").unwrap();

    copy_plugin_to_install_dir(&candidate, &dst, "claude").unwrap();

    assert_eq!(
        fs::read_to_string(dst.join("claude").join("plugin.js")).unwrap(),
        "// new"
    );
    let trashed = trash_entries_for(&dst, "claude");
    assert_eq!(trashed.len(), 1);
    assert_eq!(
        fs::read_to_string(trashed[0].join("plugin.js")).unwrap(),
        "// old"
    );
}

#[test]
fn remove_installed_plugin_moves_dir_to_trash() {
    let dst = tempdir("remove");
    write_plugin(&dst, "claude", "0.6.27");
    assert!(dst.join("claude").exists());
    remove_installed_plugin(&dst, "claude").unwrap();
    assert!(!dst.join("claude").exists());
    let trashed = trash_entries_for(&dst, "claude");
    assert_eq!(trashed.len(), 1);
    assert!(trashed[0].join("plugin.json").exists());
}

#[test]
fn switch_plugin_install_dir_moves_old_source_to_trash_and_installs_new_source_dir() {
    let src = tempdir("switch-src");
    let dst = tempdir("switch-dst");
    let old = write_plugin(&dst, "claude__old", "0.6.27");
    fs::write(old.join("plugin.js"), "// old").unwrap();
    let candidate = write_plugin(&src, "claude", "0.6.28");
    fs::write(candidate.join("plugin.js"), "// new").unwrap();

    switch_plugin_install_dir(&candidate, &dst, "claude__old", "claude__new").unwrap();

    assert!(!dst.join("claude__old").exists());
    assert_eq!(
        fs::read_to_string(dst.join("claude__new").join("plugin.js")).unwrap(),
        "// new"
    );
    let trashed = trash_entries_for(&dst, "claude__old");
    assert_eq!(trashed.len(), 1);
    assert_eq!(
        fs::read_to_string(trashed[0].join("plugin.js")).unwrap(),
        "// old"
    );
}

#[test]
fn switch_plugin_install_dir_with_metadata_installs_metadata_atomically() {
    let src = tempdir("switch-meta-src");
    let dst = tempdir("switch-meta-dst");
    let candidate = write_plugin(&src, "claude", "0.6.28");
    let metadata = InstallMetadata {
        schema_version: INSTALL_METADATA_SCHEMA_VERSION,
        source_id: "src-new".into(),
        source_url: "https://github.com/foo/bar".into(),
        source_label: "Foo".into(),
        source_kind: Some(crate::hub::source::SourceKind::Github),
        source_ref: Some("main".into()),
        source_commit_sha: Some("abc123".into()),
        plugin_id: "claude".into(),
        installed_version: "0.6.28".into(),
        package_hash: "sha256:new".into(),
        installed_at: 123,
    };

    switch_plugin_install_dir_with_metadata(&candidate, &dst, "claude", "claude", &metadata)
        .unwrap();

    assert_eq!(read_install_metadata(&dst, "claude").unwrap(), metadata);
}

#[test]
fn remove_installed_plugin_missing_is_ok() {
    let dst = tempdir("remove-missing");
    assert!(remove_installed_plugin(&dst, "claude").is_ok());
}
