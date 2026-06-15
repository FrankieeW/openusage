use tauri::{AppHandle, Manager, Position, Size};

fn monitor_contains_physical_point(
    origin_x: f64,
    origin_y: f64,
    width: f64,
    height: f64,
    point_x: f64,
    point_y: f64,
) -> bool {
    point_x >= origin_x
        && point_x < origin_x + width
        && point_y >= origin_y
        && point_y < origin_y + height
}

/// Retrieve the tray icon rect and position the window beneath it.
/// No-ops gracefully if the tray icon or its rect is unavailable.
fn position_panel_from_tray(app_handle: &AppHandle) {
    let Some(tray) = app_handle.tray_by_id("tray") else {
        log::debug!("position_panel_from_tray: tray icon not found");
        return;
    };
    match tray.rect() {
        Ok(Some(rect)) => {
            position_panel_at_tray_icon(app_handle, rect.position, rect.size);
        }
        Ok(None) => {
            log::debug!("position_panel_from_tray: tray rect not available yet");
        }
        Err(e) => {
            log::warn!("position_panel_from_tray: failed to get tray rect: {}", e);
        }
    }
}

/// Show the panel window, positioned under the tray icon.
pub fn show_panel(app_handle: &AppHandle) {
    let Some(window) = app_handle.get_webview_window("main") else {
        log::error!("show_panel: main window not found");
        return;
    };
    let _ = window.show();
    let _ = window.set_focus();
    position_panel_from_tray(app_handle);
}

/// Hide the panel window.
pub fn hide_panel(app_handle: &AppHandle) {
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.hide();
    }
}

/// Check whether the panel window is currently visible.
pub fn is_panel_visible(app_handle: &AppHandle) -> bool {
    app_handle
        .get_webview_window("main")
        .and_then(|w| w.is_visible())
        .unwrap_or(false)
}

/// Toggle panel visibility. If visible, hide it. If hidden, show it.
/// Used by global shortcut handler.
pub fn toggle_panel(app_handle: &AppHandle) {
    if is_panel_visible(app_handle) {
        log::debug!("toggle_panel: hiding panel");
        hide_panel(app_handle);
    } else {
        log::debug!("toggle_panel: showing panel");
        show_panel(app_handle);
    }
}

/// Initialize the panel window for Linux/Windows.
/// Configures always-on-top and focus-loss auto-hide behavior.
pub fn init(app_handle: &tauri::AppHandle) -> tauri::Result<()> {
    let window = app_handle.get_webview_window("main").ok_or_else(|| {
        tauri::Error::from(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "main window not found",
        ))
    })?;

    // Keep the panel above other windows (replaces NSPanel floating behavior)
    let _ = window.set_always_on_top(true);

    // Auto-hide when the window loses focus
    let handle = app_handle.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::Focused(false) = event {
            if let Some(w) = handle.get_webview_window("main") {
                let _ = w.hide();
            }
        }
    });

    Ok(())
}

/// Position the panel window relative to the tray icon.
/// Cross-platform positioning using logical coordinates.
pub fn position_panel_at_tray_icon(
    app_handle: &tauri::AppHandle,
    icon_position: Position,
    icon_size: Size,
) {
    let Some(window) = app_handle.get_webview_window("main") else {
        return;
    };

    let (icon_phys_x, icon_phys_y) = match &icon_position {
        Position::Physical(pos) => (pos.x as f64, pos.y as f64),
        Position::Logical(pos) => (pos.x, pos.y),
    };
    let (icon_phys_w, icon_phys_h) = match &icon_size {
        Size::Physical(s) => (s.width as f64, s.height as f64),
        Size::Logical(s) => (s.width, s.height),
    };

    let monitors = match window.available_monitors() {
        Ok(m) => m,
        Err(e) => {
            log::warn!("Failed to get available monitors: {}", e);
            return;
        }
    };

    let icon_center_x = icon_phys_x + (icon_phys_w / 2.0);
    let icon_center_y = icon_phys_y + (icon_phys_h / 2.0);

    let found_monitor = monitors.iter().find(|monitor| {
        let origin = monitor.position();
        let size = monitor.size();
        monitor_contains_physical_point(
            origin.x as f64,
            origin.y as f64,
            size.width as f64,
            size.height as f64,
            icon_center_x,
            icon_center_y,
        )
    });

    let monitor = match found_monitor {
        Some(m) => m.clone(),
        None => {
            log::warn!(
                "No monitor found for tray rect center at ({:.0}, {:.0}), using primary",
                icon_center_x,
                icon_center_y
            );
            match window.primary_monitor() {
                Ok(Some(m)) => m,
                _ => return,
            }
        }
    };

    let target_scale = monitor.scale_factor();
    let mon_phys_x = monitor.position().x as f64;
    let mon_phys_y = monitor.position().y as f64;

    let icon_logical_x = mon_phys_x / target_scale + (icon_phys_x - mon_phys_x) / target_scale;
    let icon_logical_y = mon_phys_y / target_scale + (icon_phys_y - mon_phys_y) / target_scale;
    let icon_logical_w = icon_phys_w / target_scale;
    let icon_logical_h = icon_phys_h / target_scale;

    // Read panel width from the window, converted to logical points.
    let panel_width = match (window.outer_size(), window.scale_factor()) {
        (Ok(s), Ok(win_scale)) => s.width as f64 / win_scale,
        _ => {
            let conf: serde_json::Value =
                serde_json::from_str(include_str!("../../tauri.conf.json"))
                    .expect("tauri.conf.json must be valid JSON");
            conf["app"]["windows"][0]["width"]
                .as_f64()
                .expect("width must be set in tauri.conf.json")
        }
    };

    let icon_center_x = icon_logical_x + (icon_logical_w / 2.0);
    let panel_x = icon_center_x - (panel_width / 2.0);
    let nudge_up: f64 = 6.0;

    // Position the panel near the tray icon.
    // If the icon is in the top third of the monitor → position below (like macOS menu bar).
    // Otherwise → position above (typical Windows/Linux bottom taskbar).
    let monitor_logical_h = monitor.size().height as f64 / target_scale;
    let panel_h = panel_height(&window).unwrap_or(500.0);

    let panel_y = if icon_logical_y < monitor_logical_h / 3.0 {
        // Top-mounted tray: position below the icon
        (icon_logical_y + icon_logical_h - nudge_up).max(mon_phys_y / target_scale)
    } else {
        // Bottom-mounted tray: position above the icon
        (icon_logical_y - panel_h - nudge_up).max(mon_phys_y / target_scale)
    };

    if let Ok(outer_size) = window.outer_size() {
        let logical_w = outer_size.width as f64 / target_scale;
        let logical_h = outer_size.height as f64 / target_scale;
        let _ = window.set_position(Position::Logical((panel_x as f64, panel_y as f64).into()));
        let _ = window.set_size(Size::Logical((logical_w, logical_h).into()));
    } else {
        let _ = window.set_position(Position::Logical((panel_x as f64, panel_y as f64).into()));
    }
}

fn panel_height(window: &tauri::WebviewWindow) -> Option<f64> {
    window
        .outer_size()
        .ok()
        .and_then(|s| window.scale_factor().ok().map(|sf| s.height as f64 / sf))
}
