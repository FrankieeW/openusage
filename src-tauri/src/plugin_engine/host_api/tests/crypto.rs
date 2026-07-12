use super::*;

fn encrypt_aes_256_gcm_envelope_for_test(key: &[u8], plaintext: &str) -> String {
    let iv = [7_u8; 16];
    type Aes256Gcm16 = AesGcm<Aes256, U16>;
    let cipher = Aes256Gcm16::new_from_slice(key).expect("encrypt init");
    let nonce = Nonce::<U16>::from_slice(&iv);
    let ciphertext_and_tag = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .expect("encrypt finalize");
    let split_at = ciphertext_and_tag.len() - 16;
    let (ciphertext, tag) = ciphertext_and_tag.split_at(split_at);

    format!(
        "{}:{}:{}",
        BASE64_STANDARD.encode(iv),
        BASE64_STANDARD.encode(tag),
        BASE64_STANDARD.encode(ciphertext)
    )
}

fn node_generated_aes_256_gcm_vector_for_test() -> (&'static str, &'static str, &'static str) {
    (
        "CwsLCwsLCwsLCwsLCwsLCwsLCwsLCwsLCwsLCwsLCws=",
        "BwcHBwcHBwcHBwcHBwcHBw==:yFbCs4LOJ0aj9NPNf5pfVA==:7PKjtOdATLClvaWrMw0b0M8Nov4KPhxwQX4hdczqQlcZi9Zhi6DjAoK+WolvMwuhPIk=",
        r#"{"access_token":"token","refresh_token":"refresh"}"#,
    )
}

#[test]
fn decrypt_aes_256_gcm_envelope_round_trips_plaintext() {
    let key = [11_u8; 32];
    let key_b64 = BASE64_STANDARD.encode(key);
    let plaintext = r#"{"access_token":"token","refresh_token":"refresh"}"#;
    let envelope = encrypt_aes_256_gcm_envelope_for_test(&key, plaintext);

    let decrypted = decrypt_aes_256_gcm_envelope(&envelope, &key_b64).expect("decrypt envelope");

    assert_eq!(decrypted, plaintext);
}

#[test]
fn encrypt_aes_256_gcm_envelope_round_trips_plaintext() {
    let key = [21_u8; 32];
    let key_b64 = BASE64_STANDARD.encode(key);
    let plaintext = r#"{"access_token":"token-2","refresh_token":"refresh-2"}"#;

    let envelope = encrypt_aes_256_gcm_envelope(plaintext, &key_b64).expect("encrypt envelope");
    let decrypted = decrypt_aes_256_gcm_envelope(&envelope, &key_b64).expect("decrypt envelope");

    assert_eq!(decrypted, plaintext);
}

#[test]
fn decrypt_aes_256_gcm_envelope_rejects_invalid_component_lengths() {
    let key_b64 = BASE64_STANDARD.encode([9_u8; 32]);
    let short_key_b64 = BASE64_STANDARD.encode([7_u8; 31]);
    let iv_b64 = BASE64_STANDARD.encode([1_u8; 15]);
    let tag_b64 = BASE64_STANDARD.encode([2_u8; 16]);
    let ciphertext_b64 = BASE64_STANDARD.encode([3_u8; 8]);

    let key_err =
        decrypt_aes_256_gcm_envelope("AQ==:AQ==:AQ==", &short_key_b64).expect_err("key length");
    assert!(key_err.contains("expected 32 bytes"));

    let iv_err = decrypt_aes_256_gcm_envelope(
        &format!("{}:{}:{}", iv_b64, tag_b64, ciphertext_b64),
        &key_b64,
    )
    .expect_err("iv length");
    assert!(iv_err.contains("iv length"));

    let short_tag_b64 = BASE64_STANDARD.encode([2_u8; 15]);
    let tag_err = decrypt_aes_256_gcm_envelope(
        &format!(
            "{}:{}:{}",
            BASE64_STANDARD.encode([1_u8; 16]),
            short_tag_b64,
            ciphertext_b64
        ),
        &key_b64,
    )
    .expect_err("tag length");
    assert!(tag_err.contains("auth tag length"));
}

#[test]
fn crypto_api_exposes_decrypt() {
    let rt = Runtime::new().expect("runtime");
    let ctx = Context::full(&rt).expect("context");
    ctx.with(|ctx| {
        let app_data = std::env::temp_dir();
        inject_host_api(&ctx, "test", &app_data, "0.0.0").expect("inject host api");
        let globals = ctx.globals();
        let probe_ctx: Object = globals.get("__openusage_ctx").expect("probe ctx");
        let host: Object = probe_ctx.get("host").expect("host");
        let crypto: Object = host.get("crypto").expect("crypto");
        let _decrypt: Function = crypto.get("decryptAes256Gcm").expect("decryptAes256Gcm");
        let _encrypt: Function = crypto.get("encryptAes256Gcm").expect("encryptAes256Gcm");
    });
}

#[test]
fn crypto_api_decrypts_node_generated_envelope_from_js() {
    let (key_b64, envelope, expected_plaintext) = node_generated_aes_256_gcm_vector_for_test();
    let rt = Runtime::new().expect("runtime");
    let ctx = Context::full(&rt).expect("context");
    ctx.with(|ctx| {
        let app_data = std::env::temp_dir();
        inject_host_api(&ctx, "test", &app_data, "0.0.0").expect("inject host api");
        let js_expr = format!(
            r#"__openusage_ctx.host.crypto.decryptAes256Gcm("{}", "{}")"#,
            envelope, key_b64
        );
        let decrypted: String = ctx.eval(js_expr).expect("js decrypt");
        assert_eq!(decrypted, expected_plaintext);
    });
}

#[test]
fn crypto_api_exposes_sha256_hex() {
    let rt = Runtime::new().expect("runtime");
    let ctx = Context::full(&rt).expect("context");
    ctx.with(|ctx| {
        let app_data = std::env::temp_dir();
        inject_host_api(&ctx, "test", &app_data, "0.0.0").expect("inject host api");
        // Vector: `printf '%s' 'hello' | shasum -a 256`
        let result: String = ctx
            .eval(r#"__openusage_ctx.host.crypto.sha256Hex("hello")"#)
            .expect("js sha256");
        assert_eq!(
            result,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );

        let empty: String = ctx
            .eval(r#"__openusage_ctx.host.crypto.sha256Hex("")"#)
            .expect("js sha256 empty");
        assert_eq!(
            empty,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    });
}
