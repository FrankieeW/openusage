#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallError {
    IdMismatch {
        dir_name: String,
        manifest_id: String,
    },
    EntryOutsidePluginDir,
    ConflictWithSource(String),
    ConflictUnmanaged,
    ManifestParse(String),
    Io(String),
}

impl std::fmt::Display for InstallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IdMismatch {
                dir_name,
                manifest_id,
            } => {
                write!(f, "id mismatch: dir={} manifest={}", dir_name, manifest_id)
            }
            Self::EntryOutsidePluginDir => write!(f, "entry path escapes plugin dir"),
            Self::ConflictWithSource(source_id) => {
                write!(f, "already installed from {}", source_id)
            }
            Self::ConflictUnmanaged => write!(f, "already installed outside Hub"),
            Self::ManifestParse(message) => write!(f, "manifest parse: {}", message),
            Self::Io(message) => write!(f, "io: {}", message),
        }
    }
}

impl std::error::Error for InstallError {}
