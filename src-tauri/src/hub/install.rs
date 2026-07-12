#![allow(unused_imports)] // Compatibility re-exports preserve the existing install API.

mod error;
mod filesystem;
mod names;
mod package;
mod sweep;
mod validation;

pub use error::InstallError;
pub use filesystem::{
    copy_dir_to, hub_trash_dir, remove_installed_plugin, switch_plugin_install_dir_with_metadata,
};
#[cfg(test)]
pub use filesystem::{copy_plugin_to_install_dir, switch_plugin_install_dir};
pub use names::{HUB_TRASH_DIRNAME, is_internal_dir_name};
pub use package::{
    INSTALL_METADATA_SCHEMA_VERSION, InstallMetadata, METADATA_FILENAME, package_hash,
    read_install_metadata, write_install_metadata,
};
pub use sweep::{OrphanReport, startup_sweep};
pub use validation::{check_conflict, validate_entry_within_dir, validate_id_match};

#[cfg(test)]
mod tests;
