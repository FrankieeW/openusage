// Git operations: clone / fetch+reset wrapped in timeouts.
// Tests that hit the network are #[ignore] — covered by manual smoke.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;
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

pub async fn is_git_available() -> bool {
    matches!(
        Command::new("git").arg("--version").output().await,
        Ok(o) if o.status.success()
    )
}

pub async fn clone(url: &str, dest: &Path) -> Result<(), GitError> {
    let fut = Command::new("git")
        .arg("clone")
        .arg("--depth=1")
        .arg(url)
        .arg(dest)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output();
    run_with_timeout(fut, CLONE_TIMEOUT_SECS).await
}

pub async fn fetch_and_reset(repo_dir: &Path) -> Result<(), GitError> {
    let fetch_fut = Command::new("git")
        .arg("-C")
        .arg(repo_dir)
        .arg("fetch")
        .arg("--depth=1")
        .arg("origin")
        .arg("HEAD")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output();
    run_with_timeout(fetch_fut, REFRESH_TIMEOUT_SECS).await?;

    let reset_fut = Command::new("git")
        .arg("-C")
        .arg(repo_dir)
        .arg("reset")
        .arg("--hard")
        .arg("FETCH_HEAD")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output();
    run_with_timeout(reset_fut, REFRESH_TIMEOUT_SECS).await
}

async fn run_with_timeout(
    fut: impl std::future::Future<Output = std::io::Result<std::process::Output>>,
    secs: u64,
) -> Result<(), GitError> {
    match timeout(Duration::from_secs(secs), fut).await {
        Ok(Ok(o)) if o.status.success() => Ok(()),
        Ok(Ok(o)) => Err(GitError::CommandFailed(format_stderr(&o.stderr))),
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => Err(GitError::NotInstalled),
        Ok(Err(e)) => Err(GitError::Io(e.to_string())),
        Err(_) => Err(GitError::Timeout),
    }
}

fn format_stderr(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn is_git_available_returns_bool_without_panic() {
        // Smoke — just exercise the path; we don't assert true/false because the
        // CI box may or may not have git installed.
        let _ = is_git_available().await;
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
        clone("https://github.com/octocat/Hello-World", &dest)
            .await
            .expect("clone should succeed");
        assert!(dest.join("README").exists() || dest.join("hello-world").is_dir());
        let _ = std::fs::remove_dir_all(&dest);
    }
}