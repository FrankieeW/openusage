use super::*;

#[test]
fn ccusage_path_entries_with_home_and_existing_path_preserves_order() {
    let home = std::path::PathBuf::from("/tmp/openusage-home");
    let existing = std::env::join_paths([
        std::path::PathBuf::from("/usr/bin"),
        std::path::PathBuf::from("/bin"),
    ])
    .expect("join existing path");

    let entries = ccusage_path_entries_with(Some(home.as_path()), Some(existing.as_os_str()));
    assert_eq!(
        entries,
        vec![
            home.join(".bun/bin"),
            home.join(".nvm/current/bin"),
            home.join(".local/bin"),
            std::path::PathBuf::from("/opt/homebrew/bin"),
            std::path::PathBuf::from("/usr/local/bin"),
            std::path::PathBuf::from("/usr/bin"),
            std::path::PathBuf::from("/bin"),
        ]
    );
}

#[test]
fn ccusage_path_entries_with_deduplicates_prefix_and_existing_entries() {
    let existing = std::env::join_paths([
        std::path::PathBuf::from("/usr/local/bin"),
        std::path::PathBuf::from("/custom/bin"),
        std::path::PathBuf::from("/custom/bin"),
        std::path::PathBuf::from("/opt/homebrew/bin"),
    ])
    .expect("join existing path");

    let entries = ccusage_path_entries_with(None, Some(existing.as_os_str()));
    assert_eq!(
        entries,
        vec![
            std::path::PathBuf::from("/opt/homebrew/bin"),
            std::path::PathBuf::from("/usr/local/bin"),
            std::path::PathBuf::from("/custom/bin"),
        ]
    );
}

#[test]
fn ccusage_enriched_path_with_uses_defaults_without_home_or_existing_path() {
    let enriched = ccusage_enriched_path_with(None, None).expect("enriched path");
    let entries: Vec<std::path::PathBuf> = std::env::split_paths(enriched.as_os_str()).collect();
    assert_eq!(
        entries,
        vec![
            std::path::PathBuf::from("/opt/homebrew/bin"),
            std::path::PathBuf::from("/usr/local/bin"),
        ]
    );
}

#[test]
fn ccusage_enriched_path_with_preserves_entries_after_join_and_split() {
    let home = std::path::PathBuf::from("/tmp/openusage-home");
    let existing = std::env::join_paths([
        std::path::PathBuf::from("/usr/bin"),
        std::path::PathBuf::from("/bin"),
    ])
    .expect("join existing path");

    let enriched =
        ccusage_enriched_path_with(Some(home.as_path()), Some(existing.as_os_str())).expect("path");
    let entries: Vec<std::path::PathBuf> = std::env::split_paths(enriched.as_os_str()).collect();

    assert_eq!(
        entries,
        vec![
            home.join(".bun/bin"),
            home.join(".nvm/current/bin"),
            home.join(".local/bin"),
            std::path::PathBuf::from("/opt/homebrew/bin"),
            std::path::PathBuf::from("/usr/local/bin"),
            std::path::PathBuf::from("/usr/bin"),
            std::path::PathBuf::from("/bin"),
        ]
    );
}

#[test]
fn nvm_default_bin_path_resolves_version_with_v_prefix() {
    let home = std::env::temp_dir().join("openusage-test-nvm-v-prefix");
    let alias_dir = home.join(".nvm/alias");
    std::fs::create_dir_all(&alias_dir).expect("create alias dir");
    std::fs::write(alias_dir.join("default"), "v22.16.0").expect("write alias");
    let result = nvm_default_bin_path(&home);
    let _ = std::fs::remove_dir_all(&home);
    assert_eq!(result, Some(home.join(".nvm/versions/node/v22.16.0/bin")));
}

#[test]
fn nvm_default_bin_path_resolves_version_without_v_prefix() {
    let home = std::env::temp_dir().join("openusage-test-nvm-no-v-prefix");
    let alias_dir = home.join(".nvm/alias");
    std::fs::create_dir_all(&alias_dir).expect("create alias dir");
    std::fs::write(alias_dir.join("default"), "22.16.0").expect("write alias");
    let result = nvm_default_bin_path(&home);
    let _ = std::fs::remove_dir_all(&home);
    assert_eq!(result, Some(home.join(".nvm/versions/node/v22.16.0/bin")));
}

#[test]
fn nvm_default_bin_path_returns_none_when_alias_missing() {
    let home = std::env::temp_dir().join("openusage-test-nvm-no-alias");
    let _ = std::fs::remove_dir_all(&home);
    let result = nvm_default_bin_path(&home);
    assert_eq!(result, None);
}

#[test]
fn ccusage_path_entries_with_includes_nvm_default_version() {
    let home = std::env::temp_dir().join("openusage-test-nvm-entries");
    let alias_dir = home.join(".nvm/alias");
    std::fs::create_dir_all(&alias_dir).expect("create alias dir");
    std::fs::write(alias_dir.join("default"), "22.16.0").expect("write alias");
    let entries = ccusage_path_entries_with(Some(&home), None);
    let _ = std::fs::remove_dir_all(&home);
    assert!(
        entries.contains(&home.join(".nvm/versions/node/v22.16.0/bin")),
        "expected nvm default version bin in entries"
    );
}

#[test]
fn configure_ccusage_command_sets_path_override() {
    let mut command = std::process::Command::new("echo");
    let args = vec!["daily".to_string(), "--json".to_string()];
    let path = std::env::join_paths([
        std::path::PathBuf::from("/tmp/bin"),
        std::path::PathBuf::from("/usr/bin"),
    ])
    .expect("join path override");

    configure_ccusage_command(&mut command, &args, Some(path.as_os_str()));

    let configured_args: Vec<String> = command
        .get_args()
        .map(|arg| arg.to_string_lossy().to_string())
        .collect();
    assert_eq!(configured_args, args);

    let configured_path = command
        .get_envs()
        .find(|(key, _)| *key == std::ffi::OsStr::new("PATH"))
        .and_then(|(_, value)| value.map(std::borrow::ToOwned::to_owned));
    assert_eq!(configured_path.as_deref(), Some(path.as_os_str()));
}

#[test]
fn configure_ccusage_command_skips_path_override_when_absent() {
    let mut command = std::process::Command::new("echo");
    let args = vec!["daily".to_string()];

    configure_ccusage_command(&mut command, &args, None);

    let has_path_override = command
        .get_envs()
        .any(|(key, _)| key == std::ffi::OsStr::new("PATH"));
    assert!(
        !has_path_override,
        "PATH should only be set when an override exists"
    );
}
