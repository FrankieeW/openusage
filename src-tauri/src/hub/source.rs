// RED phase: tests written, implementation deliberately stubbed.
// `canonicalize` returns `Err(InvalidUrl)` for every input so tests fail for the
// "missing logic" reason, not for a typo or build error.

use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
}

impl PartialEq for CanonicalSource {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind && self.url == other.url && self.local_path == other.local_path
    }
}

impl Eq for CanonicalSource {}

impl std::fmt::Debug for CanonicalSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CanonicalSource")
            .field("kind", &self.kind)
            .field("url", &self.url)
            .field("local_path", &self.local_path)
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
        let normalized = normalize_github_owner_repo(rest)?;
        return Ok(github(&format!("https://github.com/{}", normalized)));
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
                    let normalized = normalize_github_owner_repo(path)?;
                    Ok(github(&format!("https://github.com/{}", normalized)))
                } else {
                    Ok(generic_git(trimmed))
                }
            }
            _ => Err(HubError::InvalidUrl),
        };
    }

    if let Some((owner, repo)) = trimmed.split_once('/') {
        if !owner.contains('.') && !owner.is_empty() && !repo.is_empty() {
            let repo_clean = repo.trim_end_matches(".git");
            if is_simple_segment(owner) && is_simple_segment(repo_clean) {
                return Ok(github(&format!(
                    "https://github.com/{}/{}",
                    owner, repo_clean
                )));
            }
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

fn normalize_github_owner_repo(path: &str) -> Result<String, HubError> {
    let trimmed = path.trim_end_matches(".git").trim_matches('/');
    let segments: Vec<&str> = trimmed.split('/').filter(|s| !s.is_empty()).collect();
    if segments.len() != 2 {
        return Err(HubError::InvalidUrl);
    }
    if !segments.iter().all(|s| is_simple_segment(s)) {
        return Err(HubError::InvalidUrl);
    }
    Ok(segments.join("/"))
}

fn is_simple_segment(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn github(url: &str) -> CanonicalSource {
    CanonicalSource {
        kind: SourceKind::Github,
        url: url.to_string(),
        local_path: None,
    }
}

fn generic_git(url: &str) -> CanonicalSource {
    CanonicalSource {
        kind: SourceKind::GenericGit,
        url: url.to_string(),
        local_path: None,
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
        }
    }

    fn generic(url: &str) -> CanonicalSource {
        CanonicalSource {
            kind: SourceKind::GenericGit,
            url: url.to_string(),
            local_path: None,
        }
    }

    fn local(path: &std::path::Path) -> CanonicalSource {
        CanonicalSource {
            kind: SourceKind::LocalPath,
            url: path.display().to_string(),
            local_path: Some(path.to_path_buf()),
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