use crate::plugin_engine::runtime::{MetricLine, PluginOutput};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

mod settings;

const CACHE_FILE_NAME: &str = "usage-api-cache.json";

#[cfg(not(test))]
const CACHE_WRITE_DEBOUNCE: Duration = Duration::from_millis(500);
#[cfg(test)]
const CACHE_WRITE_DEBOUNCE: Duration = Duration::from_millis(10);
const CACHE_WRITE_RETRY_MAX_DELAY: Duration = Duration::from_secs(30);

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedPluginSnapshot {
    pub provider_id: String,
    pub display_name: String,
    pub plan: Option<String>,
    pub lines: Vec<MetricLine>,
    pub fetched_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UsageApiCacheFile {
    version: u32,
    snapshots: HashMap<String, CachedPluginSnapshot>,
}

pub(super) struct CacheState {
    pub snapshots: HashMap<String, CachedPluginSnapshot>,
    pub app_data_dir: PathBuf,
    pub known_plugin_ids: Vec<String>,
    dirty_generation: u64,
    flushed_generation: u64,
    flush_scheduled: bool,
}

#[derive(Debug, PartialEq, Eq)]
enum CacheFlushResult {
    Idle,
    Flushed,
    Failed(String),
}

// ---------------------------------------------------------------------------
// Global cache state (same pattern as managed_shortcut_slot in lib.rs)
// ---------------------------------------------------------------------------

pub(super) fn cache_state() -> &'static Mutex<CacheState> {
    static STATE: OnceLock<Mutex<CacheState>> = OnceLock::new();
    STATE.get_or_init(|| {
        Mutex::new(CacheState {
            snapshots: HashMap::new(),
            app_data_dir: PathBuf::new(),
            known_plugin_ids: Vec::new(),
            dirty_generation: 0,
            flushed_generation: 0,
            flush_scheduled: false,
        })
    })
}

fn cache_write_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

// ---------------------------------------------------------------------------
// Cache persistence
// ---------------------------------------------------------------------------

pub fn load_cache(app_data_dir: &Path) -> HashMap<String, CachedPluginSnapshot> {
    let path = app_data_dir.join(CACHE_FILE_NAME);
    let data = match std::fs::read_to_string(&path) {
        Ok(d) => d,
        Err(_) => return HashMap::new(),
    };
    match serde_json::from_str::<UsageApiCacheFile>(&data) {
        Ok(file) if file.version == 1 => file.snapshots,
        Ok(_) => {
            log::warn!("usage-api-cache.json has unsupported version, starting empty");
            HashMap::new()
        }
        Err(e) => {
            log::warn!(
                "failed to parse usage-api-cache.json: {}, starting empty",
                e
            );
            HashMap::new()
        }
    }
}

fn save_cache(
    app_data_dir: &Path,
    snapshots: &HashMap<String, CachedPluginSnapshot>,
) -> Result<(), String> {
    let file = UsageApiCacheFile {
        version: 1,
        snapshots: snapshots.clone(),
    };
    let path = app_data_dir.join(CACHE_FILE_NAME);
    let tmp_path = app_data_dir.join(".usage-api-cache.json.tmp");
    let json = serde_json::to_string(&file)
        .map_err(|e| format!("failed to serialize usage cache: {}", e))?;
    std::fs::write(&tmp_path, &json)
        .map_err(|e| format!("failed to write temp cache file: {}", e))?;
    std::fs::rename(&tmp_path, &path).map_err(|e| format!("failed to rename cache file: {}", e))?;
    Ok(())
}

fn schedule_cache_flush_locked(state: &mut CacheState) {
    if state.flush_scheduled {
        return;
    }

    state.flush_scheduled = true;
    std::thread::spawn(debounced_cache_flush_worker);
}

fn debounced_cache_flush_worker() {
    let mut consecutive_failures = 0_u32;
    let mut retry_delay = CACHE_WRITE_DEBOUNCE;

    loop {
        std::thread::sleep(retry_delay);

        match flush_pending_cache_once() {
            CacheFlushResult::Idle => return,
            CacheFlushResult::Flushed => {
                if consecutive_failures > 0 {
                    log::info!(
                        "usage-api-cache.json write recovered after {} failed attempts",
                        consecutive_failures
                    );
                }
                consecutive_failures = 0;
                retry_delay = CACHE_WRITE_DEBOUNCE;
            }
            CacheFlushResult::Failed(e) => {
                consecutive_failures = consecutive_failures.saturating_add(1);
                retry_delay = cache_write_retry_delay(consecutive_failures);
                if should_log_cache_write_failure(consecutive_failures) {
                    log::warn!(
                        "{}; retrying in {:?} (consecutive failures: {})",
                        e,
                        retry_delay,
                        consecutive_failures
                    );
                }
            }
        }
    }
}

fn cache_write_retry_delay(consecutive_failures: u32) -> Duration {
    let factor = 1_u32 << consecutive_failures.min(16);
    std::cmp::min(
        CACHE_WRITE_DEBOUNCE.saturating_mul(factor),
        CACHE_WRITE_RETRY_MAX_DELAY,
    )
}

fn should_log_cache_write_failure(consecutive_failures: u32) -> bool {
    consecutive_failures == 1 || consecutive_failures.is_power_of_two()
}

fn pending_cache_write() -> Option<(u64, PathBuf, HashMap<String, CachedPluginSnapshot>)> {
    let mut state = cache_state().lock().expect("cache state poisoned");
    if state.dirty_generation == state.flushed_generation {
        state.flush_scheduled = false;
        return None;
    }

    Some((
        state.dirty_generation,
        state.app_data_dir.clone(),
        state.snapshots.clone(),
    ))
}

fn mark_cache_flushed(generation: u64) {
    let mut state = cache_state().lock().expect("cache state poisoned");
    state.flushed_generation = generation;
}

fn flush_pending_cache_once() -> CacheFlushResult {
    let _write_guard = cache_write_lock()
        .lock()
        .expect("cache write lock poisoned");
    let Some((generation, app_data_dir, snapshots)) = pending_cache_write() else {
        return CacheFlushResult::Idle;
    };

    match save_cache(&app_data_dir, &snapshots) {
        Ok(()) => {
            mark_cache_flushed(generation);
            CacheFlushResult::Flushed
        }
        Err(e) => CacheFlushResult::Failed(e),
    }
}

// ---------------------------------------------------------------------------
// Public API: initialise + update cache
// ---------------------------------------------------------------------------

pub fn init(app_data_dir: &Path, known_plugin_ids: Vec<String>) {
    let snapshots = load_cache(app_data_dir);
    let mut state = cache_state().lock().expect("cache state poisoned");
    state.snapshots = snapshots;
    state.app_data_dir = app_data_dir.to_path_buf();
    state.known_plugin_ids = known_plugin_ids;
    state.dirty_generation = 0;
    state.flushed_generation = 0;
    state.flush_scheduled = false;
}

pub fn cache_successful_output(output: &PluginOutput) {
    let fetched_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default();

    let snapshot = CachedPluginSnapshot {
        provider_id: output.provider_id.clone(),
        display_name: output.display_name.clone(),
        plan: output.plan.clone(),
        lines: output.lines.clone(),
        fetched_at,
    };

    let mut state = cache_state().lock().expect("cache state poisoned");
    state.snapshots.insert(output.provider_id.clone(), snapshot);
    state.dirty_generation = state.dirty_generation.wrapping_add(1);
    schedule_cache_flush_locked(&mut state);
}

pub fn flush_cache() {
    if let CacheFlushResult::Failed(e) = flush_pending_cache_once() {
        log::warn!("{}", e);
    }
}

/// Build the ordered list of enabled cached snapshots for GET /v1/usage.
pub(super) fn enabled_snapshots_ordered(state: &CacheState) -> Vec<CachedPluginSnapshot> {
    settings::enabled_snapshots_ordered(state)
}

#[cfg(test)]
mod tests;
