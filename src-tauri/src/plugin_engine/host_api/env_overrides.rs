use super::*;

pub(super) const WHITELISTED_ENV_VARS: [&str; 17] = [
    "CODEX_HOME",
    "CLAUDE_CONFIG_DIR",
    "CLAUDE_CODE_OAUTH_TOKEN",
    "USER_TYPE",
    "USE_STAGING_OAUTH",
    "USE_LOCAL_OAUTH",
    "CLAUDE_CODE_CUSTOM_OAUTH_URL",
    "CLAUDE_CODE_OAUTH_CLIENT_ID",
    "CLAUDE_LOCAL_OAUTH_API_BASE",
    "ZAI_API_KEY",
    "GLM_API_KEY",
    "MINIMAX_API_KEY",
    "MINIMAX_API_TOKEN",
    "MINIMAX_CN_API_KEY",
    "SYNTHETIC_API_KEY",
    "PI_CODING_AGENT_DIR",
    "DEEPSEEK_API_KEY",
];

// Unsafe escape hatch: when enabled (via the Settings toggle), plugins can read
// *any* environment variable, bypassing WHITELISTED_ENV_VARS. Off by default.
// A malicious or buggy plugin could then exfiltrate arbitrary secrets, so this
// is opt-in only. Synced from `settings.json` at startup and on change.
pub(super) static ALLOW_ALL_ENV: AtomicBool = AtomicBool::new(false);

/// Toggle the unsafe "read all env vars" mode. Called from the Tauri command
/// and from startup when reading persisted settings.
pub fn set_allow_all_env(enabled: bool) {
    ALLOW_ALL_ENV.store(enabled, Ordering::Relaxed);
}

pub(super) fn allow_all_env() -> bool {
    ALLOW_ALL_ENV.load(Ordering::Relaxed)
}

// User-defined env overrides, managed from the Env page and synced via the
// `set_env_overrides` Tauri command. Overrides take precedence over the real
// environment and bypass WHITELISTED_ENV_VARS for the names they define.
pub(super) static ENV_OVERRIDES: OnceLock<Mutex<HashMap<String, EnvOverride>>> = OnceLock::new();

pub(super) fn env_overrides() -> &'static Mutex<HashMap<String, EnvOverride>> {
    ENV_OVERRIDES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Replace the entire override table. Called from the Tauri command and at
/// startup when reading persisted settings.
pub fn set_env_overrides(inputs: Vec<EnvOverrideInput>) {
    let mut map = HashMap::with_capacity(inputs.len());
    for input in inputs {
        map.insert(
            input.name,
            EnvOverride {
                kind: input.kind,
                value: input.value,
            },
        );
    }
    let mut guard = env_overrides().lock().unwrap_or_else(|poisoned| {
        log::error!("[env_overrides] mutex poisoned, recovering");
        poisoned.into_inner()
    });
    *guard = map;
}

/// Pure resolution used by the plugin `env.get` host function. `resolve` is the
/// real-environment lookup (injected so it can be stubbed in tests).
pub(super) fn resolve_env_for_plugin<F>(
    name: &str,
    allow_all: bool,
    overrides: &HashMap<String, EnvOverride>,
    resolve: F,
) -> Option<String>
where
    F: Fn(&str) -> Option<String>,
{
    if let Some(override_entry) = overrides.get(name) {
        return match override_entry.kind {
            EnvOverrideKind::Literal => Some(override_entry.value.clone()),
            // Reference resolves its target from the REAL environment only — it
            // does not chain into another override.
            EnvOverrideKind::Reference => resolve(&override_entry.value),
        };
    }

    if !allow_all && !WHITELISTED_ENV_VARS.contains(&name) {
        return None;
    }
    resolve(name)
}
