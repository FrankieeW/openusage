use std::fs;
use std::path::Path;

use crate::hub::install::{
    INSTALL_METADATA_SCHEMA_VERSION, InstallError, InstallMetadata, check_conflict,
    validate_entry_within_dir, validate_id_match, write_install_metadata,
};

use super::{tempdir, write_plugin};

#[test]
fn validate_id_match_accepts_matching() {
    assert!(validate_id_match("claude", "claude").is_ok());
}

#[test]
fn validate_id_match_rejects_mismatch() {
    assert!(matches!(
        validate_id_match("legacy-name", "claude"),
        Err(InstallError::IdMismatch { .. })
    ));
}

#[test]
fn validate_entry_accepts_simple_relative() {
    let dir = tempdir("entry-exists");
    fs::write(dir.join("plugin.js"), "// entry").unwrap();
    assert!(validate_entry_within_dir(&dir, "plugin.js").is_ok());
}

#[test]
fn validate_entry_rejects_missing_file() {
    let dir = tempdir("entry-missing");
    assert_eq!(
        validate_entry_within_dir(&dir, "plugin.js"),
        Err(InstallError::EntryOutsidePluginDir),
    );
}

#[test]
fn validate_entry_rejects_parent_traversal() {
    assert_eq!(
        validate_entry_within_dir(Path::new("/tmp/x"), "../foo.js"),
        Err(InstallError::EntryOutsidePluginDir),
    );
}

#[test]
fn validate_entry_rejects_absolute() {
    assert_eq!(
        validate_entry_within_dir(Path::new("/tmp/x"), "/etc/passwd"),
        Err(InstallError::EntryOutsidePluginDir),
    );
}

#[test]
fn check_conflict_unmanaged_when_no_metadata() {
    let dst = tempdir("conflict-unmanaged");
    write_plugin(&dst, "claude", "0.6.27");
    // No metadata sidecar — should report unmanaged
    assert_eq!(
        check_conflict(&dst, "claude", "src-new"),
        Err(InstallError::ConflictUnmanaged),
    );
}

#[test]
fn check_conflict_when_metadata_matches_candidate() {
    let dst = tempdir("conflict-match");
    write_plugin(&dst, "claude", "0.6.27");
    let metadata = InstallMetadata {
        schema_version: INSTALL_METADATA_SCHEMA_VERSION,
        source_id: "src-existing".into(),
        source_url: "https://github.com/foo/bar".into(),
        source_label: "".into(),
        source_kind: Some(crate::hub::source::SourceKind::Github),
        source_ref: Some("main".into()),
        source_commit_sha: Some("abc123".into()),
        plugin_id: "claude".into(),
        installed_version: "0.6.27".into(),
        package_hash: "sha256:fixture".into(),
        installed_at: 0,
    };
    write_install_metadata(&dst, "claude", &metadata).unwrap();
    assert!(check_conflict(&dst, "claude", "src-existing").is_ok());
}

#[test]
fn check_conflict_when_metadata_differs() {
    let dst = tempdir("conflict-diff");
    write_plugin(&dst, "claude", "0.6.27");
    let metadata = InstallMetadata {
        schema_version: INSTALL_METADATA_SCHEMA_VERSION,
        source_id: "src-existing".into(),
        source_url: "https://github.com/foo/bar".into(),
        source_label: "".into(),
        source_kind: Some(crate::hub::source::SourceKind::Github),
        source_ref: Some("main".into()),
        source_commit_sha: Some("abc123".into()),
        plugin_id: "claude".into(),
        installed_version: "0.6.27".into(),
        package_hash: "sha256:fixture".into(),
        installed_at: 0,
    };
    write_install_metadata(&dst, "claude", &metadata).unwrap();
    assert_eq!(
        check_conflict(&dst, "claude", "src-new"),
        Err(InstallError::ConflictWithSource("src-existing".into())),
    );
}

#[test]
fn check_conflict_when_same_plugin_id_exists_in_source_scoped_dir() {
    let dst = tempdir("conflict-scoped-dir");
    write_plugin(&dst, "claude__source-a", "0.6.27");
    let metadata = InstallMetadata {
        schema_version: INSTALL_METADATA_SCHEMA_VERSION,
        source_id: "src-existing".into(),
        source_url: "https://github.com/foo/bar".into(),
        source_label: "".into(),
        source_kind: Some(crate::hub::source::SourceKind::Github),
        source_ref: Some("main".into()),
        source_commit_sha: Some("abc123".into()),
        plugin_id: "claude".into(),
        installed_version: "0.6.27".into(),
        package_hash: "sha256:fixture".into(),
        installed_at: 0,
    };
    write_install_metadata(&dst, "claude__source-a", &metadata).unwrap();

    assert_eq!(
        check_conflict(&dst, "claude__source-b", "src-new"),
        Err(InstallError::ConflictWithSource("src-existing".into())),
    );
}
