use super::*;
use crate::plugin_engine::runtime::{MetricLine, PluginOutput, ProgressFormat};
use serial_test::serial;
use std::time::Instant;

fn make_snapshot(id: &str, name: &str) -> CachedPluginSnapshot {
    CachedPluginSnapshot {
        provider_id: id.to_string(),
        display_name: name.to_string(),
        plan: Some("Pro".to_string()),
        lines: vec![],
        fetched_at: "2026-03-26T08:15:30Z".to_string(),
    }
}

fn make_output(id: &str, name: &str) -> PluginOutput {
    PluginOutput {
        provider_id: id.to_string(),
        display_name: name.to_string(),
        plan: Some("Pro".to_string()),
        lines: vec![MetricLine::Text {
            label: "Usage".to_string(),
            value: "42%".to_string(),
            color: None,
            subtitle: None,
        }],
        icon_url: String::new(),
    }
}

fn temp_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "openusage-test-{}-{}",
        label,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn wait_for_cached_snapshots(
    dir: &Path,
    expected_len: usize,
) -> HashMap<String, CachedPluginSnapshot> {
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        let loaded = load_cache(dir);
        if loaded.len() == expected_len {
            return loaded;
        }
        assert!(
            Instant::now() < deadline,
            "cache file was not flushed within the test deadline"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn wait_for_cache_writer_idle() {
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        let state = cache_state().lock().unwrap();
        if !state.flush_scheduled && state.dirty_generation == state.flushed_generation {
            return;
        }
        drop(state);
        assert!(
            Instant::now() < deadline,
            "debounced cache writer did not return to idle"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn cache_write_retry_delay_backs_off_and_caps() {
    assert_eq!(
        cache_write_retry_delay(1),
        CACHE_WRITE_DEBOUNCE.saturating_mul(2)
    );
    assert_eq!(
        cache_write_retry_delay(2),
        CACHE_WRITE_DEBOUNCE.saturating_mul(4)
    );
    assert_eq!(cache_write_retry_delay(20), CACHE_WRITE_RETRY_MAX_DELAY);
}

#[test]
fn cache_write_failure_logs_are_throttled() {
    assert!(should_log_cache_write_failure(1));
    assert!(should_log_cache_write_failure(2));
    assert!(!should_log_cache_write_failure(3));
    assert!(should_log_cache_write_failure(4));
    assert!(!should_log_cache_write_failure(5));
    assert!(should_log_cache_write_failure(16));
}

#[test]
fn snapshot_serializes_with_fetched_at() {
    let snap = make_snapshot("claude", "Claude");
    let json: serde_json::Value = serde_json::to_value(&snap).unwrap();
    assert!(json.get("fetchedAt").is_some());
    assert!(json.get("fetched_at").is_none());
    assert_eq!(json["fetchedAt"], "2026-03-26T08:15:30Z");
}

#[test]
fn cache_file_round_trip() {
    let dir = temp_dir("cache");
    std::fs::create_dir_all(&dir).unwrap();

    let mut snapshots = HashMap::new();
    snapshots.insert("claude".to_string(), make_snapshot("claude", "Claude"));

    save_cache(&dir, &snapshots).unwrap();
    let loaded = load_cache(&dir);

    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded["claude"].provider_id, "claude");
    assert_eq!(loaded["claude"].fetched_at, "2026-03-26T08:15:30Z");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn load_cache_returns_empty_on_missing_file() {
    let dir = temp_dir("no-cache");
    let loaded = load_cache(&dir);
    assert!(loaded.is_empty());
}

#[test]
fn load_cache_returns_empty_on_invalid_json() {
    let dir = temp_dir("bad-cache");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(CACHE_FILE_NAME), "not json").unwrap();

    let loaded = load_cache(&dir);
    assert!(loaded.is_empty());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[serial]
fn cache_successful_output_debounces_disk_writes() {
    let dir = temp_dir("debounced-cache");
    std::fs::create_dir_all(&dir).unwrap();

    init(&dir, vec!["claude".to_string(), "codex".to_string()]);
    cache_successful_output(&make_output("claude", "Claude"));
    cache_successful_output(&make_output("codex", "Codex"));

    {
        let state = cache_state().lock().unwrap();
        assert!(state.flush_scheduled);
        assert_eq!(state.dirty_generation, 2);
        assert_eq!(state.flushed_generation, 0);
    }
    assert!(
        !dir.join(CACHE_FILE_NAME).exists(),
        "cache should not be written synchronously for every result"
    );

    let loaded = wait_for_cached_snapshots(&dir, 2);
    assert_eq!(loaded["claude"].display_name, "Claude");
    assert_eq!(loaded["codex"].display_name, "Codex");

    wait_for_cache_writer_idle();

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[serial]
fn flush_cache_persists_pending_write_synchronously() {
    let dir = temp_dir("flush-cache");
    std::fs::create_dir_all(&dir).unwrap();

    init(&dir, vec!["claude".to_string()]);
    cache_successful_output(&make_output("claude", "Claude"));
    assert!(
        !dir.join(CACHE_FILE_NAME).exists(),
        "cache write should be pending before explicit flush"
    );

    flush_cache();

    let loaded = load_cache(&dir);
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded["claude"].display_name, "Claude");

    wait_for_cache_writer_idle();

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[serial]
fn failed_cache_write_stays_pending_for_retry() {
    let dir = temp_dir("cache-write-retry");

    init(&dir, vec!["claude".to_string()]);
    {
        let mut state = cache_state().lock().unwrap();
        state
            .snapshots
            .insert("claude".to_string(), make_snapshot("claude", "Claude"));
        state.dirty_generation = 1;
        state.flushed_generation = 0;
        state.flush_scheduled = true;
    }

    assert!(matches!(
        flush_pending_cache_once(),
        CacheFlushResult::Failed(_)
    ));
    {
        let state = cache_state().lock().unwrap();
        assert_eq!(state.dirty_generation, 1);
        assert_eq!(state.flushed_generation, 0);
        assert!(state.flush_scheduled);
    }

    std::fs::create_dir_all(&dir).unwrap();
    assert_eq!(flush_pending_cache_once(), CacheFlushResult::Flushed);

    let loaded = load_cache(&dir);
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded["claude"].display_name, "Claude");

    assert_eq!(flush_pending_cache_once(), CacheFlushResult::Idle);
    {
        let state = cache_state().lock().unwrap();
        assert_eq!(state.dirty_generation, 1);
        assert_eq!(state.flushed_generation, 1);
        assert!(!state.flush_scheduled);
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn snapshot_with_progress_line_round_trips() {
    let snap = CachedPluginSnapshot {
        provider_id: "claude".to_string(),
        display_name: "Claude".to_string(),
        plan: Some("Max 20x".to_string()),
        lines: vec![crate::plugin_engine::runtime::MetricLine::Progress {
            label: "Session".to_string(),
            used: 42.0,
            limit: 100.0,
            format: ProgressFormat::Percent,
            resets_at: Some("2026-03-26T12:00:00Z".to_string()),
            period_duration_ms: Some(14400000),
            color: None,
        }],
        fetched_at: "2026-03-26T08:00:00Z".to_string(),
    };

    let json = serde_json::to_string(&snap).unwrap();
    let deserialized: CachedPluginSnapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.provider_id, "claude");
    assert_eq!(deserialized.lines.len(), 1);
}
