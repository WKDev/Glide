mod commands;
mod config;
mod hook;
mod overlay;
mod snap;
mod window_manager;

use std::sync::Arc;

use commands::AppState;
use config::AppConfig;
use parking_lot::Mutex;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_store::StoreExt;
#[cfg(target_os = "windows")]
use window_vibrancy::apply_mica;

pub fn run() {
    // Allow runtime log level override: GLIDE_LOG=debug ./glide
    // Valid values: error, warn, info, debug, trace (case-insensitive).
    // Defaults to Info when unset or unrecognised.
    let log_level: log::LevelFilter = std::env::var("GLIDE_LOG")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(log::LevelFilter::Info);
    let _sentry_guard = sentry::init((
        option_env!("SENTRY_DSN").unwrap_or(""),
        sentry::ClientOptions {
            release: sentry::release_name!(),
            // Capture 100% of errors; 0% of transactions (no perf overhead).
            traces_sample_rate: 0.0,
            ..Default::default()
        },
    ));
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![]),
        ))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                        file_name: None,
                    }),
                ])
                .level(log_level)
                .max_file_size(50_000)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepOne)
                .build(),
        )
        .setup(|app| {
            log::info!("Glide starting");

            #[cfg(target_os = "windows")]
            {
                if let Some(window) = app.get_webview_window("main") {
                    if let Err(error) = apply_mica(&window, None) {
                        log::warn!("failed to apply mica: {}", error);
                    }
                }
            }

            // Load config from store or use default
            let config = load_config(app);
            let config = Arc::new(Mutex::new(config));

            // Register managed state
            app.manage(AppState {
                config: config.clone(),
            });

            // Build system tray
            build_tray(app)?;

            // Spawn hook thread
            let hook_config = config.clone();
            let enabled = config.lock().enabled;
            hook::set_enabled(enabled);
            let hook_tid = hook::start_hook_thread(hook_config);
            log::info!("setup complete — hook_tid={}", hook_tid);

            Ok(())
        })
        .on_window_event(|window, event| {
            // Hide to tray instead of closing
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::set_config,
            commands::get_running_processes,
            commands::set_hook_enabled,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn load_config(app: &tauri::App) -> AppConfig {
    match app.store("config.json") {
        Ok(store) => match store.get("config") {
            Some(val) => match serde_json::from_value(val.clone()) {
                Ok(cfg) => cfg,
                Err(e) => {
                    log::warn!("config deserialize failed, using defaults: {}", e);
                    AppConfig::default()
                }
            },
            None => {
                let default = AppConfig::default();
                if let Ok(val) = serde_json::to_value(&default) {
                    store.set("config", val);
                    let _ = store.save();
                }
                default
            }
        },
        Err(e) => {
            log::warn!("failed to open config store, using defaults: {}", e);
            AppConfig::default()
        }
    }
}

fn build_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let settings_i = MenuItemBuilder::with_id("settings", "Settings").build(app)?;
    let quit_i = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
    let menu = MenuBuilder::new(app)
        .items(&[&settings_i, &quit_i])
        .build()?;

    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or("default window icon not found")?;

    TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .tooltip("Glide")
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "settings" => {
                show_main_window(app);
            }
            "quit" => {
                // Signal the hook thread to run its cleanup sequence
                // (UnhookWindowsHookEx, worker shutdown, overlay destroy).
                // Give it up to 500 ms before forcing exit.
                hook::shutdown();
                std::thread::sleep(std::time::Duration::from_millis(500));
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| match event {
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Down,
                ..
            }
            | TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            }
            | TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } => {
                let app = tray.app_handle();
                show_main_window(app);
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}
