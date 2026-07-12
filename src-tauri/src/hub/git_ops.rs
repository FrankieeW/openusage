// Git operations: clone / fetch+reset wrapped in timeouts.
// Tests that hit the network are #[ignore] — covered by manual smoke.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::{Child, Command};
use tokio::time::timeout;

pub const CLONE_TIMEOUT_SECS: u64 = 60;
pub const REFRESH_TIMEOUT_SECS: u64 = 30;

#[derive(Debug)]
pub enum GitError {
    NotInstalled,
    Timeout,
    CommandFailed(String),
    Io(String),
}

impl std::fmt::Display for GitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GitError::NotInstalled => write!(f, "git binary not found on PATH"),
            GitError::Timeout => write!(f, "git operation timed out"),
            GitError::CommandFailed(m) => write!(f, "git failed: {}", m),
            GitError::Io(m) => write!(f, "git io: {}", m),
        }
    }
}

impl std::error::Error for GitError {}

impl From<GitError> for super::HubError {
    fn from(e: GitError) -> Self {
        match e {
            GitError::NotInstalled => super::HubError::git_not_installed(),
            GitError::Timeout => super::HubError::clone_failed("timeout"),
            GitError::CommandFailed(m) => super::HubError::clone_failed(m),
            GitError::Io(m) => super::HubError::io(m),
        }
    }
}

#[cfg(test)]
pub async fn is_git_available() -> bool {
    matches!(
        Command::new("git").arg("--version").output().await,
        Ok(o) if o.status.success()
    )
}

pub async fn clone(url: &str, dest: &Path, branch: Option<&str>) -> Result<(), GitError> {
    let dest_existed = dest.exists();
    let mut cmd = Command::new("git");
    cmd.arg("clone").arg("--depth=1").arg("--single-branch");
    if let Some(b) = branch {
        cmd.arg("--branch").arg(b);
    }
    cmd.arg(url)
        .arg(dest)
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let result = run_with_timeout(cmd, CLONE_TIMEOUT_SECS).await;
    if result.is_err()
        && !dest_existed
        && dest.exists()
        && let Err(error) = std::fs::remove_dir_all(dest)
    {
        log::warn!(
            "hub git clone: cannot remove partial cache {}: {}",
            dest.display(),
            error
        );
    }
    result
}

pub async fn fetch_and_reset(repo_dir: &Path, branch: Option<&str>) -> Result<(), GitError> {
    let mut fetch = Command::new("git");
    fetch
        .arg("-C")
        .arg(repo_dir)
        .arg("fetch")
        .arg("--depth=1")
        .arg("origin")
        .arg(branch.unwrap_or("HEAD"))
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    run_with_timeout(fetch, REFRESH_TIMEOUT_SECS).await?;

    let mut reset = Command::new("git");
    reset
        .arg("-C")
        .arg(repo_dir)
        .arg("reset")
        .arg("--hard")
        .arg("FETCH_HEAD")
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    run_with_timeout(reset, REFRESH_TIMEOUT_SECS).await
}

pub async fn head_commit(repo_dir: &Path) -> Result<String, GitError> {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(repo_dir)
        .arg("rev-parse")
        .arg("HEAD")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = run_capture_with_timeout(command, REFRESH_TIMEOUT_SECS).await?;
    Ok(String::from_utf8_lossy(&output).trim().to_string())
}

async fn run_with_timeout(command: Command, secs: u64) -> Result<(), GitError> {
    let output = run_command_with_timeout(command, secs).await?;
    if output.status.success() {
        Ok(())
    } else {
        Err(GitError::CommandFailed(format_stderr(&output.stderr)))
    }
}

async fn run_capture_with_timeout(command: Command, secs: u64) -> Result<Vec<u8>, GitError> {
    let output = run_command_with_timeout(command, secs).await?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(GitError::CommandFailed(format_stderr(&output.stderr)))
    }
}

async fn run_command_with_timeout(
    mut command: Command,
    secs: u64,
) -> Result<std::process::Output, GitError> {
    #[cfg(unix)]
    command.process_group(0);

    let mut child = command.spawn().map_err(map_io_error)?;
    let process_id = child.id();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let wait_for_output = async {
        let (status, stdout, stderr) =
            tokio::try_join!(child.wait(), read_pipe(stdout), read_pipe(stderr),)?;
        Ok::<std::process::Output, std::io::Error>(std::process::Output {
            status,
            stdout,
            stderr,
        })
    };

    match timeout(Duration::from_secs(secs), wait_for_output).await {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(error)) => {
            terminate_child_processes(&mut child, process_id)
                .await
                .map_err(|cleanup_error| {
                    GitError::Io(format!(
                        "git operation failed: {error}; child cleanup failed: {cleanup_error}"
                    ))
                })?;
            Err(map_io_error(error))
        }
        Err(_) => {
            terminate_child_processes(&mut child, process_id)
                .await
                .map_err(|error| {
                    GitError::Io(format!(
                        "git operation timed out and child cleanup failed: {error}"
                    ))
                })?;
            Err(GitError::Timeout)
        }
    }
}

async fn terminate_child_processes(
    child: &mut Child,
    process_id: Option<u32>,
) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        let Some(process_id) = process_id else {
            return child.kill().await;
        };
        let process_group_id = i32::try_from(process_id).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid child process id")
        })?;
        if process_group_id == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid zero child process id",
            ));
        }
        // SAFETY: `process_group_id` is a positive child PID and `SIGKILL` is a valid signal;
        // the negative PID form is the POSIX contract for signaling that child's process group.
        let result = unsafe { libc::kill(-process_group_id, libc::SIGKILL) };
        if result != 0 {
            let group_error = std::io::Error::last_os_error();
            if group_error.raw_os_error() != Some(libc::ESRCH) {
                return match child.kill().await {
                    Ok(()) => Err(group_error),
                    Err(child_error) => Err(std::io::Error::other(format!(
                        "process group kill failed: {group_error}; child kill failed: {child_error}"
                    ))),
                };
            }
        }
        child.wait().await.map(|_| ())
    }

    #[cfg(not(unix))]
    {
        let _ = process_id;
        child.kill().await
    }
}

async fn read_pipe<R>(pipe: Option<R>) -> std::io::Result<Vec<u8>>
where
    R: AsyncRead + Unpin,
{
    let Some(mut pipe) = pipe else {
        return Ok(Vec::new());
    };
    let mut bytes = Vec::new();
    pipe.read_to_end(&mut bytes).await?;
    Ok(bytes)
}

fn map_io_error(error: std::io::Error) -> GitError {
    if error.kind() == std::io::ErrorKind::NotFound {
        GitError::NotInstalled
    } else {
        GitError::Io(error.to_string())
    }
}

fn format_stderr(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir(label: &str) -> std::path::PathBuf {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "openusage-hub-git-{label}-{}-{suffix}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[tokio::test]
    async fn is_git_available_returns_bool_without_panic() {
        // Smoke — just exercise the path; we don't assert true/false because the
        // CI box may or may not have git installed.
        let _ = is_git_available().await;
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn timeout_terminates_and_reaps_the_child_process() {
        // Given
        let root = tempdir("timeout");
        let pid_file = root.join("pid");
        let mut command = Command::new("/bin/sh");
        command
            .arg("-c")
            .arg("echo $$ > \"$1\"; exec sleep 10")
            .arg("openusage-timeout-test")
            .arg(&pid_file)
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        // When
        let result = run_with_timeout(command, 1).await;

        // Then
        assert!(matches!(result, Err(GitError::Timeout)));
        let pid = std::fs::read_to_string(&pid_file).unwrap();
        let process = std::process::Command::new("/bin/ps")
            .args(["-p", pid.trim(), "-o", "stat="])
            .output()
            .unwrap();
        let was_running = process.status.success() && !process.stdout.is_empty();
        if was_running {
            let _ = std::process::Command::new("/bin/kill")
                .arg(pid.trim())
                .status();
        }
        assert!(!was_running, "timed-out child {pid} was still running");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn timeout_terminates_descendant_processes() {
        // Given
        let root = tempdir("timeout-descendant");
        let pid_file = root.join("descendant-pid");
        let mut command = Command::new("/bin/sh");
        command
            .arg("-c")
            .arg("sleep 30 & echo $! > \"$1\"; wait")
            .arg("openusage-timeout-descendant-test")
            .arg(&pid_file)
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        // When
        let result = run_with_timeout(command, 1).await;

        // Then
        assert!(matches!(result, Err(GitError::Timeout)));
        let descendant_pid = std::fs::read_to_string(&pid_file).unwrap();
        let process = std::process::Command::new("/bin/ps")
            .args(["-p", descendant_pid.trim(), "-o", "stat="])
            .output()
            .unwrap();
        let was_running = process.status.success() && !process.stdout.is_empty();
        if was_running {
            let _ = std::process::Command::new("/bin/kill")
                .args(["-KILL", descendant_pid.trim()])
                .status();
        }
        assert!(
            !was_running,
            "timed-out descendant {descendant_pid} was still running"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    #[ignore] // hits the network; run with `cargo test -- --ignored`
    async fn clone_github_repo_succeeds() {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dest = std::env::temp_dir().join(format!(
            "openusage-hub-git-clone-{}-{}",
            std::process::id(),
            suffix
        ));
        clone("https://github.com/octocat/Hello-World", &dest, None)
            .await
            .expect("clone should succeed");
        assert!(dest.join("README").exists() || dest.join("hello-world").is_dir());
        let _ = std::fs::remove_dir_all(&dest);
    }
}
