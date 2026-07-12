use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::Emitter;
use uuid::Uuid;

use crate::{AppState, local_http_api, plugin_engine};

const MAX_CONCURRENT_PROBES: usize = 4;

fn probe_worker_count(plugin_count: usize) -> usize {
    plugin_count.min(MAX_CONCURRENT_PROBES)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeBatchStarted {
    pub batch_id: String,
    pub plugin_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeResult {
    pub batch_id: String,
    pub output: plugin_engine::runtime::PluginOutput,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeBatchComplete {
    pub batch_id: String,
}

#[tauri::command]
pub(crate) async fn start_probe_batch(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Mutex<AppState>>,
    batch_id: Option<String>,
    plugin_ids: Option<Vec<String>>,
) -> Result<ProbeBatchStarted, String> {
    let batch_id = batch_id
        .and_then(|id| {
            let trimmed = id.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        })
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let (plugins, app_data_dir, app_version) = {
        let locked = state.lock().map_err(|e| e.to_string())?;
        (
            locked.plugins.clone(),
            locked.app_data_dir.clone(),
            locked.app_version.clone(),
        )
    };

    let selected_plugins = match plugin_ids {
        Some(ids) => {
            let mut by_id: HashMap<String, plugin_engine::manifest::LoadedPlugin> = plugins
                .into_iter()
                .map(|plugin| (plugin.manifest.id.clone(), plugin))
                .collect();
            let mut seen = HashSet::new();
            ids.into_iter()
                .filter_map(|id| {
                    if !seen.insert(id.clone()) {
                        return None;
                    }
                    by_id.remove(&id)
                })
                .collect()
        }
        None => plugins,
    };

    let response_plugin_ids: Vec<String> = selected_plugins
        .iter()
        .map(|plugin| plugin.manifest.id.clone())
        .collect();

    log::info!(
        "probe batch {} starting: {:?}",
        batch_id,
        response_plugin_ids
    );

    if selected_plugins.is_empty() {
        let _ = app_handle.emit(
            "probe:batch-complete",
            ProbeBatchComplete {
                batch_id: batch_id.clone(),
            },
        );
        return Ok(ProbeBatchStarted {
            batch_id,
            plugin_ids: response_plugin_ids,
        });
    }

    let selected_count = selected_plugins.len();
    let worker_count = probe_worker_count(selected_count);
    if worker_count < selected_count {
        log::info!(
            "probe batch {} using {} workers for {} plugins",
            batch_id,
            worker_count,
            selected_count
        );
    }

    let remaining = Arc::new(AtomicUsize::new(selected_count));
    let probe_queue = Arc::new(Mutex::new(
        selected_plugins.into_iter().collect::<VecDeque<_>>(),
    ));

    for _ in 0..worker_count {
        let handle = app_handle.clone();
        let completion_handle = app_handle.clone();
        let bid = batch_id.clone();
        let completion_bid = batch_id.clone();
        let data_dir = app_data_dir.clone();
        let version = app_version.clone();
        let counter = Arc::clone(&remaining);
        let queue = Arc::clone(&probe_queue);

        tauri::async_runtime::spawn_blocking(move || {
            loop {
                let plugin = {
                    let mut queue = queue
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    queue.pop_front()
                };

                let Some(plugin) = plugin else {
                    break;
                };

                let plugin_id = plugin.manifest.id.clone();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    plugin_engine::runtime::run_probe(&plugin, &data_dir, &version)
                }));

                match result {
                    Ok(output) => {
                        let has_error = output.lines.iter().any(|line| {
                            matches!(
                                line,
                                plugin_engine::runtime::MetricLine::Badge { label, .. }
                                    if label == "Error"
                            )
                        });
                        if has_error {
                            log::warn!("probe {} completed with error", plugin_id);
                        } else {
                            log::info!(
                                "probe {} completed ok ({} lines)",
                                plugin_id,
                                output.lines.len()
                            );
                            local_http_api::cache_successful_output(&output);
                        }
                        let _ = handle.emit(
                            "probe:result",
                            ProbeResult {
                                batch_id: bid.clone(),
                                output,
                            },
                        );
                    }
                    Err(_) => {
                        log::error!("probe {} panicked", plugin_id);
                    }
                }

                if counter.fetch_sub(1, Ordering::SeqCst) == 1 {
                    log::info!("probe batch {} complete", completion_bid);
                    let _ = completion_handle.emit(
                        "probe:batch-complete",
                        ProbeBatchComplete {
                            batch_id: completion_bid.clone(),
                        },
                    );
                }
            }
        });
    }

    Ok(ProbeBatchStarted {
        batch_id,
        plugin_ids: response_plugin_ids,
    })
}

#[cfg(test)]
mod tests {
    use super::{MAX_CONCURRENT_PROBES, probe_worker_count};

    #[test]
    fn probe_worker_count_is_bounded() {
        assert_eq!(probe_worker_count(0), 0);
        assert_eq!(probe_worker_count(1), 1);
        assert_eq!(
            probe_worker_count(MAX_CONCURRENT_PROBES),
            MAX_CONCURRENT_PROBES
        );
        assert_eq!(
            probe_worker_count(MAX_CONCURRENT_PROBES + 1),
            MAX_CONCURRENT_PROBES
        );
    }
}
