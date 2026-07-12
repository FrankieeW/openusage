use std::collections::{HashMap, HashSet};

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EnvOverrideDto {
    name: String,
    kind: String,
    value: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct EnvGroupDto {
    #[allow(dead_code)]
    id: String,
    #[allow(dead_code)]
    name: String,
    enabled: bool,
    overrides: Vec<EnvGroupOverrideDto>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct EnvGroupOverrideDto {
    name: String,
    value: String,
}

/// Flatten groups (frontend-compatible logic). Active groups only;
/// `$REF` -> reference, `$$X` -> literal `$X`, everything else -> literal.
/// Conflicts (same name in >1 active group) become `[CONFLICT: NAME]`.
fn flatten_env_groups(groups: &[EnvGroupDto]) -> Vec<EnvOverrideDto> {
    flatten_selected_env_groups(groups.iter().filter(|group| group.enabled))
}

fn flatten_legacy_env_groups(groups: &[EnvGroupDto], active_ids: &[String]) -> Vec<EnvOverrideDto> {
    let active_set: HashSet<&str> = active_ids.iter().map(|id| id.as_str()).collect();
    flatten_selected_env_groups(
        groups
            .iter()
            .filter(|group| active_set.contains(group.id.as_str())),
    )
}

fn flatten_selected_env_groups<'a>(
    groups: impl Iterator<Item = &'a EnvGroupDto>,
) -> Vec<EnvOverrideDto> {
    let mut map: HashMap<String, Option<(String, String)>> = HashMap::new();

    for group in groups {
        for o in &group.overrides {
            if o.name.is_empty() || o.value.is_empty() {
                continue;
            }
            let (kind, val) = if o.value.starts_with("$$") {
                ("literal".to_string(), o.value[1..].to_string())
            } else if o.value.starts_with('$') && o.value.len() > 1 {
                ("reference".to_string(), o.value[1..].to_string())
            } else {
                ("literal".to_string(), o.value.clone())
            };
            if kind == "literal" && val.is_empty() {
                continue;
            }
            if map.contains_key(&o.name) {
                map.insert(o.name.clone(), None);
            } else {
                map.insert(o.name.clone(), Some((kind, val)));
            }
        }
    }

    map.into_iter()
        .map(|(name, entry)| match entry {
            Some((kind, value)) => EnvOverrideDto { name, kind, value },
            None => EnvOverrideDto {
                name: name.clone(),
                kind: "literal".to_string(),
                value: format!("[CONFLICT: {}]", name),
            },
        })
        .collect()
}

fn map_env_overrides(
    dtos: Vec<EnvOverrideDto>,
) -> Vec<crate::plugin_engine::host_api::EnvOverrideInput> {
    use crate::plugin_engine::host_api::{EnvOverrideInput, EnvOverrideKind};

    dtos.into_iter()
        .filter_map(|dto| {
            let kind = match dto.kind.as_str() {
                "literal" => EnvOverrideKind::Literal,
                "reference" => EnvOverrideKind::Reference,
                other => {
                    log::warn!("Ignoring env override with unknown kind: {}", other);
                    return None;
                }
            };
            Some(EnvOverrideInput {
                name: dto.name,
                kind,
                value: dto.value,
            })
        })
        .collect()
}

pub(crate) fn apply_unsafe_env_setting(app_handle: &tauri::AppHandle) {
    use tauri_plugin_store::StoreExt;

    let enabled = match app_handle.store("settings.json") {
        Ok(store) => store
            .get("unsafeAllowAllEnv")
            .and_then(|value| value.as_bool())
            .unwrap_or(false),
        Err(error) => {
            log::warn!("Failed to read unsafeAllowAllEnv from settings: {}", error);
            false
        }
    };
    crate::plugin_engine::host_api::set_allow_all_env(enabled);
}

pub(crate) fn apply_env_overrides(app_handle: &tauri::AppHandle) {
    use tauri_plugin_store::StoreExt;

    let store = match app_handle.store("env.json") {
        Ok(store) => store,
        Err(error) => {
            log::warn!("Failed to open env.json: {}", error);
            return;
        }
    };

    let raw_groups = store.get("groups");
    let schema_version = store
        .get("envSchemaVersion")
        .and_then(|value| value.as_u64())
        .unwrap_or(0);

    let groups: Vec<EnvGroupDto> = match raw_groups.and_then(|v| serde_json::from_value(v).ok()) {
        Some(g) => g,
        None => return,
    };

    let dtos = if schema_version >= 2 {
        flatten_env_groups(&groups)
    } else {
        match store
            .get("activeGroupIds")
            .and_then(|v| serde_json::from_value::<Vec<String>>(v).ok())
        {
            Some(active_ids) => flatten_legacy_env_groups(&groups, &active_ids),
            None => flatten_env_groups(&groups),
        }
    };
    crate::plugin_engine::host_api::set_env_overrides(map_env_overrides(dtos));
}

#[tauri::command]
pub(crate) fn set_allow_all_env(enabled: bool) {
    crate::plugin_engine::host_api::set_allow_all_env(enabled);
}

#[tauri::command]
pub(crate) fn set_env_overrides(overrides: Vec<EnvOverrideDto>) {
    crate::plugin_engine::host_api::set_env_overrides(map_env_overrides(overrides));
}

#[cfg(test)]
mod tests {
    use super::{EnvGroupDto, EnvGroupOverrideDto, flatten_env_groups, flatten_legacy_env_groups};

    #[test]
    fn flatten_env_groups_uses_enabled_as_source_of_truth() {
        let groups = vec![
            EnvGroupDto {
                id: "enabled".to_string(),
                name: "Enabled".to_string(),
                enabled: true,
                overrides: vec![EnvGroupOverrideDto {
                    name: "OPENUSAGE_ENABLED".to_string(),
                    value: "yes".to_string(),
                }],
            },
            EnvGroupDto {
                id: "disabled".to_string(),
                name: "Disabled".to_string(),
                enabled: false,
                overrides: vec![EnvGroupOverrideDto {
                    name: "OPENUSAGE_DISABLED".to_string(),
                    value: "no".to_string(),
                }],
            },
        ];

        let flattened = flatten_env_groups(&groups);

        assert_eq!(flattened.len(), 1);
        assert_eq!(flattened[0].name, "OPENUSAGE_ENABLED");
    }

    #[test]
    fn flatten_legacy_env_groups_uses_active_ids() {
        let groups = vec![EnvGroupDto {
            id: "legacy-active".to_string(),
            name: "Legacy Active".to_string(),
            enabled: false,
            overrides: vec![EnvGroupOverrideDto {
                name: "OPENUSAGE_LEGACY".to_string(),
                value: "yes".to_string(),
            }],
        }];

        let flattened = flatten_legacy_env_groups(&groups, &["legacy-active".to_string()]);

        assert_eq!(flattened.len(), 1);
        assert_eq!(flattened[0].name, "OPENUSAGE_LEGACY");
    }
}
