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
    // Server-side bounds validation to guard against out-of-range IPC values.
    if config.snap_threshold < 0 {
        return Err("snap_threshold must be non-negative".to_string());
    }
    if config.snap_threshold > 500 {
        return Err("snap_threshold must not exceed 500".to_string());
    }
    if config.drag_threshold < 0 {
        return Err("drag_threshold must be non-negative".to_string());
    }
    if config.drag_threshold > 500 {
        return Err("drag_threshold must not exceed 500".to_string());
    }

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
pub fn set_hook_enabled(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<(), String> {
    state.config.lock().enabled = enabled;
    hook::set_enabled(enabled);

    // Persist the enabled state so it survives restarts.
    let config = state.config.lock().clone();
    let store = app.store("config.json").map_err(|e| e.to_string())?;
    store.set(
        "config",
        serde_json::to_value(&config).map_err(|e| e.to_string())?,
    );
    store.save().map_err(|e| e.to_string())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::config::AppConfig;

    // Mirror the validation logic so it can be unit-tested without a live Tauri runtime.
    fn validate_config(config: &AppConfig) -> Result<(), String> {
        if config.snap_threshold < 0 {
            return Err("snap_threshold must be non-negative".to_string());
        }
        if config.snap_threshold > 500 {
            return Err("snap_threshold must not exceed 500".to_string());
        }
        if config.drag_threshold < 0 {
            return Err("drag_threshold must be non-negative".to_string());
        }
        if config.drag_threshold > 500 {
            return Err("drag_threshold must not exceed 500".to_string());
        }
        Ok(())
    }

    #[test]
    fn test_snap_threshold_lower_bound() {
        let config = AppConfig {
            snap_threshold: -1,
            ..AppConfig::default()
        };
        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn test_snap_threshold_upper_bound() {
        let config = AppConfig {
            snap_threshold: 501,
            ..AppConfig::default()
        };
        let err = validate_config(&config).unwrap_err();
        assert!(err.contains("must not exceed 500"));
    }

    #[test]
    fn test_snap_threshold_boundary_values() {
        assert!(validate_config(&AppConfig {
            snap_threshold: 0,
            ..AppConfig::default()
        })
        .is_ok());
        assert!(validate_config(&AppConfig {
            snap_threshold: 500,
            ..AppConfig::default()
        })
        .is_ok());
    }

    #[test]
    fn test_drag_threshold_lower_bound() {
        let config = AppConfig {
            drag_threshold: -1,
            ..AppConfig::default()
        };
        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn test_drag_threshold_upper_bound() {
        let config = AppConfig {
            drag_threshold: 501,
            ..AppConfig::default()
        };
        let err = validate_config(&config).unwrap_err();
        assert!(err.contains("must not exceed 500"));
    }

    #[test]
    fn test_drag_threshold_boundary_values() {
        assert!(validate_config(&AppConfig {
            drag_threshold: 0,
            ..AppConfig::default()
        })
        .is_ok());
        assert!(validate_config(&AppConfig {
            drag_threshold: 500,
            ..AppConfig::default()
        })
        .is_ok());
    }

    #[test]
    fn test_default_config_passes_validation() {
        assert!(validate_config(&AppConfig::default()).is_ok());
    }
}
