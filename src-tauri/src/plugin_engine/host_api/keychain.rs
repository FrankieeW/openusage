use super::env::read_env_value_via_command;
use super::redaction::redact_value;
use super::*;

pub(super) fn current_macos_keychain_account_from_user_env(user_env: Option<String>) -> String {
    user_env
        .and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
        .or_else(|| read_env_value_via_command("id", &["-un"]))
        .unwrap_or_else(|| "openusage-user".to_string())
}

pub(super) fn current_macos_keychain_account() -> String {
    current_macos_keychain_account_from_user_env(read_env_from_process("USER"))
}

pub(super) fn keychain_find_generic_password_args(service: &str) -> Vec<OsString> {
    vec![
        OsString::from("find-generic-password"),
        OsString::from("-s"),
        OsString::from(service),
        OsString::from("-w"),
    ]
}

pub(super) fn keychain_find_generic_password_args_for_account(
    service: &str,
    account: &str,
) -> Vec<OsString> {
    vec![
        OsString::from("find-generic-password"),
        OsString::from("-a"),
        OsString::from(account),
        OsString::from("-s"),
        OsString::from(service),
        OsString::from("-w"),
    ]
}

pub(super) fn keychain_add_generic_password_args(service: &str, value: &str) -> Vec<OsString> {
    vec![
        OsString::from("add-generic-password"),
        OsString::from("-U"),
        OsString::from("-s"),
        OsString::from(service),
        OsString::from("-w"),
        OsString::from(value),
    ]
}

pub(super) fn keychain_add_generic_password_args_for_account(
    service: &str,
    account: &str,
    value: &str,
) -> Vec<OsString> {
    vec![
        OsString::from("add-generic-password"),
        OsString::from("-U"),
        OsString::from("-a"),
        OsString::from(account),
        OsString::from("-s"),
        OsString::from(service),
        OsString::from("-w"),
        OsString::from(value),
    ]
}

pub(super) fn inject_keychain<'js>(
    ctx: &Ctx<'js>,
    host: &Object<'js>,
    plugin_id: &str,
) -> rquickjs::Result<()> {
    let keychain_obj = Object::new(ctx.clone())?;
    let pid_read = plugin_id.to_string();

    keychain_obj.set(
        "readGenericPassword",
        Function::new(
            ctx.clone(),
            move |ctx_inner: Ctx<'_>,
                  service: String,
                  account_args: Rest<Option<String>>|
                  -> rquickjs::Result<String> {
                if !cfg!(target_os = "macos") {
                    return Err(Exception::throw_message(
                        &ctx_inner,
                        "keychain API is only supported on macOS",
                    ));
                }
                let account = account_args
                    .0
                    .into_iter()
                    .next()
                    .flatten()
                    .and_then(|value| {
                        let trimmed = value.trim();
                        if trimmed.is_empty() {
                            None
                        } else {
                            Some(trimmed.to_string())
                        }
                    });
                let redacted_account = account.as_ref().map(|value| redact_value(value));
                if let Some(ref redacted) = redacted_account {
                    log::info!(
                        "[plugin:{}] keychain read: service={}, account={}",
                        pid_read,
                        service,
                        redacted
                    );
                } else {
                    log::info!("[plugin:{}] keychain read: service={}", pid_read, service);
                }
                let args = if let Some(ref account) = account {
                    keychain_find_generic_password_args_for_account(&service, account)
                } else {
                    keychain_find_generic_password_args(&service)
                };
                let output = std::process::Command::new("security")
                    .args(args)
                    .output()
                    .map_err(|e| {
                        Exception::throw_message(
                            &ctx_inner,
                            &format!("keychain read failed: {}", e),
                        )
                    })?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let first_line = stderr.lines().next().unwrap_or("").trim();
                    if let Some(ref redacted) = redacted_account {
                        log::warn!(
                            "[plugin:{}] keychain read miss: service={}, account={}, error={}",
                            pid_read,
                            service,
                            redacted,
                            first_line
                        );
                    } else {
                        log::warn!(
                            "[plugin:{}] keychain read miss: service={}, error={}",
                            pid_read,
                            service,
                            first_line
                        );
                    }
                    return Err(Exception::throw_message(
                        &ctx_inner,
                        &format!("keychain item not found: {}", first_line),
                    ));
                }

                if let Some(ref redacted) = redacted_account {
                    log::info!(
                        "[plugin:{}] keychain read hit: service={}, account={}",
                        pid_read,
                        service,
                        redacted
                    );
                } else {
                    log::info!(
                        "[plugin:{}] keychain read hit: service={}",
                        pid_read,
                        service
                    );
                }
                Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
            },
        )?,
    )?;

    let pid_read_current_user = plugin_id.to_string();
    keychain_obj.set(
        "readGenericPasswordForCurrentUser",
        Function::new(
            ctx.clone(),
            move |ctx_inner: Ctx<'_>, service: String| -> rquickjs::Result<String> {
                if !cfg!(target_os = "macos") {
                    return Err(Exception::throw_message(
                        &ctx_inner,
                        "keychain API is only supported on macOS",
                    ));
                }
                let account = current_macos_keychain_account();
                let args = keychain_find_generic_password_args_for_account(&service, &account);
                let redacted_account = redact_value(&account);
                log::info!(
                    "[plugin:{}] keychain read: service={}, account={}",
                    pid_read_current_user,
                    service,
                    redacted_account
                );
                let output = std::process::Command::new("security")
                    .args(&args)
                    .output()
                    .map_err(|e| {
                        Exception::throw_message(
                            &ctx_inner,
                            &format!("keychain read failed: {}", e),
                        )
                    })?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let first_line = stderr.lines().next().unwrap_or("").trim();
                    log::warn!(
                        "[plugin:{}] keychain read miss: service={}, account={}, error={}",
                        pid_read_current_user,
                        service,
                        redacted_account,
                        first_line
                    );
                    return Err(Exception::throw_message(
                        &ctx_inner,
                        &format!("keychain item not found: {}", first_line),
                    ));
                }

                log::info!(
                    "[plugin:{}] keychain read hit: service={}, account={}",
                    pid_read_current_user,
                    service,
                    redacted_account
                );
                Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
            },
        )?,
    )?;

    let pid_write = plugin_id.to_string();
    keychain_obj.set(
        "writeGenericPassword",
        Function::new(
            ctx.clone(),
            move |ctx_inner: Ctx<'_>, service: String, value: String| -> rquickjs::Result<()> {
                if !cfg!(target_os = "macos") {
                    return Err(Exception::throw_message(
                        &ctx_inner,
                        "keychain API is only supported on macOS",
                    ));
                }
                log::info!("[plugin:{}] keychain write: service={}", pid_write, service);

                let mut account_arg: Option<String> = None;
                let find_output = std::process::Command::new("security")
                    .args(["find-generic-password", "-s", &service])
                    .output();

                if let Ok(output) = find_output
                    && output.status.success()
                {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    for line in stdout.lines() {
                        if let Some(start) = line.find("\"acct\"<blob>=\"") {
                            let rest = &line[start + 14..];
                            if let Some(end) = rest.find('"') {
                                account_arg = Some(rest[..end].to_string());
                                break;
                            }
                        }
                    }
                }

                let output = if let Some(ref acct) = account_arg {
                    std::process::Command::new("security")
                        .args(keychain_add_generic_password_args_for_account(
                            &service, acct, &value,
                        ))
                        .output()
                } else {
                    std::process::Command::new("security")
                        .args(keychain_add_generic_password_args(&service, &value))
                        .output()
                }
                .map_err(|e| {
                    Exception::throw_message(&ctx_inner, &format!("keychain write failed: {}", e))
                })?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let first_line = stderr.lines().next().unwrap_or("").trim();
                    log::warn!(
                        "[plugin:{}] keychain write failed: service={}, error={}",
                        pid_write,
                        service,
                        first_line
                    );
                    return Err(Exception::throw_message(
                        &ctx_inner,
                        &format!("keychain write failed: {}", first_line),
                    ));
                }

                log::info!(
                    "[plugin:{}] keychain write succeeded: service={}",
                    pid_write,
                    service
                );
                Ok(())
            },
        )?,
    )?;

    let pid_write_current_user = plugin_id.to_string();
    keychain_obj.set(
        "writeGenericPasswordForCurrentUser",
        Function::new(
            ctx.clone(),
            move |ctx_inner: Ctx<'_>, service: String, value: String| -> rquickjs::Result<()> {
                if !cfg!(target_os = "macos") {
                    return Err(Exception::throw_message(
                        &ctx_inner,
                        "keychain API is only supported on macOS",
                    ));
                }
                let account = current_macos_keychain_account();
                let args =
                    keychain_add_generic_password_args_for_account(&service, &account, &value);
                let redacted_account = redact_value(&account);
                log::info!(
                    "[plugin:{}] keychain write: service={}, account={}",
                    pid_write_current_user,
                    service,
                    redacted_account
                );
                let output = std::process::Command::new("security")
                    .args(&args)
                    .output()
                    .map_err(|e| {
                        Exception::throw_message(
                            &ctx_inner,
                            &format!("keychain write failed: {}", e),
                        )
                    })?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let first_line = stderr.lines().next().unwrap_or("").trim();
                    log::warn!(
                        "[plugin:{}] keychain write failed: service={}, account={}, error={}",
                        pid_write_current_user,
                        service,
                        redacted_account,
                        first_line
                    );
                    return Err(Exception::throw_message(
                        &ctx_inner,
                        &format!("keychain write failed: {}", first_line),
                    ));
                }

                log::info!(
                    "[plugin:{}] keychain write succeeded: service={}, account={}",
                    pid_write_current_user,
                    service,
                    redacted_account
                );
                Ok(())
            },
        )?,
    )?;

    host.set("keychain", keychain_obj)?;
    Ok(())
}
