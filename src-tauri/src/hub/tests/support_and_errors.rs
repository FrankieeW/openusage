use super::super::*;

#[test]
fn normalize_plugin_filter_trims_dedupes_and_drops_empty() {
    let input = vec![
        "  claude ".to_string(),
        "openrouter".to_string(),
        "claude".to_string(),
        "".to_string(),
        "   ".to_string(),
        "OpenRouter".to_string(),
    ];
    let out = normalize_plugin_filter(Some(input)).unwrap();
    // Case-sensitive dedupe (preserves case)
    assert_eq!(out, vec!["claude", "openrouter", "OpenRouter"]);
}

#[test]
fn normalize_plugin_filter_returns_none_when_empty() {
    assert_eq!(normalize_plugin_filter(None), None);
    assert_eq!(normalize_plugin_filter(Some(vec![])), None);
    assert_eq!(normalize_plugin_filter(Some(vec!["".to_string()])), None);
}

#[test]
fn derive_label_extracts_owner_repo() {
    assert_eq!(
        derive_label_from_url("https://github.com/robinebers/openusage"),
        "robinebers/openusage"
    );
    assert_eq!(
        derive_label_from_url("https://gitlab.com/foo/bar.git"),
        "foo/bar"
    );
}

#[test]
fn hub_error_conflict_carries_other_source_id_in_context() {
    let err = HubError::conflict("src-other");
    let json = serde_json::to_value(&err).unwrap();
    assert_eq!(json["code"], "Conflict");
    assert_eq!(json["context"]["otherSourceId"], "src-other");
}

#[test]
fn hub_error_invalid_url_omits_context() {
    let err = HubError::invalid_url();
    let json = serde_json::to_value(&err).unwrap();
    assert_eq!(json["code"], "InvalidUrl");
    assert!(json.get("context").is_none() || json["context"].is_null());
}
