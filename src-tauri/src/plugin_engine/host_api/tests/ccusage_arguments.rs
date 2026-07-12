use super::*;

#[test]
fn ccusage_runner_order_matches_expected_priority() {
    assert_eq!(
        ccusage_runner_order(),
        [
            CcusageRunnerKind::Bunx,
            CcusageRunnerKind::PnpmDlx,
            CcusageRunnerKind::YarnDlx,
            CcusageRunnerKind::NpmExec,
            CcusageRunnerKind::Npx
        ]
    );
}

#[test]
fn ccusage_runner_args_include_expected_non_interactive_flags() {
    let opts = CcusageQueryOpts {
        provider: None,
        since: Some("20260101".to_string()),
        until: Some("20260131".to_string()),
        home_path: None,
        claude_path: None,
    };
    let expected_ccusage_package = ccusage_package_spec();
    assert_eq!(expected_ccusage_package, "ccusage@20.0.2");
    let expected_npm_exec_package = format!("--package={expected_ccusage_package}");

    let bunx = ccusage_runner_args(
        CcusageRunnerKind::Bunx,
        &opts,
        CcusageProvider::Claude,
        CcusageCommandFlavor::Current,
    );
    assert_eq!(
        bunx,
        vec![
            "--silent",
            expected_ccusage_package.as_str(),
            "claude",
            "daily",
            "--json",
            "--order",
            "desc",
            "--since",
            "20260101",
            "--until",
            "20260131"
        ]
    );

    let pnpm = ccusage_runner_args(
        CcusageRunnerKind::PnpmDlx,
        &opts,
        CcusageProvider::Claude,
        CcusageCommandFlavor::Current,
    );
    assert_eq!(
        pnpm,
        vec![
            "-s",
            "dlx",
            expected_ccusage_package.as_str(),
            "claude",
            "daily",
            "--json",
            "--order",
            "desc",
            "--since",
            "20260101",
            "--until",
            "20260131"
        ]
    );

    let yarn = ccusage_runner_args(
        CcusageRunnerKind::YarnDlx,
        &opts,
        CcusageProvider::Claude,
        CcusageCommandFlavor::Current,
    );
    assert_eq!(
        yarn,
        vec![
            "dlx",
            "-q",
            expected_ccusage_package.as_str(),
            "claude",
            "daily",
            "--json",
            "--order",
            "desc",
            "--since",
            "20260101",
            "--until",
            "20260131"
        ]
    );

    let npm_exec = ccusage_runner_args(
        CcusageRunnerKind::NpmExec,
        &opts,
        CcusageProvider::Claude,
        CcusageCommandFlavor::Current,
    );
    assert_eq!(
        npm_exec,
        vec![
            "exec",
            "--yes",
            expected_npm_exec_package.as_str(),
            "--",
            "ccusage",
            "claude",
            "daily",
            "--json",
            "--order",
            "desc",
            "--since",
            "20260101",
            "--until",
            "20260131"
        ]
    );

    let npx = ccusage_runner_args(
        CcusageRunnerKind::Npx,
        &opts,
        CcusageProvider::Claude,
        CcusageCommandFlavor::Current,
    );
    assert_eq!(
        npx,
        vec![
            "--yes",
            expected_ccusage_package.as_str(),
            "claude",
            "daily",
            "--json",
            "--order",
            "desc",
            "--since",
            "20260101",
            "--until",
            "20260131"
        ]
    );
}

#[test]
fn ccusage_runner_args_codex_use_unified_package_and_bin() {
    let opts = CcusageQueryOpts {
        provider: Some("codex".to_string()),
        since: Some("20260101".to_string()),
        until: Some("20260131".to_string()),
        home_path: None,
        claude_path: None,
    };
    let expected_ccusage_package = ccusage_package_spec();
    let expected_npm_exec_package = format!("--package={expected_ccusage_package}");

    let bunx = ccusage_runner_args(
        CcusageRunnerKind::Bunx,
        &opts,
        CcusageProvider::Codex,
        CcusageCommandFlavor::Current,
    );
    assert_eq!(
        bunx,
        vec![
            "--silent",
            expected_ccusage_package.as_str(),
            "codex",
            "daily",
            "--json",
            "--order",
            "desc",
            "--since",
            "20260101",
            "--until",
            "20260131"
        ]
    );

    let npm_exec = ccusage_runner_args(
        CcusageRunnerKind::NpmExec,
        &opts,
        CcusageProvider::Codex,
        CcusageCommandFlavor::Current,
    );
    assert_eq!(
        npm_exec,
        vec![
            "exec",
            "--yes",
            expected_npm_exec_package.as_str(),
            "--",
            "ccusage",
            "codex",
            "daily",
            "--json",
            "--order",
            "desc",
            "--since",
            "20260101",
            "--until",
            "20260131"
        ]
    );

    let npx = ccusage_runner_args(
        CcusageRunnerKind::Npx,
        &opts,
        CcusageProvider::Codex,
        CcusageCommandFlavor::Current,
    );
    assert_eq!(
        npx,
        vec![
            "--yes",
            expected_ccusage_package.as_str(),
            "codex",
            "daily",
            "--json",
            "--order",
            "desc",
            "--since",
            "20260101",
            "--until",
            "20260131"
        ]
    );
}

#[test]
fn ccusage_runner_args_legacy_fallback_uses_release_age_safe_packages() {
    let opts = CcusageQueryOpts {
        provider: None,
        since: Some("20260101".to_string()),
        until: Some("20260131".to_string()),
        home_path: None,
        claude_path: None,
    };

    let claude = ccusage_runner_args(
        CcusageRunnerKind::Bunx,
        &opts,
        CcusageProvider::Claude,
        CcusageCommandFlavor::Legacy,
    );
    assert_eq!(
        claude,
        vec![
            "--silent",
            "ccusage@18.0.11",
            "daily",
            "--json",
            "--order",
            "desc",
            "--since",
            "20260101",
            "--until",
            "20260131"
        ]
    );

    let codex_npm = ccusage_runner_args(
        CcusageRunnerKind::NpmExec,
        &opts,
        CcusageProvider::Codex,
        CcusageCommandFlavor::Legacy,
    );
    assert_eq!(
        codex_npm,
        vec![
            "exec",
            "--yes",
            "--package=@ccusage/codex@18.0.11",
            "--",
            "ccusage-codex",
            "daily",
            "--json",
            "--order",
            "desc",
            "--since",
            "20260101",
            "--until",
            "20260131"
        ]
    );
}

#[test]
fn resolve_ccusage_provider_prefers_explicit_opt_then_plugin_id() {
    let opts_explicit = CcusageQueryOpts {
        provider: Some("codex".to_string()),
        since: None,
        until: None,
        home_path: None,
        claude_path: None,
    };
    assert_eq!(
        resolve_ccusage_provider(&opts_explicit, "claude"),
        CcusageProvider::Codex
    );

    let opts_empty = CcusageQueryOpts::default();
    assert_eq!(
        resolve_ccusage_provider(&opts_empty, "codex"),
        CcusageProvider::Codex
    );
    assert_eq!(
        resolve_ccusage_provider(&opts_empty, "claude"),
        CcusageProvider::Claude
    );
    assert_eq!(
        resolve_ccusage_provider(&opts_empty, "unknown-provider"),
        CcusageProvider::Claude
    );
}

#[test]
fn ccusage_home_override_supports_home_path_and_claude_compat() {
    let with_home = CcusageQueryOpts {
        provider: None,
        since: None,
        until: None,
        home_path: Some("/tmp/shared-home".to_string()),
        claude_path: Some("/tmp/claude-home".to_string()),
    };
    assert_eq!(
        ccusage_home_override(&with_home, CcusageProvider::Claude),
        Some("/tmp/shared-home")
    );
    assert_eq!(
        ccusage_home_override(&with_home, CcusageProvider::Codex),
        Some("/tmp/shared-home")
    );

    let claude_compat = CcusageQueryOpts {
        provider: None,
        since: None,
        until: None,
        home_path: None,
        claude_path: Some("/tmp/legacy-claude-path".to_string()),
    };
    assert_eq!(
        ccusage_home_override(&claude_compat, CcusageProvider::Claude),
        Some("/tmp/legacy-claude-path")
    );
    assert_eq!(
        ccusage_home_override(&claude_compat, CcusageProvider::Codex),
        None
    );
}
