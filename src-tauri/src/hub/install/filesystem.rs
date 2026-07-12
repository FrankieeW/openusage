use std::path::{Path, PathBuf};

use super::error::InstallError;
use super::names::{HUB_INSTALL_TMP_PREFIX, HUB_TRASH_DIRNAME};
use super::package::{InstallMetadata, should_skip_package_file, write_install_metadata_file};
use super::validation::validate_copied_plugin;

pub fn switch_plugin_install_dir_with_metadata(
    source_plugin_dir: &Path,
    install_dir: &Path,
    old_dir_name: &str,
    new_dir_name: &str,
    metadata: &InstallMetadata,
) -> Result<(), InstallError> {
    std::fs::create_dir_all(install_dir).map_err(|error| InstallError::Io(error.to_string()))?;

    let temp = unique_install_temp_dir(install_dir, new_dir_name);
    copy_dir_recursive(source_plugin_dir, &temp).map_err(|error| {
        cleanup_dir_best_effort(&temp);
        InstallError::Io(error.to_string())
    })?;
    if let Err(error) = validate_copied_plugin(&temp, new_dir_name) {
        cleanup_dir_best_effort(&temp);
        return Err(error);
    }
    if let Err(error) = write_install_metadata_file(&temp, metadata) {
        cleanup_dir_best_effort(&temp);
        return Err(error);
    }

    finish_switch_plugin_install_dir(temp, install_dir, old_dir_name, new_dir_name)
}

#[cfg(test)]
pub fn copy_plugin_to_install_dir(
    source_plugin_dir: &Path,
    install_dir: &Path,
    plugin_id: &str,
) -> Result<(), InstallError> {
    switch_plugin_install_dir(source_plugin_dir, install_dir, plugin_id, plugin_id)
}

#[cfg(test)]
pub fn switch_plugin_install_dir(
    source_plugin_dir: &Path,
    install_dir: &Path,
    old_dir_name: &str,
    new_dir_name: &str,
) -> Result<(), InstallError> {
    std::fs::create_dir_all(install_dir).map_err(|error| InstallError::Io(error.to_string()))?;

    let temp = unique_install_temp_dir(install_dir, new_dir_name);
    copy_dir_recursive(source_plugin_dir, &temp).map_err(|error| {
        cleanup_dir_best_effort(&temp);
        InstallError::Io(error.to_string())
    })?;
    if let Err(error) = validate_copied_plugin(&temp, new_dir_name) {
        cleanup_dir_best_effort(&temp);
        return Err(error);
    }

    finish_switch_plugin_install_dir(temp, install_dir, old_dir_name, new_dir_name)
}

fn finish_switch_plugin_install_dir(
    temp: PathBuf,
    install_dir: &Path,
    old_dir_name: &str,
    new_dir_name: &str,
) -> Result<(), InstallError> {
    let mut moved_dirs: Vec<(PathBuf, PathBuf)> = Vec::new();
    let old_path = install_dir.join(old_dir_name);
    if old_path.exists() {
        let backup = unique_trash_dir(install_dir, old_dir_name);
        move_dir_for_replace(&old_path, &backup)?;
        moved_dirs.push((old_path, backup));
    }

    let dest = install_dir.join(new_dir_name);
    if new_dir_name != old_dir_name && dest.exists() {
        let backup = unique_trash_dir(install_dir, new_dir_name);
        move_dir_for_replace(&dest, &backup)?;
        moved_dirs.push((dest.clone(), backup));
    }

    if let Err(error) = std::fs::rename(&temp, &dest) {
        for (original, backup) in moved_dirs.iter().rev() {
            let _ = std::fs::rename(backup, original);
        }
        cleanup_dir_best_effort(&temp);
        return Err(InstallError::Io(error.to_string()));
    }

    Ok(())
}

pub fn remove_installed_plugin(install_dir: &Path, plugin_id: &str) -> Result<(), InstallError> {
    let path = install_dir.join(plugin_id);
    if path.exists() {
        let backup = unique_trash_dir(install_dir, plugin_id);
        if let Some(parent) = backup.parent() {
            std::fs::create_dir_all(parent).map_err(|error| InstallError::Io(error.to_string()))?;
        }
        std::fs::rename(&path, &backup).map_err(|error| InstallError::Io(error.to_string()))?;
    }
    Ok(())
}

pub fn hub_trash_dir(install_dir: &Path) -> PathBuf {
    install_dir.join(HUB_TRASH_DIRNAME)
}

fn unique_install_temp_dir(install_dir: &Path, plugin_id: &str) -> PathBuf {
    unique_child_path(
        install_dir,
        &format!(
            "{}-{}",
            HUB_INSTALL_TMP_PREFIX,
            sanitize_backup_component(plugin_id)
        ),
    )
}

fn unique_trash_dir(install_dir: &Path, plugin_id: &str) -> PathBuf {
    unique_child_path(
        &hub_trash_dir(install_dir),
        &sanitize_backup_component(plugin_id),
    )
}

fn move_dir_for_replace(from: &Path, to: &Path) -> Result<(), InstallError> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).map_err(|error| InstallError::Io(error.to_string()))?;
    }
    std::fs::rename(from, to).map_err(|error| InstallError::Io(error.to_string()))
}

fn unique_child_path(parent: &Path, prefix: &str) -> PathBuf {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let counter = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    parent.join(format!(
        "{}-{}-{}-{}",
        prefix,
        std::process::id(),
        nanos,
        counter
    ))
}

fn sanitize_backup_component(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '-'
            }
        })
        .collect()
}

fn cleanup_dir_best_effort(path: &Path) {
    if path.exists() {
        let Some(parent) = path.parent() else {
            return;
        };
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("install-temp");
        let backup = unique_child_path(&hub_trash_dir(parent), &sanitize_backup_component(name));
        if let Some(trash_parent) = backup.parent() {
            let _ = std::fs::create_dir_all(trash_parent);
        }
        let _ = std::fs::rename(path, backup);
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let entry_path = entry.path();
        let dest_path = dst.join(&name);
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            copy_dir_recursive(&entry_path, &dest_path)?;
        } else if file_type.is_file() {
            if should_skip_package_file(&name_str) {
                continue;
            }
            std::fs::copy(&entry_path, &dest_path)?;
        }
    }
    Ok(())
}

/// Public wrapper used by the Tauri command layer (e.g. when seeding a local
/// path source's cache).
pub fn copy_dir_to(src: &Path, dst: &Path) -> Result<(), InstallError> {
    copy_dir_recursive(src, dst).map_err(|error| InstallError::Io(error.to_string()))
}
