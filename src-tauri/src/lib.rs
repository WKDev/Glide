mod commands;
mod config;
mod hook;
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

pub fn run() {
    env_logger::init();
    log::info!("wkgrip starting");
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![]),
        ))
        .setup(|app| {
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
            Some(val) => serde_json::from_value(val.clone()).unwrap_or_default(),
            None => {
                let default = AppConfig::default();
                if let Ok(val) = serde_json::to_value(&default) {
                    store.set("config", val);
                    let _ = store.save();
                }
                default
            }
        },
        Err(_) => AppConfig::default(),
    }
}

fn build_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let settings_i = MenuItemBuilder::with_id("settings", "Settings").build(app)?;
    let quit_i = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
    let menu = MenuBuilder::new(app)
        .items(&[&settings_i, &quit_i])
        .build()?;

    TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .tooltip("wkgrip")
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "settings" => {
                show_main_window(app);
            }
            "quit" => {
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
                show_main_window(&app);
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
