use super::super::env_overrides::WHITELISTED_ENV_VARS;
use super::*;

#[test]
fn last_non_empty_trimmed_line_uses_final_value_when_stdout_is_noisy() {
    let stdout = "banner line\nanother message\n  sk-test-key-12345  \n";
    let value = last_non_empty_trimmed_line(stdout);
    assert_eq!(value.as_deref(), Some("sk-test-key-12345"));
}

#[test]
fn last_non_empty_trimmed_line_returns_none_for_empty_stdout() {
    let stdout = "  \n\n\t\n";
    let value = last_non_empty_trimmed_line(stdout);
    assert!(value.is_none());
}

#[test]
fn sanitize_env_value_strips_ansi_and_control_sequences() {
    let raw = "\u{1b}[?1000l\n  sk-test-key-12345\u{1b}[?2004h\r\n";
    let value = sanitize_env_value(raw);
    assert_eq!(value.as_deref(), Some("sk-test-key-12345"));
}

#[test]
fn extract_marked_value_ignores_noisy_shell_output() {
    let stdout = concat!(
        "startup banner\n",
        "\u{1b}[31mplugin failed\u{1b}[0m\n",
        "__OPENUSAGE_ENV_START__\n",
        "  sk-test-key-12345  \n",
        "__OPENUSAGE_ENV_END__\n",
        "\u{1b}[32muser@host\u{1b}[0m\n"
    );
    let value = extract_marked_value(stdout, "__OPENUSAGE_ENV_START__", "__OPENUSAGE_ENV_END__");
    assert_eq!(value.as_deref(), Some("sk-test-key-12345"));
}

#[test]
fn extract_marked_value_strips_inline_terminal_sequences_from_marked_value() {
    let stdout = concat!(
        "__OPENUSAGE_ENV_START__\n",
        "\u{1b}[?1000l\n",
        "  sk-test-key-12345\u{1b}[?2004h\r\n",
        "__OPENUSAGE_ENV_END__\n"
    );
    let value = extract_marked_value(stdout, "__OPENUSAGE_ENV_START__", "__OPENUSAGE_ENV_END__");
    assert_eq!(value.as_deref(), Some("sk-test-key-12345"));
}

#[test]
fn extract_marked_value_returns_none_when_marked_value_is_empty() {
    let stdout = "__OPENUSAGE_ENV_START__\n  \n__OPENUSAGE_ENV_END__\n";
    let value = extract_marked_value(stdout, "__OPENUSAGE_ENV_START__", "__OPENUSAGE_ENV_END__");
    assert!(value.is_none());
}

#[test]
fn parse_interactive_shell_env_output_does_not_fallback_to_end_marker_for_empty_value() {
    let stdout = "__OPENUSAGE_ENV_START__\n  \n__OPENUSAGE_ENV_END__\n";
    let value = parse_interactive_shell_env_output(
        stdout,
        "__OPENUSAGE_ENV_START__",
        "__OPENUSAGE_ENV_END__",
    );
    assert!(value.is_none());
}

#[test]
fn parse_interactive_shell_env_output_falls_back_without_markers() {
    let stdout = "\u{1b}[?1000l\n  sk-test-key-12345\u{1b}[?2004h\r\n";
    let value = parse_interactive_shell_env_output(
        stdout,
        "__OPENUSAGE_ENV_START__",
        "__OPENUSAGE_ENV_END__",
    );
    assert_eq!(value.as_deref(), Some("sk-test-key-12345"));
}

#[test]
fn env_api_respects_allowlist_in_host_and_js() {
    let claude_env_vars = [
        "CLAUDE_CONFIG_DIR",
        "CLAUDE_CODE_OAUTH_TOKEN",
        "USER_TYPE",
        "USE_STAGING_OAUTH",
        "USE_LOCAL_OAUTH",
        "CLAUDE_CODE_CUSTOM_OAUTH_URL",
        "CLAUDE_CODE_OAUTH_CLIENT_ID",
        "CLAUDE_LOCAL_OAUTH_API_BASE",
    ];

    for name in claude_env_vars {
        assert!(
            WHITELISTED_ENV_VARS.contains(&name),
            "{name} must be whitelisted for Claude auth compatibility"
        );
    }

    let rt = Runtime::new().expect("runtime");
    let ctx = Context::full(&rt).expect("context");
    ctx.with(|ctx| {
        let app_data = std::env::temp_dir();
        inject_host_api(&ctx, "test", &app_data, "0.0.0").expect("inject host api");
        let globals = ctx.globals();
        let probe_ctx: Object = globals.get("__openusage_ctx").expect("probe ctx");
        let host: Object = probe_ctx.get("host").expect("host");
        let env: Object = host.get("env").expect("env");
        let get: Function = env.get("get").expect("get");

        for name in WHITELISTED_ENV_VARS {
            let expected = resolve_env_value(name);
            let value: Option<String> = get.call((name.to_string(),)).expect("get whitelisted var");
            assert_eq!(value, expected, "{name} should match host env resolver");

            let js_expr = format!(r#"__openusage_ctx.host.env.get("{}")"#, name);
            let js_value: Option<String> = ctx.eval(js_expr).expect("js get whitelisted var");
            assert_eq!(
                js_value, expected,
                "{name} should match host env resolver from JS"
            );
        }

        let blocked: Option<String> = get
            .call(("__OPENUSAGE_TEST_NOT_WHITELISTED__".to_string(),))
            .expect("get blocked var");
        assert!(
            blocked.is_none(),
            "non-whitelisted vars must not be exposed"
        );

        let js_blocked: Option<String> = ctx
            .eval(r#"__openusage_ctx.host.env.get("__OPENUSAGE_TEST_NOT_WHITELISTED__")"#)
            .expect("js get blocked var");
        assert!(
            js_blocked.is_none(),
            "non-whitelisted vars must not be exposed from JS"
        );
    });
}

#[test]
fn env_api_prefers_process_env() {
    struct RestoreEnvVar {
        name: &'static str,
        old: Option<String>,
    }

    impl Drop for RestoreEnvVar {
        fn drop(&mut self) {
            if let Some(value) = self.old.take() {
                // SAFETY: tests serialize env changes via this guard; value is restored on drop.
                unsafe { std::env::set_var(self.name, value) };
            } else {
                // SAFETY: tests serialize env changes via this guard; var is restored/removed on drop.
                unsafe { std::env::remove_var(self.name) };
            }
        }
    }

    let name = "ZAI_API_KEY";
    let old = std::env::var(name).ok();
    let _restore = RestoreEnvVar { name, old };
    // SAFETY: this test restores the previous value in `Drop`.
    unsafe { std::env::set_var(name, "sk-process-env-test-1234567890") };

    let rt = Runtime::new().expect("runtime");
    let ctx = Context::full(&rt).expect("context");
    ctx.with(|ctx| {
        let app_data = std::env::temp_dir();
        inject_host_api(&ctx, "test", &app_data, "0.0.0").expect("inject host api");
        let globals = ctx.globals();
        let probe_ctx: Object = globals.get("__openusage_ctx").expect("probe ctx");
        let host: Object = probe_ctx.get("host").expect("host");
        let env: Object = host.get("env").expect("env");
        let get: Function = env.get("get").expect("get");

        let value: Option<String> = get.call((name.to_string(),)).expect("get");
        assert_eq!(
            value.as_deref(),
            Some("sk-process-env-test-1234567890"),
            "process env should be preferred over shell lookup"
        );

        let js_value: Option<String> = ctx
            .eval(r#"__openusage_ctx.host.env.get("ZAI_API_KEY")"#)
            .expect("js get");
        assert_eq!(
            js_value.as_deref(),
            Some("sk-process-env-test-1234567890"),
            "process env should be preferred from JS"
        );
    });
}

#[test]
fn override_literal_is_returned_and_bypasses_whitelist() {
    let mut overrides = HashMap::new();
    overrides.insert(
        "NOT_WHITELISTED".to_string(),
        EnvOverride {
            kind: EnvOverrideKind::Literal,
            value: "api".to_string(),
        },
    );
    let value = resolve_env_for_plugin("NOT_WHITELISTED", false, &overrides, |_name| {
        panic!("resolver must not run for a literal")
    });
    assert_eq!(value.as_deref(), Some("api"));
}

#[test]
fn override_reference_resolves_target_and_bypasses_whitelist() {
    let mut overrides = HashMap::new();
    overrides.insert(
        "A".to_string(),
        EnvOverride {
            kind: EnvOverrideKind::Reference,
            value: "B".to_string(),
        },
    );
    let value = resolve_env_for_plugin("A", false, &overrides, |name| {
        if name == "B" {
            Some("b-value".to_string())
        } else {
            None
        }
    });
    assert_eq!(value.as_deref(), Some("b-value"));
}

#[test]
fn miss_falls_back_to_whitelist_gate() {
    let overrides: HashMap<String, EnvOverride> = HashMap::new();
    // Not whitelisted, allow_all=false -> blocked, resolver never runs.
    let blocked = resolve_env_for_plugin("RANDOM_SECRET", false, &overrides, |_| {
        Some("leaked".to_string())
    });
    assert_eq!(blocked, None);

    // Whitelisted name -> resolver runs.
    let allowed = resolve_env_for_plugin("CODEX_HOME", false, &overrides, |name| {
        if name == "CODEX_HOME" {
            Some("/codex".to_string())
        } else {
            None
        }
    });
    assert_eq!(allowed.as_deref(), Some("/codex"));
}
