use serde::{Deserialize, Serialize};

use super::{SkippedPlugin, install, registry};

/// Error type returned to the JS bridge. Always carries a stable `code` and a
/// human-readable `message`. Optional `context` carries structured details
/// (e.g. the conflicting source id for `Conflict`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubError {
    pub code: HubErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum HubErrorCode {
    InvalidUrl,
    GitNotInstalled,
    CloneFailed,
    NotFound,
    Conflict,
    IoError,
    ManifestParse,
    SourceHealthFailed,
}

impl std::fmt::Display for HubError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for HubError {}

impl HubError {
    pub fn new(code: HubErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            context: None,
        }
    }

    pub fn with_context(
        code: HubErrorCode,
        message: impl Into<String>,
        ctx: serde_json::Value,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            context: Some(ctx),
        }
    }

    pub fn invalid_url() -> Self {
        Self::new(HubErrorCode::InvalidUrl, "invalid source URL")
    }
    pub fn git_not_installed() -> Self {
        Self::new(
            HubErrorCode::GitNotInstalled,
            "git binary not found on PATH",
        )
    }
    pub fn clone_failed(msg: impl Into<String>) -> Self {
        Self::new(HubErrorCode::CloneFailed, msg)
    }
    pub fn not_found(what: impl Into<String>) -> Self {
        Self::new(HubErrorCode::NotFound, what)
    }
    pub fn conflict(other_source_id: &str) -> Self {
        Self::with_context(
            HubErrorCode::Conflict,
            format!("already installed from {}", other_source_id),
            serde_json::json!({ "otherSourceId": other_source_id }),
        )
    }
    pub fn conflict_unmanaged() -> Self {
        Self::new(HubErrorCode::Conflict, "already installed outside Hub")
    }
    pub fn io(msg: impl Into<String>) -> Self {
        Self::new(HubErrorCode::IoError, msg)
    }
    pub fn manifest_parse(msg: impl Into<String>) -> Self {
        Self::new(HubErrorCode::ManifestParse, msg)
    }
    pub fn source_health_failed(
        message: impl Into<String>,
        available_count: usize,
        skipped: &[SkippedPlugin],
    ) -> Self {
        Self::with_context(
            HubErrorCode::SourceHealthFailed,
            message,
            serde_json::json!({
                "availableCount": available_count,
                "skippedCount": skipped.len(),
                "skipped": skipped,
            }),
        )
    }
}

impl From<install::InstallError> for HubError {
    fn from(e: install::InstallError) -> Self {
        match e {
            install::InstallError::ConflictWithSource(s) => Self::conflict(&s),
            install::InstallError::ConflictUnmanaged => Self::conflict_unmanaged(),
            install::InstallError::ManifestParse(m) => Self::manifest_parse(m),
            install::InstallError::Io(m) => Self::io(m),
            install::InstallError::IdMismatch {
                dir_name,
                manifest_id,
            } => Self::manifest_parse(format!(
                "id mismatch: dir={} manifest={}",
                dir_name, manifest_id
            )),
            install::InstallError::EntryOutsidePluginDir => {
                Self::manifest_parse("entry path escapes plugin dir")
            }
        }
    }
}

impl From<registry::RegistryError> for HubError {
    fn from(e: registry::RegistryError) -> Self {
        match e {
            registry::RegistryError::Io(m) => Self::io(m),
            registry::RegistryError::Json(m) => Self::manifest_parse(m),
        }
    }
}
