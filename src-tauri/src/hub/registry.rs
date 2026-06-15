// RED phase: registry read/write stubs that don't actually persist anything.
// Tests below expect round-trip + atomic write + crash recovery semantics.

use std::path::Path;
#[cfg(test)] use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::hub::source::SourceKind;

pub const CURRENT_VERSION: u32 = 1;
pub const DEFAULT_HUB_URL: &str = "https://github.com/FrankieeW/openusage-collection";
pub const DEFAULT_HUB_LABEL: &str = "Frankie's";
pub const DEFAULT_HUB_ID: &str = "default";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    pub id: String,
    pub label: String,
    pub url: String,
    pub kind: SourceKind,
    pub added_at: i64,
    pub last_refreshed_at: Option<i64>,
    pub auto_check: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryFile {
    pub version: u32,
    pub sources: Vec<Source>,
}

#[derive(Debug)]
pub enum RegistryError {
    Io(String),
    Json(String),
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegistryError::Io(m) => write!(f, "io: {}", m),
            RegistryError::Json(m) => write!(f, "json: {}", m),
        }
    }
}

impl std::error::Error for RegistryError {}

pub fn default_registry() -> RegistryFile {
    RegistryFile {
        version: CURRENT_VERSION,
        sources: vec![Source {
            id: DEFAULT_HUB_ID.into(),
            label: DEFAULT_HUB_LABEL.into(),
            url: DEFAULT_HUB_URL.into(),
            kind: SourceKind::Github,
            added_at: 0,
            last_refreshed_at: None,
            auto_check: false,
        }],
    }
}

pub fn read(hub_dir: &Path) -> Result<RegistryFile, RegistryError> {
    std::fs::create_dir_all(hub_dir)
        .map_err(|e| RegistryError::Io(format!("create_dir_all {}: {}", hub_dir.display(), e)))?;

    let main = hub_dir.join("sources.json");
    if main.exists() {
        let text = std::fs::read_to_string(&main)
            .map_err(|e| RegistryError::Io(format!("read {}: {}", main.display(), e)))?;
        match serde_json::from_str::<RegistryFile>(&text) {
            Ok(parsed) if parsed.version == CURRENT_VERSION => return Ok(parsed),
            _ => {
                let bak = hub_dir.join("sources.json.bak");
                let _ = std::fs::rename(&main, &bak);
                return Ok(default_registry());
            }
        }
    }

    let tmp = hub_dir.join("sources.json.tmp");
    if tmp.exists() {
        if let Ok(text) = std::fs::read_to_string(&tmp) {
            if let Ok(parsed) = serde_json::from_str::<RegistryFile>(&text) {
                if parsed.version == CURRENT_VERSION {
                    let _ = std::fs::rename(&tmp, &main);
                    return Ok(parsed);
                }
            }
        }
    }

    Ok(default_registry())
}

pub fn write(hub_dir: &Path, file: &RegistryFile) -> Result<(), RegistryError> {
    std::fs::create_dir_all(hub_dir)
        .map_err(|e| RegistryError::Io(format!("create_dir_all {}: {}", hub_dir.display(), e)))?;

    let text = serde_json::to_string_pretty(file)
        .map_err(|e| RegistryError::Json(format!("serialize: {}", e)))?;

    let main = hub_dir.join("sources.json");
    let tmp = hub_dir.join("sources.json.tmp");
    std::fs::write(&tmp, text)
        .map_err(|e| RegistryError::Io(format!("write {}: {}", tmp.display(), e)))?;
    std::fs::rename(&tmp, &main)
        .map_err(|e| RegistryError::Io(format!("rename {} -> {}: {}", tmp.display(), main.display(), e)))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tempdir() -> PathBuf {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "openusage-hub-registry-test-{}-{}",
            std::process::id(),
            suffix
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn sample_source(id: &str) -> Source {
        Source {
            id: id.into(),
            label: format!("Label {}", id),
            url: "https://github.com/foo/bar".into(),
            kind: SourceKind::Github,
            added_at: 1234567890,
            last_refreshed_at: Some(1234567891),
            auto_check: true,
        }
    }

    #[test]
    fn read_missing_returns_default_with_upstream_source() {
        let dir = tempdir();
        let file = read(&dir).expect("read should succeed");
        assert_eq!(file.version, CURRENT_VERSION);
        assert_eq!(file.sources.len(), 1);
        assert_eq!(file.sources[0].url, DEFAULT_HUB_URL);
        assert_eq!(file.sources[0].kind, SourceKind::Github);
        assert_eq!(file.sources[0].label, DEFAULT_HUB_LABEL);
        assert_eq!(file.sources[0].id, DEFAULT_HUB_ID);
        assert!(!file.sources[0].auto_check);
    }

    #[test]
    fn round_trip_preserves_all_fields() {
        let dir = tempdir();
        let original = RegistryFile {
            version: CURRENT_VERSION,
            sources: vec![sample_source("abc"), sample_source("xyz")],
        };
        write(&dir, &original).expect("write should succeed");
        let loaded = read(&dir).expect("read should succeed");
        assert_eq!(loaded.version, original.version);
        assert_eq!(loaded.sources, original.sources);
    }

    #[test]
    fn read_version_zero_backs_up_and_returns_default() {
        let dir = tempdir();
        let bad = serde_json::json!({ "version": 0, "sources": [] });
        fs::write(
            dir.join("sources.json"),
            serde_json::to_string(&bad).unwrap(),
        )
        .unwrap();

        let file = read(&dir).expect("read should succeed");
        assert_eq!(file.version, CURRENT_VERSION);
        assert_eq!(file.sources.len(), 1);
        assert_eq!(file.sources[0].url, DEFAULT_HUB_URL);
        assert!(dir.join("sources.json.bak").exists());
    }

    #[test]
    fn read_recovers_from_tmp_when_sources_json_missing() {
        let dir = tempdir();
        let recovered = RegistryFile {
            version: CURRENT_VERSION,
            sources: vec![sample_source("recovered")],
        };
        let tmp_path = dir.join("sources.json.tmp");
        fs::write(
            &tmp_path,
            serde_json::to_string(&recovered).unwrap(),
        )
        .unwrap();

        let file = read(&dir).expect("read should succeed");
        assert_eq!(file.sources.len(), 1);
        assert_eq!(file.sources[0].id, "recovered");
    }

    #[test]
    fn write_does_not_leave_tmp_on_success() {
        let dir = tempdir();
        let file = RegistryFile {
            version: CURRENT_VERSION,
            sources: vec![],
        };
        write(&dir, &file).expect("write should succeed");
        assert!(!dir.join("sources.json.tmp").exists());
        assert!(dir.join("sources.json").exists());
    }

    #[test]
    fn write_creates_hub_dir_if_missing() {
        let dir = tempdir();
        let nested = dir.join("does/not/exist");
        assert!(!nested.exists());
        write(
            &nested,
            &RegistryFile {
                version: CURRENT_VERSION,
                sources: vec![],
            },
        )
        .expect("write should succeed");
        assert!(nested.join("sources.json").exists());
    }
}