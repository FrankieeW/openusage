use super::bar_chart::MAX_BAR_CHART_POINTS;
use super::*;
use crate::plugin_engine::manifest::{LoadedPlugin, PluginManifest};
use serde_json::Value as JsonValue;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn test_plugin(entry_script: &str) -> LoadedPlugin {
    LoadedPlugin {
        manifest: PluginManifest {
            schema_version: 1,
            id: "test".to_string(),
            name: "Test".to_string(),
            version: "0.0.0".to_string(),
            entry: "plugin.js".to_string(),
            icon: "icon.svg".to_string(),
            brand_color: None,
            lines: vec![],
            links: vec![],
        },
        plugin_dir: PathBuf::from("."),
        entry_script: entry_script.to_string(),
        icon_data_url: "data:image/svg+xml;base64,".to_string(),
    }
}

fn temp_app_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!("openusage-test-{}-{}", label, nanos))
}

fn error_text(output: PluginOutput) -> String {
    match output.lines.first() {
        Some(MetricLine::Badge { text, .. }) => text.clone(),
        other => panic!("expected error badge, got {:?}", other),
    }
}

#[test]
fn run_probe_returns_thrown_string_from_sync_error() {
    let plugin = test_plugin(
        r#"
        globalThis.__openusage_plugin = {
            probe() {
                throw "boom";
            }
        };
        "#,
    );
    let output = run_probe(&plugin, &temp_app_dir("sync"), "0.0.0");
    assert_eq!(error_text(output), "boom");
}

#[test]
fn run_probe_returns_thrown_string_from_async_error() {
    let plugin = test_plugin(
        r#"
        globalThis.__openusage_plugin = {
            probe: async function () {
                throw "boom";
            }
        };
        "#,
    );
    let output = run_probe(&plugin, &temp_app_dir("async"), "0.0.0");
    assert_eq!(error_text(output), "boom");
}

#[test]
fn run_probe_times_out_cpu_bound_script() {
    let plugin = test_plugin(
        r#"
        globalThis.__openusage_plugin = {
            probe() {
                while (true) {}
            }
        };
        "#,
    );

    let output = run_probe_with_timeout(
        &plugin,
        &temp_app_dir("timeout"),
        "0.0.0",
        Duration::from_millis(5),
    );

    assert_eq!(error_text(output), "probe timed out after 5ms");
}

#[test]
fn progress_resets_at_serializes_as_resets_at_camelcase() {
    let line = MetricLine::Progress {
        label: "Session".to_string(),
        used: 1.0,
        limit: 100.0,
        format: ProgressFormat::Percent,
        resets_at: Some("2099-01-01T00:00:00.000Z".to_string()),
        period_duration_ms: None,
        color: None,
    };

    let json: JsonValue = serde_json::to_value(&line).expect("serialize");
    let obj = json.as_object().expect("object");
    assert!(obj.get("resetsAt").is_some(), "expected resetsAt key");
    assert!(
        obj.get("resets_at").is_none(),
        "did not expect resets_at key"
    );
}

#[test]
fn bar_chart_line_round_trips_from_builder() {
    let plugin = test_plugin(
        r#"
        globalThis.__openusage_plugin = {
            probe(ctx) {
                return {
                    lines: [
                        ctx.line.barChart({
                            label: "Usage Trend",
                            points: [{ label: "Today", value: 42, valueLabel: "42 tokens" }],
                            note: "Estimated from local logs"
                        })
                    ]
                };
            }
        };
        "#,
    );

    let output = run_probe(&plugin, &temp_app_dir("bar-chart"), "0.0.0");
    let json: JsonValue = serde_json::to_value(&output.lines[0]).expect("serialize");
    assert_eq!(json["type"], "barChart");
    assert_eq!(json["label"], "Usage Trend");
    assert_eq!(json["points"][0]["valueLabel"], "42 tokens");
    assert_eq!(json["note"], "Estimated from local logs");
}

#[test]
fn bar_chart_caps_excessive_points() {
    // A plugin-controlled points array must not parse unbounded: this path
    // is native and runs after the JS deadline interrupt can fire.
    let plugin = test_plugin(
        r#"
        globalThis.__openusage_plugin = {
            probe(ctx) {
                var points = [];
                for (var i = 0; i < 5000; i++) {
                    points.push({ label: "d" + i, value: i });
                }
                return { lines: [ctx.line.barChart({ label: "Big", points: points })] };
            }
        };
        "#,
    );

    let output = run_probe(&plugin, &temp_app_dir("bar-chart-cap"), "0.0.0");
    let json: JsonValue = serde_json::to_value(&output.lines[0]).expect("serialize");
    assert_eq!(json["type"], "barChart");
    assert_eq!(
        json["points"].as_array().expect("points array").len(),
        MAX_BAR_CHART_POINTS
    );
}
