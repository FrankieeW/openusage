use super::*;

pub(super) fn decrypt_aes_256_gcm_envelope(
    envelope: &str,
    key_b64: &str,
) -> Result<String, String> {
    let trimmed_envelope = envelope.trim();
    let trimmed_key = key_b64.trim();
    let parts: Vec<&str> = trimmed_envelope.split(':').collect();
    if parts.len() != 3 {
        return Err("invalid AES-GCM envelope".to_string());
    }

    let key = BASE64_STANDARD
        .decode(trimmed_key)
        .map_err(|e| format!("invalid base64 key: {}", e))?;
    if key.len() != 32 {
        return Err(format!(
            "invalid AES-256 key length: expected 32 bytes, got {}",
            key.len()
        ));
    }

    let iv = BASE64_STANDARD
        .decode(parts[0])
        .map_err(|e| format!("invalid base64 iv: {}", e))?;
    if iv.len() != 16 {
        return Err(format!(
            "invalid AES-GCM iv length: expected 16 bytes, got {}",
            iv.len()
        ));
    }

    let tag = BASE64_STANDARD
        .decode(parts[1])
        .map_err(|e| format!("invalid base64 auth tag: {}", e))?;
    if tag.len() != 16 {
        return Err(format!(
            "invalid AES-GCM auth tag length: expected 16 bytes, got {}",
            tag.len()
        ));
    }

    let ciphertext = BASE64_STANDARD
        .decode(parts[2])
        .map_err(|e| format!("invalid base64 ciphertext: {}", e))?;

    type Aes256Gcm16 = AesGcm<Aes256, U16>;
    let cipher =
        Aes256Gcm16::new_from_slice(&key).map_err(|e| format!("decrypt init failed: {}", e))?;
    let nonce = Nonce::<U16>::from_slice(&iv);

    let mut ciphertext_and_tag = ciphertext;
    ciphertext_and_tag.extend_from_slice(&tag);
    let plaintext = cipher
        .decrypt(nonce, ciphertext_and_tag.as_ref())
        .map_err(|_| "decrypt finalize failed".to_string())?;

    String::from_utf8(plaintext).map_err(|e| format!("decrypted payload is not UTF-8: {}", e))
}

pub(super) fn encrypt_aes_256_gcm_envelope(
    plaintext: &str,
    key_b64: &str,
) -> Result<String, String> {
    let trimmed_key = key_b64.trim();
    let key = BASE64_STANDARD
        .decode(trimmed_key)
        .map_err(|e| format!("invalid base64 key: {}", e))?;
    if key.len() != 32 {
        return Err(format!(
            "invalid AES-256 key length: expected 32 bytes, got {}",
            key.len()
        ));
    }

    type Aes256Gcm16 = AesGcm<Aes256, U16>;
    let cipher =
        Aes256Gcm16::new_from_slice(&key).map_err(|e| format!("encrypt init failed: {}", e))?;
    let mut iv = [0_u8; 16];
    OsRng.fill_bytes(&mut iv);
    let nonce = Nonce::<U16>::from_slice(&iv);
    let ciphertext_and_tag = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|_| "encrypt finalize failed".to_string())?;
    if ciphertext_and_tag.len() < 16 {
        return Err("encrypted payload missing auth tag".to_string());
    }
    let split_at = ciphertext_and_tag.len() - 16;
    let (ciphertext, tag) = ciphertext_and_tag.split_at(split_at);

    Ok(format!(
        "{}:{}:{}",
        BASE64_STANDARD.encode(iv),
        BASE64_STANDARD.encode(tag),
        BASE64_STANDARD.encode(ciphertext)
    ))
}

pub(super) fn inject_crypto<'js>(ctx: &Ctx<'js>, host: &Object<'js>) -> rquickjs::Result<()> {
    let crypto_obj = Object::new(ctx.clone())?;

    crypto_obj.set(
        "decryptAes256Gcm",
        Function::new(
            ctx.clone(),
            move |ctx_inner: Ctx<'_>,
                  envelope: String,
                  key_b64: String|
                  -> rquickjs::Result<String> {
                decrypt_aes_256_gcm_envelope(&envelope, &key_b64)
                    .map_err(|e| Exception::throw_message(&ctx_inner, &e))
            },
        )?,
    )?;

    crypto_obj.set(
        "encryptAes256Gcm",
        Function::new(
            ctx.clone(),
            move |ctx_inner: Ctx<'_>,
                  plaintext: String,
                  key_b64: String|
                  -> rquickjs::Result<String> {
                encrypt_aes_256_gcm_envelope(&plaintext, &key_b64)
                    .map_err(|e| Exception::throw_message(&ctx_inner, &e))
            },
        )?,
    )?;

    crypto_obj.set(
        "sha256Hex",
        Function::new(ctx.clone(), move |text: String| -> String {
            let digest = Sha256::digest(text.as_bytes());
            // Lowercase hex, matches Node's `crypto.createHash("sha256").update(x).digest("hex")`
            // and the upstream Claude Code keychain helper.
            let mut out = String::with_capacity(digest.len() * 2);
            for byte in digest.iter() {
                use std::fmt::Write as _;
                let _ = write!(&mut out, "{:02x}", byte);
            }
            out
        })?,
    )?;

    host.set("crypto", crypto_obj)?;
    Ok(())
}
