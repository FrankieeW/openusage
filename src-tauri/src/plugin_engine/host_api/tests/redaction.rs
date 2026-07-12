use super::*;

#[test]
fn redact_value_shows_first_and_last_four() {
    assert_eq!(redact_value("sk-1234567890abcdef"), "sk-1...cdef");
    assert_eq!(redact_value("short"), "[REDACTED]");
}

#[test]
fn redact_url_redacts_api_key_param() {
    let url = "https://api.example.com/v1?api_key=sk-1234567890abcdef&other=value";
    let redacted = redact_url(url);
    assert!(redacted.contains("api_key=sk-1...cdef"));
    assert!(redacted.contains("other=value"));
}

#[test]
fn redact_url_redacts_user_query_param() {
    let url = "https://cursor.com/api/usage?user=user_abcdefghijklmnopqrstuvwxyz&limit=10";
    let redacted = redact_url(url);
    assert!(
        redacted.contains("user=user...wxyz"),
        "user query param should be redacted, got: {}",
        redacted
    );
    assert!(
        redacted.contains("limit=10"),
        "non-sensitive params should be preserved, got: {}",
        redacted
    );
}

#[test]
fn redact_url_preserves_non_sensitive_params() {
    let url = "https://api.example.com/v1?limit=10&offset=20";
    assert_eq!(redact_url(url), url);
}

#[test]
fn redact_url_redacts_profile_arn_query_param() {
    let url = "https://q.us-east-1.amazonaws.com/getUsageLimits?profileArn=arn:aws:codewhisperer:us-east-1:699475941385:profile/EHGA3GRVQMUK&origin=AI_EDITOR";
    let redacted = redact_url(url);
    assert!(
        !redacted.contains("699475941385"),
        "profileArn should be redacted, got: {}",
        redacted
    );
    assert!(
        redacted.contains("origin=AI_EDITOR"),
        "non-sensitive params should remain visible, got: {}",
        redacted
    );
}

#[test]
fn redact_body_redacts_jwt() {
    let body = r#"{"token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U"}"#;
    let redacted = redact_body(body);
    // JWT gets redacted to first4...last4 format
    assert!(
        !redacted.contains("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"),
        "full JWT should be redacted, got: {}",
        redacted
    );
}

#[test]
fn redact_body_redacts_api_keys() {
    let body = r#"{"key": "sk-1234567890abcdefghij"}"#;
    let redacted = redact_body(body);
    assert!(redacted.contains("sk-1...ghij"));
}

#[test]
fn redact_body_redacts_devin_session_token() {
    let body = r#"metadata apiKey=devin-session-token$abcdefghijklmnopqrstuvwxyz123456"#;
    let redacted = redact_body(body);
    assert!(
        !redacted.contains("devin-session-token$abcdefghijklmnopqrstuvwxyz123456"),
        "Devin session token should be redacted, got: {}",
        redacted
    );
    assert!(
        redacted.contains("devi...3456"),
        "Devin session token should use first4...last4 redaction, got: {}",
        redacted
    );
}

#[test]
fn redact_body_redacts_json_password_field() {
    let body = r#"{"password": "supersecretpassword123"}"#;
    let redacted = redact_body(body);
    assert!(
        !redacted.contains("supersecretpassword123"),
        "password should be redacted, got: {}",
        redacted
    );
}

#[test]
fn redact_body_redacts_user_id_and_email() {
    let body = r#"{"user_id": "user-iupzZ7KFykMLrnzpkHSq7wjo", "email": "rob@sunstory.com"}"#;
    let redacted = redact_body(body);
    assert!(
        !redacted.contains("user-iupzZ7KFykMLrnzpkHSq7wjo"),
        "user_id should be redacted, got: {}",
        redacted
    );
    assert!(
        !redacted.contains("rob@sunstory.com"),
        "email should be redacted, got: {}",
        redacted
    );
    // Should show first4...last4
    assert!(
        redacted.contains("user...7wjo"),
        "user_id should show first4...last4, got: {}",
        redacted
    );
    assert!(
        redacted.contains("rob@....com"),
        "email should show first4...last4, got: {}",
        redacted
    );
}

#[test]
fn redact_body_redacts_camel_case_user_and_account_ids() {
    let body =
        r#"{"userId": "user_abcdefghijklmnopqrstuvwxyz", "accountId": "acct_1234567890abcdef"}"#;
    let redacted = redact_body(body);
    assert!(
        !redacted.contains("user_abcdefghijklmnopqrstuvwxyz"),
        "userId should be redacted, got: {}",
        redacted
    );
    assert!(
        !redacted.contains("acct_1234567890abcdef"),
        "accountId should be redacted, got: {}",
        redacted
    );
    assert!(
        redacted.contains("user...wxyz"),
        "userId should show first4...last4, got: {}",
        redacted
    );
    assert!(
        redacted.contains("acct...cdef"),
        "accountId should show first4...last4, got: {}",
        redacted
    );
}

#[test]
fn redact_body_redacts_devin_org_and_account_display_name() {
    let body = r#"{"orgId":"org-6b6e9de248db472bb25b296599ea3dc0","accountDisplayName":"rob@sunstory.com","devinInfo":{"org_id":"org-abcdef1234567890","account_display_name":"team@example.com"}}"#;
    let redacted = redact_body(body);
    assert!(
        !redacted.contains("org-6b6e9de248db472bb25b296599ea3dc0"),
        "orgId should be redacted, got: {}",
        redacted
    );
    assert!(
        !redacted.contains("rob@sunstory.com"),
        "accountDisplayName should be redacted, got: {}",
        redacted
    );
    assert!(
        !redacted.contains("org-abcdef1234567890"),
        "org_id should be redacted, got: {}",
        redacted
    );
    assert!(
        !redacted.contains("team@example.com"),
        "account_display_name should be redacted, got: {}",
        redacted
    );
    assert!(
        redacted.contains("org-...3dc0"),
        "orgId should show first4...last4, got: {}",
        redacted
    );
    assert!(
        redacted.contains("rob@....com"),
        "accountDisplayName should show first4...last4, got: {}",
        redacted
    );
}

#[test]
fn redact_body_redacts_team_id_payment_id_and_paths() {
    let body = r#"{"teamId":"cc1ac023-9ff5-4c1f-a5a4-ae2a82df4243","paymentId":"cus_S5m1PGxjLWoc1c","binaryPath":"/opt/homebrew/bin/bunx","homePath":"/Users/rebers/.claude"}"#;
    let redacted = redact_body(body);
    assert!(
        !redacted.contains("cc1ac023-9ff5-4c1f-a5a4-ae2a82df4243"),
        "teamId should be redacted, got: {}",
        redacted
    );
    assert!(
        !redacted.contains("cus_S5m1PGxjLWoc1c"),
        "paymentId should be redacted, got: {}",
        redacted
    );
    assert!(
        !redacted.contains("/opt/homebrew/bin/bunx"),
        "path should be redacted, got: {}",
        redacted
    );
    assert!(
        !redacted.contains("/Users/rebers/.claude"),
        "path should be redacted, got: {}",
        redacted
    );
    assert!(
        redacted.contains("[PATH]"),
        "expected path marker, got: {}",
        redacted
    );
}

#[test]
fn redact_body_redacts_profile_arn_fields() {
    let body = r#"{"profileArn":"arn:aws:codewhisperer:us-east-1:699475941385:profile/EHGA3GRVQMUK","profile_arn":"arn:aws:codewhisperer:us-east-1:699475941385:profile/EHGA3GRVQMUK"}"#;
    let redacted = redact_body(body);
    assert!(
        !redacted.contains("699475941385"),
        "profile arn should be redacted, got: {}",
        redacted
    );
    assert!(
        redacted.contains("arn:...QMUK"),
        "profile arn should use first4...last4 redaction, got: {}",
        redacted
    );
}

#[test]
fn redact_log_message_redacts_jwt_and_api_key() {
    let msg = "token=eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U key=sk-1234567890abcdef";
    let redacted = redact_log_message(msg);
    assert!(
        !redacted.contains("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"),
        "JWT should be redacted"
    );
    assert!(
        !redacted.contains("sk-1234567890abcdef"),
        "API key should be redacted"
    );
}

#[test]
fn redact_log_message_redacts_opaque_json_api_key() {
    // Given
    let msg = r#"{"apiKey":"opaque-secret"}"#;

    // When
    let redacted = redact_log_message(msg);

    // Then
    assert_eq!(redacted, r#"{"apiKey": "opaq...cret"}"#);
}

#[test]
fn redact_log_message_redacts_devin_session_token() {
    let msg = "auth=devin-session-token$abcdefghijklmnopqrstuvwxyz123456";
    let redacted = redact_log_message(msg);
    assert!(
        !redacted.contains("devin-session-token$abcdefghijklmnopqrstuvwxyz123456"),
        "Devin session token should be redacted, got: {}",
        redacted
    );
    assert!(
        redacted.contains("devi...3456"),
        "Devin session token should use first4...last4 redaction, got: {}",
        redacted
    );
}

#[test]
fn redact_log_message_redacts_account_and_paths() {
    let msg = "keychain read: service=Claude Code-credentials, account=rebers path=/opt/homebrew/bin/bunx home=/Users/rebers/.claude";
    let redacted = redact_log_message(msg);
    assert!(
        !redacted.contains("account=rebers"),
        "account should be redacted, got: {}",
        redacted
    );
    assert!(
        !redacted.contains("/opt/homebrew/bin/bunx"),
        "path should be redacted, got: {}",
        redacted
    );
    assert!(
        !redacted.contains("/Users/rebers/.claude"),
        "path should be redacted, got: {}",
        redacted
    );
    assert!(
        redacted.contains("account=[REDACTED]"),
        "expected redacted account, got: {}",
        redacted
    );
    assert!(
        redacted.contains("[PATH]"),
        "expected redacted path, got: {}",
        redacted
    );
}

#[test]
fn redact_body_redacts_login_and_analytics_tracking_id() {
    let body =
        r#"{"login":"robinebers","analytics_tracking_id":"c9df3f012bb8c2eb7aae6868ee8da6cf"}"#;
    let redacted = redact_body(body);
    assert!(
        !redacted.contains("robinebers"),
        "login should be redacted, got: {}",
        redacted
    );
    assert!(
        !redacted.contains("c9df3f012bb8c2eb7aae6868ee8da6cf"),
        "analytics_tracking_id should be redacted, got: {}",
        redacted
    );
    // login is short (<=12 chars) so becomes [REDACTED]; analytics_tracking_id is long so first4...last4
    assert!(
        redacted.contains("[REDACTED]"),
        "login should be redacted, got: {}",
        redacted
    );
    assert!(
        redacted.contains("c9df...a6cf"),
        "analytics_tracking_id should show first4...last4, got: {}",
        redacted
    );
}

#[test]
fn redact_body_redacts_name_field() {
    let body =
        r#"{"userStatus":{"name":"Robin Ebers","email":"rob@sunstory.com","planStatus":{}}}"#;
    let redacted = redact_body(body);
    assert!(
        !redacted.contains("Robin Ebers"),
        "name should be redacted, got: {}",
        redacted
    );
    assert!(
        !redacted.contains("rob@sunstory.com"),
        "email should be redacted, got: {}",
        redacted
    );
    // "Robin Ebers" is 11 chars (<=12) so becomes [REDACTED]
    assert!(
        redacted.contains("\"name\": \"[REDACTED]\""),
        "name should show [REDACTED], got: {}",
        redacted
    );
}
