use super::*;

#[test]
fn normalize_ccusage_output_converts_empty_array_to_daily_object() {
    let normalized = normalize_ccusage_output("noise\n[]\n").expect("normalized output");
    let value: serde_json::Value = serde_json::from_str(&normalized).expect("valid json");
    assert_eq!(value, serde_json::json!({ "daily": [] }));
}

#[test]
fn normalize_ccusage_output_keeps_daily_object_shape() {
    let output = r#"
Saved lockfile
{
  "daily": [
{ "date": "2026-02-21", "totalTokens": 123, "totalCost": 0.5 }
  ],
  "totals": { "totalTokens": 123 }
}
"#;
    let normalized = normalize_ccusage_output(output).expect("normalized output");
    let value: serde_json::Value = serde_json::from_str(&normalized).expect("valid json");
    assert!(value.get("daily").and_then(|v| v.as_array()).is_some());
    assert!(value.get("totals").is_some());
}

#[test]
fn normalize_ccusage_output_rejects_invalid_payloads() {
    assert!(normalize_ccusage_output("not-json").is_none());
    assert!(normalize_ccusage_output(r#"{"totals":{"totalTokens":1}}"#).is_none());
}

#[test]
fn collect_ccusage_runners_uses_fallback_order() {
    let runners = collect_ccusage_runners_with(|kind| match kind {
        CcusageRunnerKind::Bunx => None,
        CcusageRunnerKind::PnpmDlx => Some("pnpm".to_string()),
        CcusageRunnerKind::YarnDlx => Some("yarn".to_string()),
        CcusageRunnerKind::NpmExec => Some("npm".to_string()),
        CcusageRunnerKind::Npx => Some("npx".to_string()),
    });
    assert_eq!(
        runners,
        vec![
            (CcusageRunnerKind::PnpmDlx, "pnpm".to_string()),
            (CcusageRunnerKind::YarnDlx, "yarn".to_string()),
            (CcusageRunnerKind::NpmExec, "npm".to_string()),
            (CcusageRunnerKind::Npx, "npx".to_string()),
        ]
    );
}

#[test]
fn collect_ccusage_runners_returns_empty_when_none_available() {
    let runners = collect_ccusage_runners_with(|_| None);
    assert!(runners.is_empty());
}

#[test]
fn ccusage_query_guard_blocks_overlapping_provider_query() {
    let first = CcusageQueryGuard::acquire(CcusageProvider::Codex)
        .expect("first query should acquire guard");
    assert!(
        CcusageQueryGuard::acquire(CcusageProvider::Codex).is_none(),
        "second query for same provider should be blocked"
    );
    assert!(
        CcusageQueryGuard::acquire(CcusageProvider::Claude).is_some(),
        "different provider should have its own guard"
    );
    drop(first);
    assert!(
        CcusageQueryGuard::acquire(CcusageProvider::Codex).is_some(),
        "guard should release on drop"
    );
}
