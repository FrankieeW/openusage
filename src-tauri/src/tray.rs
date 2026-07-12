use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::path::BaseDirectory;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};
use tauri_nspanel::ManagerExt;
use tauri_plugin_store::StoreExt;

#[cfg(target_os = "macos")]
use objc2::msg_send;

use crate::log_path;
use crate::panel::{get_or_init_panel, position_panel_at_tray_icon, show_panel};

const LOG_LEVEL_STORE_KEY: &str = "logLevel";
const DEFAULT_LOG_LEVEL: log::LevelFilter = log::LevelFilter::Error;
const LOG_LEVEL_MENU_ITEMS: [(&str, log::LevelFilter); 5] = [
    ("log_error", log::LevelFilter::Error),
    ("log_warn", log::LevelFilter::Warn),
    ("log_info", log::LevelFilter::Info),
    ("log_debug", log::LevelFilter::Debug),
    ("log_trace", log::LevelFilter::Trace),
];

struct LogLevelMenuItems {
    items: [(CheckMenuItem<tauri::Wry>, log::LevelFilter); 5],
}

impl LogLevelMenuItems {
    fn set_checked_level(&self, selected_level: log::LevelFilter) -> Result<(), String> {
        let checks = checked_log_level_items(selected_level);
        let mut first_error: Option<String> = None;

        for ((item, _), (_, checked)) in self.items.iter().zip(checks) {
            if let Err(error) = item.set_checked(checked) {
                let message = error.to_string();
                log::error!("Failed to sync debug level menu item: {}", message);
                if first_error.is_none() {
                    first_error = Some(message);
                }
            }
        }

        match first_error {
            Some(error) => Err(format!("failed to sync debug level menu: {}", error)),
            None => Ok(()),
        }
    }
}

fn checked_log_level_items(selected_level: log::LevelFilter) -> [(&'static str, bool); 5] {
    LOG_LEVEL_MENU_ITEMS.map(|(id, level)| (id, level == selected_level))
}

pub fn parse_log_level(value: &str) -> Option<log::LevelFilter> {
    match value.trim().to_ascii_lowercase().as_str() {
        "error" => Some(log::LevelFilter::Error),
        "warn" => Some(log::LevelFilter::Warn),
        "info" => Some(log::LevelFilter::Info),
        "debug" => Some(log::LevelFilter::Debug),
        "trace" => Some(log::LevelFilter::Trace),
        _ => None,
    }
}

pub fn log_level_to_str(level: log::LevelFilter) -> &'static str {
    match level {
        log::LevelFilter::Error => "error",
        log::LevelFilter::Warn => "warn",
        log::LevelFilter::Info => "info",
        log::LevelFilter::Debug => "debug",
        log::LevelFilter::Trace => "trace",
        // `Off` is never persisted or exposed (the default is Error and
        // `parse_log_level` never yields Off); map it to "error" defensively.
        log::LevelFilter::Off => "error",
    }
}

pub fn get_stored_log_level(app_handle: &AppHandle) -> log::LevelFilter {
    let store = match app_handle.store("settings.json") {
        Ok(s) => s,
        Err(error) => {
            log::warn!("failed to open settings store for log level: {}", error);
            return DEFAULT_LOG_LEVEL;
        }
    };
    let value = store.get(LOG_LEVEL_STORE_KEY);
    let level_str = value.and_then(|v| v.as_str().map(|s| s.to_string()));
    level_str
        .as_deref()
        .and_then(parse_log_level)
        .unwrap_or(DEFAULT_LOG_LEVEL)
}

pub fn get_stored_log_level_value(app_handle: &AppHandle) -> &'static str {
    log_level_to_str(get_stored_log_level(app_handle))
}

pub fn set_stored_log_level(app_handle: &AppHandle, level: log::LevelFilter) -> Result<(), String> {
    let level_str = log_level_to_str(level);
    log::info!("Log level changing to {:?}", level);
    let store = app_handle
        .store("settings.json")
        .map_err(|error| format!("failed to open settings store: {}", error))?;
    store.set(LOG_LEVEL_STORE_KEY, serde_json::json!(level_str));
    store
        .save()
        .map_err(|error| format!("failed to save settings store: {}", error))?;
    log::set_max_level(level);
    Ok(())
}

pub fn sync_log_level_menu(app_handle: &AppHandle, level: log::LevelFilter) -> Result<(), String> {
    let Some(menu_items) = app_handle.try_state::<LogLevelMenuItems>() else {
        let message = "debug level menu state is not initialized";
        log::error!("{}", message);
        return Err(message.to_string());
    };

    menu_items.set_checked_level(level)
}

pub fn save_log_level(app_handle: &AppHandle, level: log::LevelFilter) -> Result<(), String> {
    set_stored_log_level(app_handle, level)?;
    sync_log_level_menu(app_handle, level)
}

pub fn create(app_handle: &AppHandle) -> tauri::Result<()> {
    let tray_icon_path = app_handle
        .path()
        .resolve("icons/tray-icon.png", BaseDirectory::Resource)?;
    let icon = Image::from_path(tray_icon_path)?;

    // Load persisted log level
    let current_level = get_stored_log_level(app_handle);
    log::set_max_level(current_level);

    let show_stats = MenuItem::with_id(app_handle, "show_stats", "Show Stats", true, None::<&str>)?;
    let go_to_settings = MenuItem::with_id(
        app_handle,
        "go_to_settings",
        "Go to Settings",
        true,
        None::<&str>,
    )?;

    // Log level submenu items are shared with Settings so both surfaces keep
    // the same checked state.
    let log_error = CheckMenuItem::with_id(
        app_handle,
        "log_error",
        "Error",
        true,
        current_level == log::LevelFilter::Error,
        None::<&str>,
    )?;
    let log_warn = CheckMenuItem::with_id(
        app_handle,
        "log_warn",
        "Warn",
        true,
        current_level == log::LevelFilter::Warn,
        None::<&str>,
    )?;
    let log_info = CheckMenuItem::with_id(
        app_handle,
        "log_info",
        "Info",
        true,
        current_level == log::LevelFilter::Info,
        None::<&str>,
    )?;
    let log_debug = CheckMenuItem::with_id(
        app_handle,
        "log_debug",
        "Debug",
        true,
        current_level == log::LevelFilter::Debug,
        None::<&str>,
    )?;
    let log_trace = CheckMenuItem::with_id(
        app_handle,
        "log_trace",
        "Trace",
        true,
        current_level == log::LevelFilter::Trace,
        None::<&str>,
    )?;
    let log_level_separator = PredefinedMenuItem::separator(app_handle)?;
    let copy_log_path = MenuItem::with_id(
        app_handle,
        "copy_log_path",
        "Copy Log Path",
        true,
        None::<&str>,
    )?;
    let log_level_submenu = Submenu::with_items(
        app_handle,
        "Debug Level",
        true,
        &[
            &log_error,
            &log_warn,
            &log_info,
            &log_debug,
            &log_trace,
            &log_level_separator,
            &copy_log_path,
        ],
    )?;

    let log_items = LogLevelMenuItems {
        items: [
            (log_error.clone(), log::LevelFilter::Error),
            (log_warn.clone(), log::LevelFilter::Warn),
            (log_info.clone(), log::LevelFilter::Info),
            (log_debug.clone(), log::LevelFilter::Debug),
            (log_trace.clone(), log::LevelFilter::Trace),
        ],
    };
    if !app_handle.manage(log_items) {
        log::error!("Debug level menu state was already initialized");
    }

    let separator = PredefinedMenuItem::separator(app_handle)?;
    let about = MenuItem::with_id(app_handle, "about", "About OpenUsage", true, None::<&str>)?;
    let quit = MenuItem::with_id(app_handle, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(
        app_handle,
        &[
            &show_stats,
            &go_to_settings,
            &log_level_submenu,
            &separator,
            &about,
            &quit,
        ],
    )?;

    #[cfg(target_os = "macos")]
    let context_menu = menu.clone();

    let tray_builder = TrayIconBuilder::with_id("tray")
        .icon(icon)
        .tooltip("OpenUsage")
        .show_menu_on_left_click(false)
        .on_menu_event(move |app_handle, event| {
            log::debug!("tray menu: {}", event.id.as_ref());
            match event.id.as_ref() {
                "show_stats" => {
                    show_panel(app_handle);
                    let _ = app_handle.emit("tray:navigate", "home");
                }
                "go_to_settings" => {
                    show_panel(app_handle);
                    let _ = app_handle.emit("tray:navigate", "settings");
                }
                "about" => {
                    show_panel(app_handle);
                    let _ = app_handle.emit("tray:show-about", ());
                }
                "quit" => {
                    log::info!("quit requested via tray");
                    app_handle.exit(0);
                }
                "log_error" | "log_warn" | "log_info" | "log_debug" | "log_trace" => {
                    let selected_level = match event.id.as_ref() {
                        "log_error" => log::LevelFilter::Error,
                        "log_warn" => log::LevelFilter::Warn,
                        "log_info" => log::LevelFilter::Info,
                        "log_debug" => log::LevelFilter::Debug,
                        "log_trace" => log::LevelFilter::Trace,
                        _ => unreachable!(),
                    };
                    if let Err(error) = save_log_level(app_handle, selected_level) {
                        log::error!("tray menu: failed to set log level: {}", error);
                    } else {
                        // Notify the frontend so the Settings page reflects the
                        // change made from the tray menu.
                        if let Err(error) =
                            app_handle.emit("tray:log-level", log_level_to_str(selected_level))
                        {
                            log::error!("failed to emit tray log level update: {}", error);
                        }
                    }
                }
                "copy_log_path" => match log_path::copy_to_clipboard(app_handle) {
                    Ok(()) => {
                        log::info!("copied log path to clipboard");
                    }
                    Err(error) => {
                        log::error!("failed to copy log path to clipboard: {}", error);
                    }
                },
                _ => {}
            }
        })
        .on_tray_icon_event(move |tray, event| {
            let TrayIconEvent::Click {
                button,
                button_state,
                rect,
                ..
            } = event
            else {
                return;
            };
            if button_state != MouseButtonState::Up {
                return;
            }

            match button {
                MouseButton::Left => {
                    let app_handle = tray.app_handle();
                    let Some(panel) = get_or_init_panel!(app_handle) else {
                        return;
                    };
                    if panel.is_visible() {
                        log::debug!("tray click: hiding panel");
                        panel.hide();
                        return;
                    }
                    log::debug!("tray click: showing panel");
                    panel.show_and_make_key();
                    position_panel_at_tray_icon(app_handle, rect.position, rect.size);
                }
                MouseButton::Right => {
                    #[cfg(target_os = "macos")]
                    {
                        if let Err(error) = tray.set_menu(Some(context_menu.clone())) {
                            log::error!(
                                "tray right-click: failed to attach context menu: {}",
                                error
                            );
                            return;
                        }
                        let _ = tray.with_inner_tray_icon(|inner| {
                            let mtm = objc2_foundation::MainThreadMarker::new()
                                .expect("with_inner_tray_icon closure must run on the main thread");
                            let Some(status) = inner.ns_status_item() else {
                                return;
                            };
                            let Some(menu) = status.menu(mtm) else {
                                return;
                            };
                            let button = status.button(mtm).unwrap();
                            let _: () = unsafe { msg_send![&button, popUpStatusItemMenu: &*menu] };
                        });
                        if let Err(error) = tray.set_menu::<Menu<tauri::Wry>>(None) {
                            log::error!(
                                "tray right-click: failed to detach context menu: {}",
                                error
                            );
                        }
                    }
                }
                MouseButton::Middle => {}
            }
        });

    #[cfg(not(target_os = "macos"))]
    let tray_builder = tray_builder.menu(&menu);

    let tray = tray_builder.build(app_handle)?;

    // macOS 27 click-routing override (issue #573): do not attach the menu to
    // NSStatusItem. A directly attached menu can preempt left-click handling and
    // open the context menu before the usage panel toggle sees the click. The
    // right-click path attaches the menu only while showing it.
    #[cfg(target_os = "macos")]
    {
        let _ = tray.with_inner_tray_icon(|inner| {
            inner.set_show_menu_on_right_click(false);
            let Some(status) = inner.ns_status_item() else {
                return;
            };
            // with_inner_tray_icon runs the closure on the main thread
            // (see tauri 2.11.2 src/tray/mod.rs `run_item_main_thread!`),
            // so constructing a MainThreadMarker here is sound.
            let mtm = objc2_foundation::MainThreadMarker::new()
                .expect("with_inner_tray_icon closure must run on the main thread");
            unsafe {
                let button = status.button(mtm).unwrap();
                button.setTarget(None);
                button.setAction(None);
            }
        });
        log::info!("Applied macOS tray click-routing override");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{checked_log_level_items, log_level_to_str, parse_log_level};

    #[test]
    fn parses_supported_log_levels() {
        assert_eq!(parse_log_level("error"), Some(log::LevelFilter::Error));
        assert_eq!(parse_log_level("warn"), Some(log::LevelFilter::Warn));
        assert_eq!(parse_log_level("info"), Some(log::LevelFilter::Info));
        assert_eq!(parse_log_level("debug"), Some(log::LevelFilter::Debug));
        assert_eq!(parse_log_level("trace"), Some(log::LevelFilter::Trace));
    }

    #[test]
    fn rejects_unsupported_log_levels() {
        assert_eq!(parse_log_level("off"), None);
        assert_eq!(parse_log_level("verbose"), None);
        assert_eq!(parse_log_level(""), None);
    }

    #[test]
    fn serializes_runtime_log_levels() {
        assert_eq!(log_level_to_str(log::LevelFilter::Error), "error");
        assert_eq!(log_level_to_str(log::LevelFilter::Warn), "warn");
        assert_eq!(log_level_to_str(log::LevelFilter::Info), "info");
        assert_eq!(log_level_to_str(log::LevelFilter::Debug), "debug");
        assert_eq!(log_level_to_str(log::LevelFilter::Trace), "trace");
    }

    #[test]
    fn checks_only_the_selected_log_level_menu_item() {
        assert_eq!(
            checked_log_level_items(log::LevelFilter::Debug),
            [
                ("log_error", false),
                ("log_warn", false),
                ("log_info", false),
                ("log_debug", true),
                ("log_trace", false),
            ],
        );
    }
}
