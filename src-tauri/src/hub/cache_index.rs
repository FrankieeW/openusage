use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{PluginInfo, SkippedPlugin};

const CACHE_INDEX_SCHEMA_VERSION: u32 = 1;
pub const CACHE_INDEX_FILENAME: &str = "hub-cache-index.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedPluginSummary {
    pub id: String,
    pub name: String,
    pub brand_color: Option<String>,
    pub icon_data_url: Option<String>,
    pub available_version: String,
    pub updated_at: Option<i64>,
    pub package_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheIndex {
    pub schema_version: u32,
    pub source_id: String,
    pub commit_sha: String,
    pub plugin_filter: Option<Vec<String>>,
    pub generated_at: i64,
    pub plugins: Vec<CachedPluginSummary>,
    pub skipped: Vec<SkippedPlugin>,
}

pub fn load(
    cache_dir: &Path,
    source_id: &str,
    commit_sha: &str,
    plugin_filter: Option<&[String]>,
) -> Option<CacheIndex> {
    let path = cache_dir.join(CACHE_INDEX_FILENAME);
    let text = std::fs::read_to_string(path).ok()?;
    let index = serde_json::from_str::<CacheIndex>(&text).ok()?;
    if index.schema_version != CACHE_INDEX_SCHEMA_VERSION {
        return None;
    }
    if index.source_id != source_id || index.commit_sha != commit_sha {
        return None;
    }
    if index.plugin_filter != plugin_filter.map(|filter| filter.to_vec()) {
        return None;
    }
    Some(index)
}

pub fn build(
    source_id: &str,
    commit_sha: &str,
    plugin_filter: Option<&[String]>,
    generated_at: i64,
    plugins: &[PluginInfo],
    skipped: &[SkippedPlugin],
) -> CacheIndex {
    CacheIndex {
        schema_version: CACHE_INDEX_SCHEMA_VERSION,
        source_id: source_id.to_string(),
        commit_sha: commit_sha.to_string(),
        plugin_filter: plugin_filter.map(|filter| filter.to_vec()),
        generated_at,
        plugins: plugins.iter().map(CachedPluginSummary::from).collect(),
        skipped: skipped.to_vec(),
    }
}

pub fn write(cache_dir: &Path, index: &CacheIndex) -> Result<(), String> {
    std::fs::create_dir_all(cache_dir).map_err(|err| err.to_string())?;
    let text = serde_json::to_string_pretty(index).map_err(|err| err.to_string())?;
    let tmp = cache_dir.join(format!("{}.tmp", CACHE_INDEX_FILENAME));
    let path = cache_dir.join(CACHE_INDEX_FILENAME);
    std::fs::write(&tmp, text).map_err(|err| err.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|err| err.to_string())
}

impl From<&PluginInfo> for CachedPluginSummary {
    fn from(plugin: &PluginInfo) -> Self {
        Self {
            id: plugin.id.clone(),
            name: plugin.name.clone(),
            brand_color: plugin.brand_color.clone(),
            icon_data_url: plugin.icon_data_url.clone(),
            available_version: plugin.available_version.clone(),
            updated_at: plugin.updated_at,
            package_hash: plugin.package_hash.clone(),
        }
    }
}
