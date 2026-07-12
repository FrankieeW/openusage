use super::*;

pub(in crate::plugin_engine::host_api) fn extract_last_json_value(stdout: &str) -> Option<String> {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return None;
    }

    if serde_json::from_str::<serde_json::Value>(trimmed).is_ok() {
        return Some(trimmed.to_string());
    }

    let mut starts: Vec<usize> = trimmed
        .char_indices()
        .filter(|(_, c)| *c == '{' || *c == '[')
        .map(|(idx, _)| idx)
        .collect();
    starts.reverse();

    for start in starts {
        let candidate = trimmed[start..].trim();
        if serde_json::from_str::<serde_json::Value>(candidate).is_ok() {
            return Some(candidate.to_string());
        }
    }

    None
}

pub(in crate::plugin_engine::host_api) fn normalize_ccusage_output(stdout: &str) -> Option<String> {
    let json_value = extract_last_json_value(stdout)?;
    let parsed: serde_json::Value = serde_json::from_str(&json_value).ok()?;

    let normalized = match parsed {
        serde_json::Value::Array(daily) => serde_json::json!({ "daily": daily }),
        serde_json::Value::Object(map) => {
            let daily = map.get("daily")?;
            if !daily.is_array() {
                return None;
            }
            serde_json::Value::Object(map)
        }
        _ => return None,
    };

    serde_json::to_string(&normalized).ok()
}

#[derive(Debug, Eq, PartialEq)]
pub(in crate::plugin_engine::host_api) enum CcusageRunnerResult {
    Success(String),
    Failed,
    TimedOut,
}

#[cfg(unix)]
pub(in crate::plugin_engine::host_api) fn kill_ccusage_process_group(
    child_id: u32,
) -> std::io::Result<()> {
    let pgid = i32::try_from(child_id)
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid child pid"))?;
    let rc = unsafe { libc::kill(-pgid, libc::SIGKILL) };
    if rc == 0 {
        return Ok(());
    }

    let err = std::io::Error::last_os_error();
    if err.raw_os_error() == Some(libc::ESRCH) {
        return Ok(());
    }
    Err(err)
}

pub(in crate::plugin_engine::host_api) fn kill_ccusage_on_timeout(
    child: &mut std::process::Child,
) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        kill_ccusage_process_group(child.id())
    }

    #[cfg(not(unix))]
    {
        child.kill()
    }
}

pub(in crate::plugin_engine::host_api) fn format_ccusage_timeout(
    timeout: std::time::Duration,
) -> String {
    if timeout.subsec_millis() == 0 {
        return format!("{}s", timeout.as_secs());
    }
    if timeout.as_secs() == 0 {
        return format!("{}ms", timeout.as_millis());
    }
    format!("{:.3}s", timeout.as_secs_f64())
}

#[cfg(test)]
pub(in crate::plugin_engine::host_api) fn run_ccusage_with_runner(
    kind: CcusageRunnerKind,
    program: &str,
    opts: &CcusageQueryOpts,
    provider: CcusageProvider,
    plugin_id: &str,
) -> CcusageRunnerResult {
    run_ccusage_with_runner_deadline(
        kind,
        program,
        opts,
        provider,
        plugin_id,
        ProbeDeadline::none(),
    )
}

pub(in crate::plugin_engine::host_api) fn run_ccusage_with_runner_deadline(
    kind: CcusageRunnerKind,
    program: &str,
    opts: &CcusageQueryOpts,
    provider: CcusageProvider,
    plugin_id: &str,
    deadline: ProbeDeadline,
) -> CcusageRunnerResult {
    if deadline.has_elapsed() {
        log::warn!("[plugin:{}] ccusage skipped: probe timed out", plugin_id);
        return CcusageRunnerResult::TimedOut;
    }

    let Some(current_timeout) = deadline.clamp_duration(Duration::from_secs(CCUSAGE_TIMEOUT_SECS))
    else {
        log_probe_deadline_skip(plugin_id, "ccusage");
        return CcusageRunnerResult::TimedOut;
    };

    let current = run_ccusage_with_runner_timeout(
        kind,
        program,
        opts,
        provider,
        plugin_id,
        CcusageCommandFlavor::Current,
        current_timeout,
    );
    match current {
        CcusageRunnerResult::Failed if deadline.has_elapsed() => CcusageRunnerResult::TimedOut,
        CcusageRunnerResult::Failed => {
            let Some(legacy_timeout) =
                deadline.clamp_duration(Duration::from_secs(CCUSAGE_TIMEOUT_SECS))
            else {
                log_probe_deadline_skip(plugin_id, "ccusage legacy fallback");
                return CcusageRunnerResult::TimedOut;
            };
            run_ccusage_with_runner_timeout(
                kind,
                program,
                opts,
                provider,
                plugin_id,
                CcusageCommandFlavor::Legacy,
                legacy_timeout,
            )
        }
        other => other,
    }
}

pub(in crate::plugin_engine::host_api) fn run_ccusage_with_runner_timeout(
    kind: CcusageRunnerKind,
    program: &str,
    opts: &CcusageQueryOpts,
    provider: CcusageProvider,
    plugin_id: &str,
    flavor: CcusageCommandFlavor,
    timeout: std::time::Duration,
) -> CcusageRunnerResult {
    let args = ccusage_runner_args(kind, opts, provider, flavor);
    let enriched_path = ccusage_enriched_path();
    let mut command = std::process::Command::new(program);
    configure_ccusage_command(&mut command, &args, enriched_path.as_deref());

    if let Some(home_path) = ccusage_home_override(opts, provider) {
        let config = ccusage_provider_config(provider);
        command.env(config.home_env_var, expand_path(home_path));
    }

    let redacted_program = redact_log_message(program);

    log::info!(
        "[plugin:{}] ccusage query via {} {:?} ({})",
        plugin_id,
        ccusage_runner_label(kind),
        flavor,
        redacted_program
    );

    let mut child = match command.spawn() {
        Ok(c) => c,
        Err(e) => {
            log::warn!(
                "[plugin:{}] ccusage spawn failed for {}: {}",
                plugin_id,
                ccusage_runner_label(kind),
                e
            );
            return CcusageRunnerResult::Failed;
        }
    };

    // Drain pipes concurrently while the process is running so the child cannot block on full
    // stdout/stderr buffers before exit.
    let mut stdout_reader = child.stdout.take().map(|mut stdout| {
        std::thread::spawn(move || {
            let mut v = Vec::new();
            let _ = std::io::Read::read_to_end(&mut stdout, &mut v);
            v
        })
    });
    let mut stderr_reader = child.stderr.take().map(|mut stderr| {
        std::thread::spawn(move || {
            let mut v = Vec::new();
            let _ = std::io::Read::read_to_end(&mut stderr, &mut v);
            v
        })
    });

    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = stdout_reader
                    .take()
                    .and_then(|reader| reader.join().ok())
                    .unwrap_or_default();
                let stderr = stderr_reader
                    .take()
                    .and_then(|reader| reader.join().ok())
                    .unwrap_or_default();

                if status.success() {
                    let out = String::from_utf8_lossy(&stdout);
                    if let Some(normalized_json) = normalize_ccusage_output(&out) {
                        return CcusageRunnerResult::Success(normalized_json);
                    }
                    log::warn!(
                        "[plugin:{}] ccusage output parse failed for {}",
                        plugin_id,
                        ccusage_runner_label(kind)
                    );
                    return CcusageRunnerResult::Failed;
                }

                let err = String::from_utf8_lossy(&stderr);
                log::warn!(
                    "[plugin:{}] ccusage failed for {}: {}",
                    plugin_id,
                    ccusage_runner_label(kind),
                    err.trim()
                );
                return CcusageRunnerResult::Failed;
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    if let Err(e) = kill_ccusage_on_timeout(&mut child) {
                        log::warn!(
                            "[plugin:{}] ccusage process group kill failed for {}: {}",
                            plugin_id,
                            ccusage_runner_label(kind),
                            e
                        );
                        let _ = child.kill();
                    }
                    let _ = child.wait();
                    let _ = stdout_reader.take().and_then(|reader| reader.join().ok());
                    let _ = stderr_reader.take().and_then(|reader| reader.join().ok());
                    log::warn!(
                        "[plugin:{}] ccusage timed out after {} for {}",
                        plugin_id,
                        format_ccusage_timeout(timeout),
                        ccusage_runner_label(kind)
                    );
                    return CcusageRunnerResult::TimedOut;
                }
                std::thread::sleep(std::time::Duration::from_millis(CCUSAGE_POLL_INTERVAL_MS));
            }
            Err(e) => {
                log::warn!(
                    "[plugin:{}] ccusage wait failed for {}: {}",
                    plugin_id,
                    ccusage_runner_label(kind),
                    e
                );
                return CcusageRunnerResult::Failed;
            }
        }
    }
}

pub(in crate::plugin_engine::host_api) fn run_ccusage_query_with_runners<F>(
    runners: Vec<(CcusageRunnerKind, String)>,
    opts: &CcusageQueryOpts,
    provider: CcusageProvider,
    plugin_id: &str,
    mut run: F,
) -> String
where
    F: FnMut(
        CcusageRunnerKind,
        &str,
        &CcusageQueryOpts,
        CcusageProvider,
        &str,
    ) -> CcusageRunnerResult,
{
    if runners.is_empty() {
        log::warn!(
            "[plugin:{}] no package runner found for ccusage query",
            plugin_id
        );
        return serde_json::json!({ "status": "no_runner" }).to_string();
    }

    for (kind, program) in runners {
        match run(kind, &program, opts, provider, plugin_id) {
            CcusageRunnerResult::Success(result) => {
                let data: serde_json::Value = match serde_json::from_str(&result) {
                    Ok(v) => v,
                    Err(e) => {
                        log::warn!(
                            "[plugin:{}] ccusage normalized payload parse failed: {}",
                            plugin_id,
                            e
                        );
                        continue;
                    }
                };
                return serde_json::json!({ "status": "ok", "data": data }).to_string();
            }
            CcusageRunnerResult::Failed => {}
            CcusageRunnerResult::TimedOut => {
                log::warn!(
                    "[plugin:{}] ccusage query timed out; skipping fallback runners",
                    plugin_id
                );
                return serde_json::json!({ "status": "runner_failed" }).to_string();
            }
        }
    }

    log::warn!(
        "[plugin:{}] ccusage query failed with all available runners",
        plugin_id
    );
    serde_json::json!({ "status": "runner_failed" }).to_string()
}
