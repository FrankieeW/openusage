use serde::{Deserialize, Serialize};

use super::Source;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub brand_color: Option<String>,
    pub icon_data_url: Option<String>,
    pub source_id: String,
    pub installed: bool,
    pub installed_source_id: Option<String>,
    pub unmanaged: bool,
    pub installed_version: Option<String>,
    pub available_version: String,
    pub updated_at: Option<i64>,
    pub package_hash: String,
    pub package_status: PackageStatus,
    pub update_available: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PackageStatus {
    NotInstalled,
    Installed,
    UpdateAvailable,
    SourceChanged,
    InstalledNewerThanSource,
    SamePackageFromOtherSource,
    DifferentPackageSamePluginId,
    UnmanagedInstalled,
    OrphanedSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkippedPlugin {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceSnapshot {
    pub branch: Option<String>,
    pub commit_sha: Option<String>,
    pub checked_at: i64,
    pub discovered_count: usize,
    pub skipped_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HubBrowseView {
    pub source: Source,
    pub available: Vec<PluginInfo>,
    pub skipped: Vec<SkippedPlugin>,
    pub snapshot: SourceSnapshot,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub source_id: String,
    pub plugin_id: String,
    pub from: String,
    pub to: String,
    pub package_hash: String,
}

/// What the JS Hub install side knows about an already-installed plugin.
#[derive(Debug, Clone)]
pub struct InstalledLookupEntry {
    pub source_id: String,
    #[allow(
        dead_code,
        reason = "retain installed source provenance with the lookup entry"
    )]
    pub source_url: String,
    pub version: String,
    pub package_hash: String,
}

pub type InstalledLookup<'a> = std::collections::HashMap<String, InstalledLookupEntry>;
