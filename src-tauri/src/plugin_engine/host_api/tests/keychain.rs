use super::*;

#[test]
fn keychain_api_exposes_write_variants() {
    let rt = Runtime::new().expect("runtime");
    let ctx = Context::full(&rt).expect("context");
    ctx.with(|ctx| {
        let app_data = std::env::temp_dir();
        inject_host_api(&ctx, "test", &app_data, "0.0.0").expect("inject host api");
        let globals = ctx.globals();
        let probe_ctx: Object = globals.get("__openusage_ctx").expect("probe ctx");
        let host: Object = probe_ctx.get("host").expect("host");
        let keychain: Object = host.get("keychain").expect("keychain");
        let _read: Function = keychain
            .get("readGenericPassword")
            .expect("readGenericPassword");
        let _read_current_user: Function = keychain
            .get("readGenericPasswordForCurrentUser")
            .expect("readGenericPasswordForCurrentUser");
        let _write: Function = keychain
            .get("writeGenericPassword")
            .expect("writeGenericPassword");
        let _write_current_user: Function = keychain
            .get("writeGenericPasswordForCurrentUser")
            .expect("writeGenericPasswordForCurrentUser");
    });
}

#[test]
fn keychain_read_generic_password_accepts_optional_account_arg_from_js() {
    let rt = Runtime::new().expect("runtime");
    let ctx = Context::full(&rt).expect("context");
    ctx.with(|ctx| {
        let app_data = std::env::temp_dir();
        inject_host_api(&ctx, "test", &app_data, "0.0.0").expect("inject host api");

        let message: String = ctx
            .eval(
                r#"
                try {
                    __openusage_ctx.host.keychain.readGenericPassword("__openusage_missing_service__");
                    "ok";
                } catch (e) {
                    String(e);
                }
                "#,
            )
            .expect("js eval");

        assert!(
            !message.contains("2 where expected"),
            "single-arg call should reach the keychain implementation, got: {}",
            message
        );
    });
}

#[test]
fn current_macos_keychain_account_prefers_explicit_user_value() {
    assert_eq!(
        current_macos_keychain_account_from_user_env(Some("openusage-test-user".to_string())),
        "openusage-test-user"
    );
}

#[test]
fn keychain_find_generic_password_args_include_service_only_lookup() {
    let args = keychain_find_generic_password_args("Claude Code-credentials");
    let rendered: Vec<String> = args
        .into_iter()
        .map(|value| value.to_string_lossy().into_owned())
        .collect();

    assert_eq!(
        rendered,
        vec![
            "find-generic-password",
            "-s",
            "Claude Code-credentials",
            "-w",
        ]
    );
}

#[test]
fn keychain_find_generic_password_args_for_account_include_account_and_service() {
    let args = keychain_find_generic_password_args_for_account(
        "Claude Code-credentials",
        "openusage-test-user",
    );
    let rendered: Vec<String> = args
        .into_iter()
        .map(|value| value.to_string_lossy().into_owned())
        .collect();

    assert_eq!(
        rendered,
        vec![
            "find-generic-password",
            "-a",
            "openusage-test-user",
            "-s",
            "Claude Code-credentials",
            "-w",
        ]
    );
}

#[test]
fn keychain_add_generic_password_args_include_service_only_write() {
    let args = keychain_add_generic_password_args("Claude Code-credentials", "secret-value");
    let rendered: Vec<String> = args
        .into_iter()
        .map(|value| value.to_string_lossy().into_owned())
        .collect();

    assert_eq!(
        rendered,
        vec![
            "add-generic-password",
            "-U",
            "-s",
            "Claude Code-credentials",
            "-w",
            "secret-value",
        ]
    );
}

#[test]
fn keychain_add_generic_password_args_for_account_include_update_account_service_and_value() {
    let args = keychain_add_generic_password_args_for_account(
        "Claude Code-credentials",
        "openusage-test-user",
        "secret-value",
    );
    let rendered: Vec<String> = args
        .into_iter()
        .map(|value| value.to_string_lossy().into_owned())
        .collect();

    assert_eq!(
        rendered,
        vec![
            "add-generic-password",
            "-U",
            "-a",
            "openusage-test-user",
            "-s",
            "Claude Code-credentials",
            "-w",
            "secret-value",
        ]
    );
}
