use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SourceKind {
    Github,
    GenericGit,
    LocalPath,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HubError {
    InvalidUrl,
}

pub struct CanonicalSource {
    pub kind: SourceKind,
    pub url: String,
    pub local_path: Option<std::path::PathBuf>,
    pub branch: Option<String>,
}

impl PartialEq for CanonicalSource {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.url == other.url
            && self.local_path == other.local_path
            && self.branch == other.branch
    }
}

impl Eq for CanonicalSource {}

impl std::fmt::Debug for CanonicalSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CanonicalSource")
            .field("kind", &self.kind)
            .field("url", &self.url)
            .field("local_path", &self.local_path)
            .field("branch", &self.branch)
            .finish()
    }
}

pub fn canonicalize(input: &str) -> Result<CanonicalSource, HubError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(HubError::InvalidUrl);
    }

    if let Some(rest) = trimmed.strip_prefix("file://") {
        return local_path(std::path::PathBuf::from(rest));
    }

    if let Some(rest) = trimmed.strip_prefix("git@github.com:") {
        let (normalized, branch) = normalize_github_path(rest)?;
        return Ok(github_with_branch(
            &format!("https://github.com/{}", normalized),
            branch,
        ));
    }

    let as_path = Path::new(trimmed);
    if as_path.is_absolute() {
        return local_path(as_path.to_path_buf());
    }

    if let Some((scheme, rest)) = trimmed.split_once("://") {
        return match scheme {
            "https" | "http" => {
                let (host, path) = split_host_path(rest);
                if host == "github.com" {
                    let (normalized, branch) = normalize_github_path(path)?;
                    Ok(github_with_branch(
                        &format!("https://github.com/{}", normalized),
                        branch,
                    ))
                } else {
                    Ok(generic_git(trimmed))
                }
            }
            _ => Err(HubError::InvalidUrl),
        };
    }

    if let Some((owner, repo)) = trimmed.split_once('/')
        && !owner.contains('.')
        && !owner.is_empty()
        && !repo.is_empty()
    {
        let repo_clean = repo.trim_end_matches(".git");
        if is_simple_segment(owner) && is_simple_segment(repo_clean) {
            return Ok(github(&format!(
                "https://github.com/{}/{}",
                owner, repo_clean
            )));
        }
    }

    Err(HubError::InvalidUrl)
}

fn local_path(path: std::path::PathBuf) -> Result<CanonicalSource, HubError> {
    if path.is_dir() {
        Ok(CanonicalSource {
            kind: SourceKind::LocalPath,
            url: path.display().to_string(),
            local_path: Some(path),
            branch: None,
        })
    } else {
        Err(HubError::InvalidUrl)
    }
}

fn split_host_path(s: &str) -> (&str, &str) {
    match s.find('/') {
        Some(i) => (&s[..i], &s[i + 1..]),
        None => (s, ""),
    }
}

fn normalize_github_path(path: &str) -> Result<(String, Option<String>), HubError> {
    let trimmed = path.trim_end_matches(".git").trim_matches('/');
    let segments: Vec<&str> = trimmed.split('/').filter(|s| !s.is_empty()).collect();
    if segments.len() != 2 {
        if segments.len() >= 4 && segments[2] == "tree" {
            let owner_repo = &segments[..2];
            if !owner_repo.iter().all(|s| is_simple_segment(s)) {
                return Err(HubError::InvalidUrl);
            }
            let branch = segments[3..].join("/");
            if !is_valid_branch_path(&branch) {
                return Err(HubError::InvalidUrl);
            }
            return Ok((owner_repo.join("/"), Some(branch)));
        }
        return Err(HubError::InvalidUrl);
    }
    if !segments.iter().all(|s| is_simple_segment(s)) {
        return Err(HubError::InvalidUrl);
    }
    Ok((segments.join("/"), None))
}

fn is_simple_segment(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn is_valid_branch_path(s: &str) -> bool {
    !s.is_empty()
        && !s.contains("..")
        && !s.contains('\\')
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'))
}

fn github(url: &str) -> CanonicalSource {
    github_with_branch(url, None)
}

fn github_with_branch(url: &str, branch: Option<String>) -> CanonicalSource {
    CanonicalSource {
        kind: SourceKind::Github,
        url: url.to_string(),
        local_path: None,
        branch,
    }
}

fn generic_git(url: &str) -> CanonicalSource {
    CanonicalSource {
        kind: SourceKind::GenericGit,
        url: url.to_string(),
        local_path: None,
        branch: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn github(url: &str) -> CanonicalSource {
        CanonicalSource {
            kind: SourceKind::Github,
            url: url.to_string(),
            local_path: None,
            branch: None,
        }
    }

    fn generic(url: &str) -> CanonicalSource {
        CanonicalSource {
            kind: SourceKind::GenericGit,
            url: url.to_string(),
            local_path: None,
            branch: None,
        }
    }

    fn local(path: &std::path::Path) -> CanonicalSource {
        CanonicalSource {
            kind: SourceKind::LocalPath,
            url: path.display().to_string(),
            local_path: Some(path.to_path_buf()),
            branch: None,
        }
    }

    fn github_branch(url: &str, branch: &str) -> CanonicalSource {
        CanonicalSource {
            kind: SourceKind::Github,
            url: url.to_string(),
            local_path: None,
            branch: Some(branch.to_string()),
        }
    }

    #[test]
    fn shorthand_owner_repo() {
        assert_eq!(
            canonicalize("robinebers/openusage").unwrap(),
            github("https://github.com/robinebers/openusage"),
        );
    }

    #[test]
    fn github_https_url() {
        assert_eq!(
            canonicalize("https://github.com/foo/bar").unwrap(),
            github("https://github.com/foo/bar"),
        );
    }

    #[test]
    fn github_tree_url_preserves_branch_as_source_ref() {
        assert_eq!(
            canonicalize("https://github.com/foo/bar/tree/feat/openrouter-provider").unwrap(),
            github_branch("https://github.com/foo/bar", "feat/openrouter-provider"),
        );
    }

    #[test]
    fn ssh_url_normalizes_to_https() {
        assert_eq!(
            canonicalize("git@github.com:foo/bar.git").unwrap(),
            github("https://github.com/foo/bar"),
        );
    }

    #[test]
    fn gitlab_is_generic_git() {
        assert_eq!(
            canonicalize("https://gitlab.com/foo/bar").unwrap(),
            generic("https://gitlab.com/foo/bar"),
        );
    }

    #[test]
    fn file_url() {
        let dir = tempdir();
        let input = format!("file://{}", dir.display());
        assert_eq!(canonicalize(&input).unwrap(), local(&dir));
    }

    #[test]
    fn absolute_existing_path() {
        let dir = tempdir();
        assert_eq!(canonicalize(dir.to_str().unwrap()).unwrap(), local(&dir));
    }

    #[test]
    fn ftp_is_invalid() {
        assert_eq!(
            canonicalize("ftp://example.com/repo").unwrap_err(),
            HubError::InvalidUrl,
        );
    }

    #[test]
    fn github_owner_without_repo_is_invalid() {
        assert_eq!(
            canonicalize("github.com/foo").unwrap_err(),
            HubError::InvalidUrl,
        );
    }

    fn tempdir() -> std::path::PathBuf {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "openusage-hub-source-test-{}-{}",
            std::process::id(),
            suffix
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }
}
