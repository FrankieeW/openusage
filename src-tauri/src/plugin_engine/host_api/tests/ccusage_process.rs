use super::*;

#[test]
fn ccusage_timeout_stops_runner_fallback() {
    let opts = CcusageQueryOpts::default();
    let runners = vec![
        (CcusageRunnerKind::Bunx, "bunx".to_string()),
        (CcusageRunnerKind::Npx, "npx".to_string()),
    ];
    let mut calls = Vec::new();

    let result = run_ccusage_query_with_runners(
        runners,
        &opts,
        CcusageProvider::Codex,
        "codex",
        |kind, _, _, _, _| {
            calls.push(kind);
            CcusageRunnerResult::TimedOut
        },
    );

    let value: serde_json::Value = serde_json::from_str(&result).expect("valid status json");
    assert_eq!(value["status"], "runner_failed");
    assert_eq!(calls, vec![CcusageRunnerKind::Bunx]);
}

#[cfg(unix)]
#[test]
fn ccusage_runner_retries_legacy_package_when_current_package_fails() {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    let test_id = format!(
        "openusage-ccusage-legacy-fallback-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos()
    );
    let dir = std::env::temp_dir().join(test_id);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let script_path = dir.join("fake-bunx.sh");
    let args_path = dir.join("args.log");

    let mut script = std::fs::File::create(&script_path).expect("create script");
    let script_body = format!(
        r#"#!/bin/sh
echo "$*" >> "{}"
case "$*" in
  *"@ccusage/codex@18.0.11"*)
printf '{{"daily":[]}}\n'
exit 0
;;
  *)
echo "blocked current package" >&2
exit 1
;;
esac
"#,
        args_path.display()
    );
    script
        .write_all(script_body.as_bytes())
        .expect("write script");
    let mut permissions = script.metadata().expect("script metadata").permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&script_path, permissions).expect("make script executable");

    let opts = CcusageQueryOpts {
        provider: Some("codex".to_string()),
        since: Some("20260101".to_string()),
        until: None,
        home_path: None,
        claude_path: None,
    };
    let result = run_ccusage_with_runner(
        CcusageRunnerKind::Bunx,
        script_path.to_string_lossy().as_ref(),
        &opts,
        CcusageProvider::Codex,
        "codex",
    );
    assert_eq!(
        result,
        CcusageRunnerResult::Success(r#"{"daily":[]}"#.to_string())
    );

    let calls = std::fs::read_to_string(&args_path).expect("read args log");
    assert!(calls.contains("ccusage@20.0.2 codex daily"));
    assert!(calls.contains("@ccusage/codex@18.0.11 daily"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ccusage_timeout_log_uses_actual_timeout() {
    assert_eq!(
        format_ccusage_timeout(std::time::Duration::from_millis(100)),
        "100ms"
    );
    assert_eq!(
        format_ccusage_timeout(std::time::Duration::from_secs(CCUSAGE_TIMEOUT_SECS)),
        "15s"
    );
}

#[test]
fn probe_deadline_clamps_host_timeout_to_remaining_budget() {
    let deadline = ProbeDeadline::at(Instant::now() + Duration::from_millis(25));
    let clamped = deadline
        .clamp_duration(Duration::from_secs(10))
        .expect("remaining budget should produce a host timeout");

    assert!(
        clamped <= Duration::from_millis(25),
        "host timeout should not exceed remaining probe budget"
    );
    assert!(
        clamped >= Duration::from_millis(1),
        "host timeout should stay non-zero for blocking clients"
    );
}

#[test]
fn probe_deadline_does_not_extend_elapsed_budget() {
    let deadline = ProbeDeadline::at(Instant::now());

    assert_eq!(deadline.clamp_duration(Duration::from_secs(10)), None);
}

#[cfg(unix)]
#[test]
fn ccusage_timeout_kills_descendant_and_closes_pipes() {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::time::{Duration, Instant};

    fn pid_exists(pid: i32) -> bool {
        unsafe { libc::kill(pid, 0) == 0 }
    }

    fn read_pid_file(path: &Path, deadline: Instant) -> i32 {
        loop {
            if let Ok(pid_text) = std::fs::read_to_string(path) {
                let pid_text = pid_text.trim();
                if !pid_text.is_empty() {
                    return pid_text.parse().expect("parse descendant pid");
                }
            }
            if Instant::now() >= deadline {
                panic!("descendant pid file was not created at {}", path.display());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    let test_id = format!(
        "openusage-ccusage-timeout-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos()
    );
    let dir = std::env::temp_dir().join(test_id);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let script_path = dir.join("fake-ccusage-runner.sh");
    let pid_path = dir.join("descendant.pid");

    let mut script = std::fs::File::create(&script_path).expect("create script");
    let script_body = format!(
        r#"#!/bin/sh
sh -c 'sleep 30' &
echo $! > "{}"
echo "started"
wait
"#,
        pid_path.display()
    );
    script
        .write_all(script_body.as_bytes())
        .expect("write script");
    let mut permissions = script.metadata().expect("script metadata").permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&script_path, permissions).expect("make script executable");

    let opts = CcusageQueryOpts::default();
    let start = Instant::now();
    let result = run_ccusage_with_runner_timeout(
        CcusageRunnerKind::Bunx,
        script_path.to_string_lossy().as_ref(),
        &opts,
        CcusageProvider::Codex,
        "codex",
        CcusageCommandFlavor::Current,
        Duration::from_secs(1),
    );

    assert_eq!(result, CcusageRunnerResult::TimedOut);
    assert!(
        start.elapsed() < Duration::from_secs(3),
        "timeout cleanup should not hang on inherited stdout/stderr pipes"
    );

    let descendant_pid = read_pid_file(&pid_path, Instant::now() + Duration::from_secs(1));

    let deadline = Instant::now() + Duration::from_secs(2);
    while pid_exists(descendant_pid) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        !pid_exists(descendant_pid),
        "descendant process should be killed with ccusage process group"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
