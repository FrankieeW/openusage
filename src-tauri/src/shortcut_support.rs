#[cfg(desktop)]
use std::sync::{Mutex, OnceLock};

#[cfg(desktop)]
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

#[cfg(desktop)]
const GLOBAL_SHORTCUT_STORE_KEY: &str = "globalShortcut";

#[cfg(desktop)]
fn managed_shortcut_slot() -> &'static Mutex<Option<String>> {
    static SLOT: OnceLock<Mutex<Option<String>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

/// Shared shortcut handler that toggles the panel when the shortcut is pressed.
#[cfg(desktop)]
fn handle_global_shortcut(
    app: &tauri::AppHandle,
    event: tauri_plugin_global_shortcut::ShortcutEvent,
) {
    if event.state == ShortcutState::Pressed {
        log::debug!("Global shortcut triggered");
        crate::panel::toggle_panel(app);
    }
}

/// Update the global shortcut registration.
/// Pass `null` to disable the shortcut, or a shortcut string like "CommandOrControl+Shift+U".
#[cfg(desktop)]
#[tauri::command]
pub(crate) fn update_global_shortcut(
    app_handle: tauri::AppHandle,
    shortcut: Option<String>,
) -> Result<(), String> {
    let global_shortcut = app_handle.global_shortcut();
    let normalized_shortcut = shortcut.and_then(|value| {
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    });
    let mut managed_shortcut = managed_shortcut_slot()
        .lock()
        .map_err(|e| format!("failed to lock managed shortcut state: {}", e))?;

    if *managed_shortcut == normalized_shortcut {
        log::debug!("Global shortcut unchanged");
        return Ok(());
    }

    let previous_shortcut = managed_shortcut.clone();
    if let Some(existing) = previous_shortcut.as_deref() {
        match global_shortcut.unregister(existing) {
            Ok(()) => {
                *managed_shortcut = None;
            }
            Err(e) => {
                log::warn!(
                    "Failed to unregister existing shortcut '{}': {}",
                    existing,
                    e
                );
            }
        }
    }

    if let Some(shortcut) = normalized_shortcut {
        log::info!("Registering global shortcut: {}", shortcut);
        global_shortcut
            .on_shortcut(shortcut.as_str(), |app, _shortcut, event| {
                handle_global_shortcut(app, event);
            })
            .map_err(|e| format!("Failed to register shortcut '{}': {}", shortcut, e))?;
        *managed_shortcut = Some(shortcut);
    } else {
        log::info!("Global shortcut disabled");
        *managed_shortcut = None;
    }

    Ok(())
}

#[cfg(desktop)]
pub(crate) fn register_initial_global_shortcut(app_handle: &tauri::AppHandle) {
    use tauri_plugin_store::StoreExt;

    if let Ok(store) = app_handle.store("settings.json")
        && let Some(shortcut_value) = store.get(GLOBAL_SHORTCUT_STORE_KEY)
        && let Some(shortcut) = shortcut_value.as_str()
    {
        let shortcut = shortcut.trim();
        if !shortcut.is_empty() {
            let handle = app_handle.clone();
            log::info!("Registering initial global shortcut: {}", shortcut);
            if let Err(e) =
                handle
                    .global_shortcut()
                    .on_shortcut(shortcut, |app, _shortcut, event| {
                        handle_global_shortcut(app, event);
                    })
            {
                log::warn!("Failed to register initial global shortcut: {}", e);
            } else if let Ok(mut managed_shortcut) = managed_shortcut_slot().lock() {
                *managed_shortcut = Some(shortcut.to_string());
            } else {
                log::warn!("Failed to store managed shortcut in memory");
            }
        }
    }
}
