use super::*;

#[test]
fn ls_command_matches_language_server_variants() {
    assert!(ls_command_matches_process(
        "/Applications/Antigravity IDE.app/Contents/Resources/language_server_macos_arm --app_data_dir antigravity-ide",
        "language_server"
    ));
    assert!(ls_command_matches_process(
        "/tmp/language_server --app_data_dir antigravity-ide",
        "language_server"
    ));
}

#[test]
fn ls_command_matches_short_process_names_exactly() {
    assert!(ls_command_matches_process(
        "/opt/homebrew/bin/agy --some-flag",
        "agy"
    ));
    assert!(ls_command_matches_process(
        "/Applications/Antigravity IDE.app/Contents/Resources/agy --some-flag",
        "agy"
    ));
    assert!(ls_command_matches_process(
        "\"/Applications/Antigravity IDE.app/Contents/Resources/agy\" --some-flag",
        "agy"
    ));
    assert!(!ls_command_matches_process(
        "/opt/homebrew/bin/not-agy-helper --some-flag agy",
        "agy"
    ));
}

#[test]
fn ls_marker_rank_prefers_exact_flags_over_path_fallback() {
    let markers = vec!["antigravity".to_string()];

    assert_eq!(
        ls_marker_rank(
            "/tmp/windsurf/language_server --ide_name antigravity",
            &markers
        ),
        Some(0)
    );
    assert_eq!(
        ls_marker_rank("/tmp/antigravity/language_server", &markers),
        Some(1)
    );
    assert_eq!(
        ls_marker_rank(
            "/tmp/antigravity/language_server --ide_name windsurf",
            &markers
        ),
        None
    );
}
