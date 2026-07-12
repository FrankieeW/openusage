use super::*;

pub(in crate::plugin_engine::host_api) fn ccusage_runner_candidates(
    kind: CcusageRunnerKind,
) -> Vec<String> {
    let mut candidates: Vec<String> = Vec::new();
    match kind {
        CcusageRunnerKind::Bunx => {
            if let Some(home) = dirs::home_dir() {
                candidates.push(home.join(".bun/bin/bunx").to_string_lossy().to_string());
            }
            candidates.extend(
                ["/opt/homebrew/bin/bunx", "/usr/local/bin/bunx", "bunx"]
                    .into_iter()
                    .map(str::to_string),
            );
        }
        CcusageRunnerKind::PnpmDlx => {
            candidates.extend(
                ["/opt/homebrew/bin/pnpm", "/usr/local/bin/pnpm", "pnpm"]
                    .into_iter()
                    .map(str::to_string),
            );
        }
        CcusageRunnerKind::YarnDlx => {
            candidates.extend(
                ["/opt/homebrew/bin/yarn", "/usr/local/bin/yarn", "yarn"]
                    .into_iter()
                    .map(str::to_string),
            );
        }
        CcusageRunnerKind::NpmExec => {
            candidates.extend(
                ["/opt/homebrew/bin/npm", "/usr/local/bin/npm", "npm"]
                    .into_iter()
                    .map(str::to_string),
            );
        }
        CcusageRunnerKind::Npx => {
            candidates.extend(
                ["/opt/homebrew/bin/npx", "/usr/local/bin/npx", "npx"]
                    .into_iter()
                    .map(str::to_string),
            );
        }
    }

    let mut unique = Vec::new();
    for candidate in candidates {
        if candidate.is_empty() || unique.iter().any(|c| c == &candidate) {
            continue;
        }
        unique.push(candidate);
    }
    unique
}

pub(in crate::plugin_engine::host_api) fn nvm_default_bin_path(home: &Path) -> Option<PathBuf> {
    let alias_path = home.join(".nvm/alias/default");
    let version = std::fs::read_to_string(&alias_path).ok()?;
    let version = version.trim();
    if version.is_empty() {
        return None;
    }
    let version = if version.starts_with('v') {
        version.to_string()
    } else {
        format!("v{version}")
    };
    Some(home.join(".nvm/versions/node").join(version).join("bin"))
}

pub(in crate::plugin_engine::host_api) fn ccusage_path_entries_with(
    home: Option<&Path>,
    existing_path: Option<&OsStr>,
) -> Vec<PathBuf> {
    let mut entries: Vec<PathBuf> = Vec::new();

    if let Some(home) = home {
        entries.push(home.join(".bun/bin"));
        entries.push(home.join(".nvm/current/bin"));
        if let Some(nvm_bin) = nvm_default_bin_path(home) {
            entries.push(nvm_bin);
        }
        entries.push(home.join(".local/bin"));
    }

    entries.extend(
        ["/opt/homebrew/bin", "/usr/local/bin"]
            .into_iter()
            .map(PathBuf::from),
    );

    if let Some(existing_path) = existing_path {
        for path in std::env::split_paths(existing_path) {
            entries.push(path);
        }
    }

    let mut unique_entries = Vec::new();
    for entry in entries {
        if entry.as_os_str().is_empty() || unique_entries.iter().any(|path| path == &entry) {
            continue;
        }
        unique_entries.push(entry);
    }
    unique_entries
}

pub(in crate::plugin_engine::host_api) fn ccusage_enriched_path_with(
    home: Option<&Path>,
    existing_path: Option<&OsStr>,
) -> Option<OsString> {
    let entries = ccusage_path_entries_with(home, existing_path);
    if entries.is_empty() {
        return None;
    }
    std::env::join_paths(entries).ok()
}

pub(in crate::plugin_engine::host_api) fn ccusage_enriched_path() -> Option<OsString> {
    let home = dirs::home_dir();
    let existing_path = std::env::var_os("PATH");
    ccusage_enriched_path_with(home.as_deref(), existing_path.as_deref())
}

pub(in crate::plugin_engine::host_api) fn ccusage_runner_available(
    candidate: &str,
    enriched_path: Option<&OsStr>,
) -> bool {
    let mut command = std::process::Command::new(candidate);
    command.arg("--version");
    if let Some(path) = enriched_path {
        command.env("PATH", path);
    }
    command
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    command.status().map(|s| s.success()).unwrap_or(false)
}

pub(in crate::plugin_engine::host_api) fn configure_ccusage_command(
    command: &mut std::process::Command,
    args: &[String],
    enriched_path: Option<&OsStr>,
) {
    command.args(args);
    if let Some(path) = enriched_path {
        command.env("PATH", path);
    }
    command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
}

pub(in crate::plugin_engine::host_api) fn resolve_ccusage_runner_binary(
    kind: CcusageRunnerKind,
) -> Option<String> {
    let path = ccusage_enriched_path();
    ccusage_runner_candidates(kind)
        .into_iter()
        .find(|candidate| ccusage_runner_available(candidate, path.as_deref()))
}

pub(in crate::plugin_engine::host_api) fn collect_ccusage_runners_with<F>(
    mut resolver: F,
) -> Vec<(CcusageRunnerKind, String)>
where
    F: FnMut(CcusageRunnerKind) -> Option<String>,
{
    let mut runners = Vec::new();
    for kind in ccusage_runner_order() {
        if let Some(program) = resolver(kind) {
            runners.push((kind, program));
        }
    }
    runners
}

pub(in crate::plugin_engine::host_api) fn collect_ccusage_runners()
-> Vec<(CcusageRunnerKind, String)> {
    collect_ccusage_runners_with(resolve_ccusage_runner_binary)
}

pub(in crate::plugin_engine::host_api) fn append_ccusage_common_args(
    args: &mut Vec<String>,
    opts: &CcusageQueryOpts,
    provider: CcusageProvider,
    flavor: CcusageCommandFlavor,
) {
    let config = ccusage_provider_config(provider);
    if flavor == CcusageCommandFlavor::Current {
        args.push(config.command_namespace.to_string());
    }
    args.extend([
        "daily".to_string(),
        "--json".to_string(),
        "--order".to_string(),
        "desc".to_string(),
    ]);

    if let Some(since) = opts
        .since
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        args.push("--since".to_string());
        args.push(since.to_string());
    }

    if let Some(until) = opts
        .until
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        args.push("--until".to_string());
        args.push(until.to_string());
    }
}

pub(in crate::plugin_engine::host_api) fn ccusage_runner_args(
    kind: CcusageRunnerKind,
    opts: &CcusageQueryOpts,
    provider: CcusageProvider,
    flavor: CcusageCommandFlavor,
) -> Vec<String> {
    let package_spec = match flavor {
        CcusageCommandFlavor::Current => ccusage_package_spec(),
        CcusageCommandFlavor::Legacy => ccusage_legacy_package_spec(provider),
    };
    let npm_exec_bin = match (flavor, provider) {
        (CcusageCommandFlavor::Current, _) => CCUSAGE_BIN_NAME,
        (CcusageCommandFlavor::Legacy, CcusageProvider::Claude) => CCUSAGE_BIN_NAME,
        (CcusageCommandFlavor::Legacy, CcusageProvider::Codex) => CCUSAGE_LEGACY_CODEX_BIN_NAME,
    };
    let mut args: Vec<String> = match kind {
        CcusageRunnerKind::Bunx => vec!["--silent".to_string(), package_spec.clone()],
        CcusageRunnerKind::PnpmDlx => {
            vec!["-s".to_string(), "dlx".to_string(), package_spec.clone()]
        }
        CcusageRunnerKind::YarnDlx => {
            vec!["dlx".to_string(), "-q".to_string(), package_spec.clone()]
        }
        CcusageRunnerKind::NpmExec => vec![
            "exec".to_string(),
            "--yes".to_string(),
            format!("--package={package_spec}"),
            "--".to_string(),
            npm_exec_bin.to_string(),
        ],
        CcusageRunnerKind::Npx => vec!["--yes".to_string(), package_spec],
    };

    append_ccusage_common_args(&mut args, opts, provider, flavor);
    args
}
