pub(super) fn redact_value(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= 12 {
        "[REDACTED]".to_string()
    } else {
        let first4: String = chars.iter().take(4).collect();
        let last4: String = chars
            .iter()
            .rev()
            .take(4)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        format!("{}...{}", first4, last4)
    }
}

/// Redact sensitive query parameters in URL
pub(super) fn redact_url(url: &str) -> String {
    let sensitive_params = [
        "key",
        "api_key",
        "apikey",
        "token",
        "access_token",
        "secret",
        "password",
        "auth",
        "authorization",
        "bearer",
        "credential",
        "user",
        "user_id",
        "userid",
        "account_id",
        "accountid",
        "profilearn",
        "profile_arn",
        "email",
        "login",
    ];

    if let Some(query_start) = url.find('?') {
        let (base, query) = url.split_at(query_start + 1);
        let redacted_params: Vec<String> = query
            .split('&')
            .map(|param| {
                if let Some(eq_pos) = param.find('=') {
                    let (name, value) = param.split_at(eq_pos);
                    let value = &value[1..]; // skip '='
                    let name_lower = name.to_lowercase();
                    if sensitive_params.iter().any(|s| name_lower.contains(s)) && !value.is_empty()
                    {
                        format!("{}={}", name, redact_value(value))
                    } else {
                        param.to_string()
                    }
                } else {
                    param.to_string()
                }
            })
            .collect();
        format!("{}{}", base, redacted_params.join("&"))
    } else {
        url.to_string()
    }
}

pub(super) const SENSITIVE_JSON_KEYS: &[&str] = &[
    "name",
    "password",
    "token",
    "access_token",
    "refresh_token",
    "secret",
    "api_key",
    "apiKey",
    "authorization",
    "bearer",
    "credential",
    "session_token",
    "sessionToken",
    "auth_token",
    "authToken",
    "id_token",
    "idToken",
    "accessToken",
    "refreshToken",
    "user_id",
    "userId",
    "account_id",
    "accountId",
    "team_id",
    "teamId",
    "org_id",
    "orgId",
    "account_display_name",
    "accountDisplayName",
    "payment_id",
    "paymentId",
    "profile_arn",
    "profileArn",
    "email",
    "login",
    "analytics_tracking_id",
];

pub(super) fn redact_sensitive_json_values(result: &mut String) {
    for &key in SENSITIVE_JSON_KEYS {
        // Match "key": "value" or "key":"value"
        let pattern = format!(r#""{}":\s*"([^"]+)""#, key);
        if let Ok(re) = regex_lite::Regex::new(&pattern) {
            *result = re
                .replace_all(result.as_str(), |caps: &regex_lite::Captures| {
                    let value = &caps[1];
                    format!("\"{}\": \"{}\"", key, redact_value(value))
                })
                .to_string();
        }
    }
}

/// Redact sensitive patterns in response body for logging
pub(super) fn redact_body(body: &str) -> String {
    let mut result = body.to_string();

    // Redact JWTs (eyJ... pattern with dots)
    let jwt_pattern =
        regex_lite::Regex::new(r"eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+").unwrap();
    result = jwt_pattern
        .replace_all(&result, |caps: &regex_lite::Captures| {
            redact_value(&caps[0])
        })
        .to_string();

    // Redact common API key patterns (sk-xxx, pk-xxx, api_xxx, etc.)
    let api_key_pattern =
        regex_lite::Regex::new(r#"["']?(sk-|pk-|api_|key_|secret_)[A-Za-z0-9_-]{12,}["']?"#)
            .unwrap();
    result = api_key_pattern
        .replace_all(&result, |caps: &regex_lite::Captures| {
            let key = caps[0].trim_matches(|c| c == '"' || c == '\'');
            redact_value(key)
        })
        .to_string();

    if let Ok(devin_session_re) = regex_lite::Regex::new(r#"devin-session-token\$[^\s"',}\]]+"#) {
        result = devin_session_re
            .replace_all(&result, |caps: &regex_lite::Captures| {
                redact_value(&caps[0])
            })
            .to_string();
    }

    redact_sensitive_json_values(&mut result);

    if let Ok(path_re) =
        regex_lite::Regex::new(r#"(/(?:Users|home|opt|private|var|tmp|Applications)/[^\s"')]+)"#)
    {
        result = path_re.replace_all(&result, "[PATH]").to_string();
    }

    result
}

/// Lightweight redaction for log messages.
pub(crate) fn redact_log_message(msg: &str) -> String {
    let mut result = msg.to_string();
    if let Ok(jwt_re) = regex_lite::Regex::new(r"eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+")
    {
        result = jwt_re
            .replace_all(&result, |caps: &regex_lite::Captures| {
                redact_value(&caps[0])
            })
            .to_string();
    }
    if let Ok(api_re) = regex_lite::Regex::new(r#"(sk-|pk-|api_|key_|secret_)[A-Za-z0-9_-]{12,}"#) {
        result = api_re
            .replace_all(&result, |caps: &regex_lite::Captures| {
                redact_value(&caps[0])
            })
            .to_string();
    }
    if let Ok(devin_session_re) = regex_lite::Regex::new(r#"devin-session-token\$[^\s"',}\]]+"#) {
        result = devin_session_re
            .replace_all(&result, |caps: &regex_lite::Captures| {
                redact_value(&caps[0])
            })
            .to_string();
    }
    redact_sensitive_json_values(&mut result);
    if let Ok(account_re) = regex_lite::Regex::new(r#"(account=)([^,\s]+)"#) {
        result = account_re
            .replace_all(&result, |caps: &regex_lite::Captures| {
                format!("{}{}", &caps[1], redact_value(&caps[2]))
            })
            .to_string();
    }
    if let Ok(path_re) =
        regex_lite::Regex::new(r#"(/(?:Users|home|opt|private|var|tmp|Applications)/[^\s"')]+)"#)
    {
        result = path_re.replace_all(&result, "[PATH]").to_string();
    }
    result
}
