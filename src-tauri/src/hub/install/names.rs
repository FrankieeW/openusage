pub const HUB_TRASH_DIRNAME: &str = ".openusage-trash";
pub(super) const HUB_INSTALL_TMP_PREFIX: &str = ".openusage-installing";

pub fn is_internal_dir_name(name: &str) -> bool {
    name == HUB_TRASH_DIRNAME || name.starts_with(HUB_INSTALL_TMP_PREFIX)
}

pub(super) fn plugin_id_from_install_dir_name(dir_name: &str) -> &str {
    dir_name
        .split_once("__")
        .map(|(id, _)| id)
        .unwrap_or(dir_name)
}
