use super::*;

pub(in crate::plugin_engine::host_api) const CCUSAGE_VERSION: &str = "20.0.2";
pub(in crate::plugin_engine::host_api) const CCUSAGE_PACKAGE_NAME: &str = "ccusage";
pub(in crate::plugin_engine::host_api) const CCUSAGE_BIN_NAME: &str = "ccusage";
pub(in crate::plugin_engine::host_api) const CCUSAGE_LEGACY_VERSION: &str = "18.0.11";
pub(in crate::plugin_engine::host_api) const CCUSAGE_LEGACY_CLAUDE_PACKAGE_NAME: &str = "ccusage";
pub(in crate::plugin_engine::host_api) const CCUSAGE_LEGACY_CODEX_PACKAGE_NAME: &str =
    "@ccusage/codex";
pub(in crate::plugin_engine::host_api) const CCUSAGE_LEGACY_CODEX_BIN_NAME: &str = "ccusage-codex";
pub(in crate::plugin_engine::host_api) const CCUSAGE_TIMEOUT_SECS: u64 = 15;
pub(in crate::plugin_engine::host_api) const CCUSAGE_POLL_INTERVAL_MS: u64 = 100;

#[derive(Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::plugin_engine::host_api) struct CcusageQueryOpts {
    pub(in crate::plugin_engine::host_api) provider: Option<String>,
    pub(in crate::plugin_engine::host_api) since: Option<String>,
    pub(in crate::plugin_engine::host_api) until: Option<String>,
    pub(in crate::plugin_engine::host_api) home_path: Option<String>,
    pub(in crate::plugin_engine::host_api) claude_path: Option<String>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub(in crate::plugin_engine::host_api) enum CcusageProvider {
    Claude,
    Codex,
}

pub(in crate::plugin_engine::host_api) static CCUSAGE_ACTIVE_PROVIDERS: OnceLock<
    Mutex<HashSet<CcusageProvider>>,
> = OnceLock::new();

pub(in crate::plugin_engine::host_api) struct CcusageQueryGuard {
    pub(in crate::plugin_engine::host_api) provider: CcusageProvider,
}

impl CcusageQueryGuard {
    pub(in crate::plugin_engine::host_api) fn acquire(provider: CcusageProvider) -> Option<Self> {
        let active = CCUSAGE_ACTIVE_PROVIDERS.get_or_init(|| Mutex::new(HashSet::new()));
        let mut active = active.lock().unwrap_or_else(|err| err.into_inner());
        if !active.insert(provider) {
            return None;
        }
        Some(Self { provider })
    }
}

impl Drop for CcusageQueryGuard {
    fn drop(&mut self) {
        let active = CCUSAGE_ACTIVE_PROVIDERS.get_or_init(|| Mutex::new(HashSet::new()));
        let mut active = active.lock().unwrap_or_else(|err| err.into_inner());
        active.remove(&self.provider);
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(in crate::plugin_engine::host_api) enum CcusageRunnerKind {
    Bunx,
    PnpmDlx,
    YarnDlx,
    NpmExec,
    Npx,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(in crate::plugin_engine::host_api) enum CcusageCommandFlavor {
    Current,
    Legacy,
}

pub(in crate::plugin_engine::host_api) fn ccusage_runner_order() -> [CcusageRunnerKind; 5] {
    [
        CcusageRunnerKind::Bunx,
        CcusageRunnerKind::PnpmDlx,
        CcusageRunnerKind::YarnDlx,
        CcusageRunnerKind::NpmExec,
        CcusageRunnerKind::Npx,
    ]
}

pub(in crate::plugin_engine::host_api) fn ccusage_runner_label(
    kind: CcusageRunnerKind,
) -> &'static str {
    match kind {
        CcusageRunnerKind::Bunx => "bunx",
        CcusageRunnerKind::PnpmDlx => "pnpm dlx",
        CcusageRunnerKind::YarnDlx => "yarn dlx",
        CcusageRunnerKind::NpmExec => "npm exec",
        CcusageRunnerKind::Npx => "npx",
    }
}

#[derive(Copy, Clone)]
pub(in crate::plugin_engine::host_api) struct CcusageProviderConfig {
    pub(in crate::plugin_engine::host_api) command_namespace: &'static str,
    pub(in crate::plugin_engine::host_api) home_env_var: &'static str,
}

pub(in crate::plugin_engine::host_api) fn parse_ccusage_provider(
    value: &str,
) -> Option<CcusageProvider> {
    match value.trim().to_ascii_lowercase().as_str() {
        "claude" => Some(CcusageProvider::Claude),
        "codex" => Some(CcusageProvider::Codex),
        _ => None,
    }
}

pub(in crate::plugin_engine::host_api) fn infer_ccusage_provider(
    plugin_id: &str,
) -> Option<CcusageProvider> {
    parse_ccusage_provider(plugin_id)
}

pub(in crate::plugin_engine::host_api) fn resolve_ccusage_provider(
    opts: &CcusageQueryOpts,
    plugin_id: &str,
) -> CcusageProvider {
    opts.provider
        .as_deref()
        .and_then(parse_ccusage_provider)
        .or_else(|| infer_ccusage_provider(plugin_id))
        .unwrap_or(CcusageProvider::Claude)
}

pub(in crate::plugin_engine::host_api) fn ccusage_provider_config(
    provider: CcusageProvider,
) -> CcusageProviderConfig {
    match provider {
        CcusageProvider::Claude => CcusageProviderConfig {
            command_namespace: "claude",
            home_env_var: "CLAUDE_CONFIG_DIR",
        },
        CcusageProvider::Codex => CcusageProviderConfig {
            command_namespace: "codex",
            home_env_var: "CODEX_HOME",
        },
    }
}

pub(in crate::plugin_engine::host_api) fn ccusage_package_spec() -> String {
    format!("{}@{}", CCUSAGE_PACKAGE_NAME, CCUSAGE_VERSION)
}

pub(in crate::plugin_engine::host_api) fn ccusage_legacy_package_spec(
    provider: CcusageProvider,
) -> String {
    let package_name = match provider {
        CcusageProvider::Claude => CCUSAGE_LEGACY_CLAUDE_PACKAGE_NAME,
        CcusageProvider::Codex => CCUSAGE_LEGACY_CODEX_PACKAGE_NAME,
    };
    format!("{}@{}", package_name, CCUSAGE_LEGACY_VERSION)
}

pub(in crate::plugin_engine::host_api) fn ccusage_home_override(
    opts: &CcusageQueryOpts,
    provider: CcusageProvider,
) -> Option<&str> {
    if let Some(home_path) = opts
        .home_path
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return Some(home_path);
    }

    match provider {
        CcusageProvider::Claude => opts
            .claude_path
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty()),
        CcusageProvider::Codex => None,
    }
}
