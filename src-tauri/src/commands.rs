use std::sync::Arc;

use parking_lot::Mutex;
use tauri::State;
use tauri_plugin_store::StoreExt;

use crate::config::{self, AppConfig};
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
    mut config: AppConfig,
) -> Result<(), String> {
    validate_config(&config)?;
    let mut current = state.config.lock();
    // The tray owns this flag. A settings window can have an older snapshot.
    config.enabled = current.enabled;
    persist_config(&app, &config)?;
    *current = config;
    Ok(())
}

fn validate_config(config: &AppConfig) -> Result<(), String> {
    for (name, keys) in [
        ("move_modifiers", &config.move_modifiers),
        ("resize_modifiers", &config.resize_modifiers),
    ] {
        if !config::is_valid_combo(keys) {
            return Err(format!("{name} must have one to three distinct keys"));
        }
    }
    for (name, value) in [
        ("snap_threshold", config.snap_threshold),
        ("drag_threshold", config.drag_threshold),
    ] {
        if value < 0 {
            return Err(format!("{name} must be non-negative"));
        }
        if value > 500 {
            return Err(format!("{name} must not exceed 500"));
        }
    }
    Ok(())
}

fn persist_config(app: &tauri::AppHandle, config: &AppConfig) -> Result<(), String> {
    let store = app.store("config.json").map_err(|e| e.to_string())?;
    let previous = store.get("config");
    store.set(
        "config",
        serde_json::to_value(config).map_err(|e| e.to_string())?,
    );
    if let Err(error) = store.save() {
        if let Some(previous) = previous {
            store.set("config", previous);
        } else {
            store.delete("config");
        }
        return Err(error.to_string());
    }
    Ok(())
}

#[tauri::command]
pub fn get_running_processes() -> Vec<String> {
    window_manager::get_running_process_names()
}

#[derive(serde::Serialize)]
pub struct AppStatus {
    hook_ready: bool,
    store_build: bool,
}

#[tauri::command]
pub fn get_app_status() -> AppStatus {
    AppStatus {
        hook_ready: hook::is_ready(),
        store_build: crate::STORE_BUILD,
    }
}

#[tauri::command]
pub async fn store_autostart(enabled: Option<bool>) -> Result<bool, String> {
    if !crate::STORE_BUILD {
        return Err("Windows startup tasks require the Store package.".into());
    }

    tauri::async_runtime::spawn_blocking(move || {
        use windows::core::HSTRING;
        use windows::ApplicationModel::{StartupTask, StartupTaskState};
        use windows::Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED};

        unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.map_err(|e| e.to_string())?;
        let result = (|| {
            let task = StartupTask::GetAsync(&HSTRING::from("GlideStartup"))?.get()?;
            let status = match enabled {
                Some(true) => task.RequestEnableAsync()?.get()?,
                Some(false) => {
                    task.Disable()?;
                    task.State()?
                }
                None => task.State()?,
            };
            Ok(matches!(
                status,
                StartupTaskState::Enabled | StartupTaskState::EnabledByPolicy
            ))
        })();
        unsafe { RoUninitialize() };
        result.map_err(|e: windows::core::Error| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

pub fn toggle_hook(app: &tauri::AppHandle, state: State<'_, AppState>) -> Result<bool, String> {
    let mut current = state.config.lock();
    let mut config = current.clone();
    config.enabled = !config.enabled;
    persist_config(app, &config)?;
    *current = config;
    hook::set_enabled(current.enabled);
    Ok(current.enabled)
}

#[cfg(test)]
mod tests {
    use crate::config::AppConfig;

    use super::validate_config;

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
