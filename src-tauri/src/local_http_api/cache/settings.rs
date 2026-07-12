use super::{CacheState, CachedPluginSnapshot};
use serde::Deserialize;
use std::collections::HashSet;
use std::path::Path;

const SETTINGS_FILE_NAME: &str = "settings.json";
const DEFAULT_ENABLED_PLUGINS: &[&str] = &["claude", "codex", "cursor"];

#[derive(Deserialize)]
struct SettingsFile {
    plugins: Option<PluginSettingsJson>,
}

#[derive(Deserialize)]
struct PluginSettingsJson {
    order: Option<Vec<String>>,
    disabled: Option<Vec<String>>,
}

fn read_plugin_settings(app_data_dir: &Path) -> (Vec<String>, HashSet<String>, bool) {
    let path = app_data_dir.join(SETTINGS_FILE_NAME);
    let data = match std::fs::read_to_string(&path) {
        Ok(d) => d,
        Err(_) => return (Vec::new(), HashSet::new(), false),
    };
    match serde_json::from_str::<SettingsFile>(&data) {
        Ok(sf) => {
            let ps = sf.plugins.unwrap_or(PluginSettingsJson {
                order: None,
                disabled: None,
            });
            let has_settings = ps.order.is_some() || ps.disabled.is_some();
            let order = ps.order.unwrap_or_default();
            let disabled: HashSet<String> = ps.disabled.unwrap_or_default().into_iter().collect();
            (order, disabled, has_settings)
        }
        Err(_) => (Vec::new(), HashSet::new(), false),
    }
}

pub(super) fn enabled_snapshots_ordered(state: &CacheState) -> Vec<CachedPluginSnapshot> {
    let (settings_order, disabled, has_settings) = read_plugin_settings(&state.app_data_dir);

    let default_enabled: HashSet<&str> = DEFAULT_ENABLED_PLUGINS.iter().copied().collect();

    let is_enabled = |id: &str| -> bool {
        if has_settings {
            !disabled.contains(id)
        } else {
            default_enabled.contains(id)
        }
    };

    // Build ordered plugin ids: settings order first, then remaining known ids.
    let mut ordered: Vec<String> = Vec::new();
    let mut seen = HashSet::new();
    for id in &settings_order {
        if seen.insert(id.clone()) {
            ordered.push(id.clone());
        }
    }
    for id in &state.known_plugin_ids {
        if seen.insert(id.clone()) {
            ordered.push(id.clone());
        }
    }

    ordered
        .into_iter()
        .filter(|id| is_enabled(id))
        .filter_map(|id| state.snapshots.get(&id).cloned())
        .collect()
}
