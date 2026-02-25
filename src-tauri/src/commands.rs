use std::sync::Arc;

use parking_lot::Mutex;
use tauri::State;
use tauri_plugin_store::StoreExt;

use crate::config::AppConfig;
use crate::hook;
use crate::window_manager;

pub struct AppState {
    pub config: Arc<Mutex<AppConfig>>,
}

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> AppConfig {
    state.config.lock().clone()
}

#[tauri::command]
pub fn set_config(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    config: AppConfig,
) -> Result<(), String> {
    // Update shared state
    *state.config.lock() = config.clone();

    // Update hook enabled state
    hook::set_enabled(config.enabled);

    // Propagate config to hook thread
    hook::update_config(state.config.clone());

    // Persist to store
    let store = app.store("config.json").map_err(|e| e.to_string())?;
    store.set(
        "config",
        serde_json::to_value(&config).map_err(|e| e.to_string())?,
    );
    store.save().map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn get_running_processes() -> Vec<String> {
    window_manager::get_running_process_names()
}

#[tauri::command]
pub fn set_hook_enabled(state: State<'_, AppState>, enabled: bool) {
    state.config.lock().enabled = enabled;
    hook::set_enabled(enabled);
}
