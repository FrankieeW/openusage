mod analytics;
#[cfg(target_os = "macos")]
mod app_nap;
mod config;
mod env_config;
mod hub;
mod local_http_api;
mod log_path;
mod panel;
mod plugin_engine;
mod plugin_metadata;
mod probe_command;
mod shortcut_support;
mod tray;
#[cfg(target_os = "macos")]
mod webkit_config;

use std::path::PathBuf;
use std::sync::Mutex;

use tauri_plugin_log::{Target, TargetKind};

pub use plugin_metadata::{ManifestLineDto, PluginLinkDto, PluginMeta, plugins_to_meta};
pub use probe_command::{ProbeBatchComplete, ProbeBatchStarted, ProbeResult};

pub struct AppState {
    pub plugins: Vec<plugin_engine::manifest::LoadedPlugin>,
    pub app_data_dir: PathBuf,
    pub app_version: String,
    pub hub_dir: PathBuf,
    pub hub_registry: hub::registry::RegistryFile,
    pub plugins_dir: PathBuf,
}

#[tauri::command]
fn init_panel(app_handle: tauri::AppHandle) {
    panel::init(&app_handle).expect("Failed to initialize panel");
}

#[tauri::command]
fn hide_panel(app_handle: tauri::AppHandle) {
    use tauri_nspanel::ManagerExt;
    if let Ok(panel) = app_handle.get_webview_panel("main") {
        panel.hide();
    }
}

#[tauri::command]
fn load_log_level(app_handle: tauri::AppHandle) -> String {
    tray::get_stored_log_level_value(&app_handle).to_string()
}

#[tauri::command]
fn save_log_level(app_handle: tauri::AppHandle, level: String) -> Result<(), String> {
    let Some(parsed) = tray::parse_log_level(&level) else {
        log::warn!("Rejected invalid debug level from frontend: {}", level);
        return Err("Invalid debug level".to_string());
    };
    tray::save_log_level(&app_handle, parsed).map_err(|error| {
        log::error!("Failed to save debug level: {}", error);
        "Failed to save debug level".to_string()
    })
}

#[tauri::command]
fn open_devtools(#[allow(unused)] app_handle: tauri::AppHandle) {
    #[cfg(debug_assertions)]
    {
        use tauri::Manager;
        if let Some(window) = app_handle.get_webview_window("main") {
            window.open_devtools();
        }
    }
}

#[tauri::command]
fn get_log_path(app_handle: tauri::AppHandle) -> Result<String, String> {
    log_path::for_app(&app_handle).map(|path| path.to_string_lossy().to_string())
}

#[tauri::command]
fn copy_log_path(app_handle: tauri::AppHandle) -> Result<(), String> {
    log_path::copy_to_clipboard(&app_handle).map_err(|error| {
        log::error!("Failed to copy log path: {}", error);
        "Failed to copy log path".to_string()
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    let _guard = runtime.enter();

    tauri::Builder::default()
        .plugin(tauri_plugin_aptabase::Builder::new("A-US-6435241436").build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_nspanel::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::LogDir { file_name: None }),
                ])
                .max_file_size(10_000_000) // 10 MB
                .level(log::LevelFilter::Trace) // Allow all levels; runtime filter via tray or Settings
                .level_for("hyper", log::LevelFilter::Warn)
                .level_for("reqwest", log::LevelFilter::Warn)
                .level_for("tao", log::LevelFilter::Info)
                .level_for("tauri_plugin_updater", log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            init_panel,
            hide_panel,
            open_devtools,
            env_config::set_allow_all_env,
            load_log_level,
            save_log_level,
            env_config::set_env_overrides,
            probe_command::start_probe_batch,
            plugin_metadata::list_plugins,
            get_log_path,
            copy_log_path,
            shortcut_support::update_global_shortcut,
            hub::hub_list_sources,
            hub::hub_add_source,
            hub::hub_update_source,
            hub::hub_remove_source,
            hub::hub_browse_source,
            hub::hub_install,
            hub::hub_switch_source,
            hub::hub_uninstall,
            hub::hub_refresh_source,
            hub::hub_reload_plugins,
            hub::hub_check_updates,
            hub::hub_list_local_plugins
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            #[cfg(target_os = "macos")]
            {
                app_nap::disable_app_nap();
                webkit_config::disable_webview_suspension(app.handle());
            }

            use tauri::Manager;

            let version = app.package_info().version.to_string();
            log::info!("OpenUsage v{} starting", version);

            // Load config early (lazy init via OnceLock, zero-cost after)
            let _proxy = config::get_resolved_proxy();

            env_config::apply_unsafe_env_setting(app.handle());
            env_config::apply_env_overrides(app.handle());

            analytics::track_daily_active_if_needed(app.handle());
            #[cfg(desktop)]
            analytics::spawn_daily_active_rollover_tracker(app.handle().clone());

            let app_data_dir = app.path().app_data_dir().expect("no app data dir");
            let resource_dir = app.path().resource_dir().expect("no resource dir");
            let app_data_dir_tail = app_data_dir
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("unknown");
            let redacted_app_data_dir =
                plugin_engine::host_api::redact_log_message(&app_data_dir.display().to_string());
            log::debug!(
                "app_data_dir: tail={}, path={}",
                app_data_dir_tail,
                redacted_app_data_dir
            );

            let (plugins_dir, plugins) =
                plugin_engine::initialize_plugins(&app_data_dir, &resource_dir);
            let known_plugin_ids: Vec<String> =
                plugins.iter().map(|p| p.manifest.id.clone()).collect();

            let hub_dir = hub::hub_dir(&app_data_dir);
            let mut hub_registry = hub::registry::read(&hub_dir).unwrap_or_else(|err| {
                log::warn!("hub registry load failed: {}, starting from default", err);
                hub::registry::default_registry()
            });
            // Persist default to disk so sources.json exists before the first
            // Browse (which triggers auto-clone into cache/).
            if let Err(err) = hub::registry::write(&hub_dir, &hub_registry) {
                log::warn!("hub registry write failed (non-fatal): {}", err);
                // Reset to in-memory default so the UI still shows the upstream source.
                hub_registry = hub::registry::default_registry();
            }

            app.manage(Mutex::new(AppState {
                plugins,
                app_data_dir: app_data_dir.clone(),
                app_version: app.package_info().version.to_string(),
                hub_dir,
                hub_registry,
                plugins_dir,
            }));

            local_http_api::init(&app_data_dir, known_plugin_ids);
            local_http_api::start_server();

            tray::create(app.handle())?;

            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;

            // Register global shortcut from stored settings
            #[cfg(desktop)]
            shortcut_support::register_initial_global_shortcut(app.handle());

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_, event| match event {
            tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit => {
                local_http_api::flush_cache();
            }
            _ => {}
        });
}
