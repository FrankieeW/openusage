use super::*;

#[test]
fn inject_host_api_preserves_context_shape_and_host_registration_order() {
    // Given
    let rt = Runtime::new().expect("runtime");
    let ctx = Context::full(&rt).expect("context");

    ctx.with(|ctx| {
        let app_data = std::env::temp_dir();

        // When
        inject_host_api(&ctx, "test", &app_data, "0.0.0").expect("inject host api");

        // Then
        let context_keys: String = ctx
            .eval("Object.keys(__openusage_ctx).join(',')")
            .expect("context keys");
        let app_keys: String = ctx
            .eval("Object.keys(__openusage_ctx.app).join(',')")
            .expect("app keys");
        let host_keys: String = ctx
            .eval("Object.keys(__openusage_ctx.host).join(',')")
            .expect("host keys");

        assert_eq!(context_keys, "nowIso,app,host");
        assert_eq!(app_keys, "version,platform,appDataDir,pluginDataDir");
        assert_eq!(
            host_keys,
            "log,fs,crypto,env,http,keychain,sqlite,ls,ccusage"
        );
    });
}

#[test]
fn runtime_patches_preserve_js_callable_shape() {
    // Given
    let rt = Runtime::new().expect("runtime");
    let ctx = Context::full(&rt).expect("context");

    ctx.with(|ctx| {
        let app_data = std::env::temp_dir();
        inject_host_api(&ctx, "test", &app_data, "0.0.0").expect("inject host api");

        // When
        patch_http_wrapper(&ctx).expect("patch http wrapper");
        patch_ls_wrapper(&ctx).expect("patch ls wrapper");
        patch_ccusage_wrapper(&ctx).expect("patch ccusage wrapper");
        inject_utils(&ctx).expect("inject utils");

        // Then
        let callable_types: String = ctx
            .eval(
                "[typeof __openusage_ctx.host.http.request, \
                 typeof __openusage_ctx.host.ls.discover, \
                 typeof __openusage_ctx.host.ccusage.query, \
                 typeof __openusage_ctx.line.text].join(',')",
            )
            .expect("callable types");
        assert_eq!(callable_types, "function,function,function,function");
    });
}
