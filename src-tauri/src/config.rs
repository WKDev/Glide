use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModifierKey {
    Alt,
    Ctrl,
    Shift,
    Win,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FilterMode {
    Whitelist,
    Blacklist,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResizeMode {
    /// Direction depends on cursor position relative to window centre.
    Quadrant,
    /// Cursor right = grow right, cursor down = grow down, always.
    Absolute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GestureTrigger {
    /// Hold the modifiers and drag with a mouse button.
    Click,
    /// Hold the modifiers and move the mouse past the drag threshold.
    Pan,
}

/// A gesture fires only on this exact set of modifier keys.
pub const MAX_COMBO_KEYS: usize = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub enabled: bool,
    #[serde(default = "default_move_enabled")]
    pub move_enabled: bool,
    #[serde(default = "default_resize_enabled")]
    pub resize_enabled: bool,
    #[serde(default = "default_move_modifiers")]
    pub move_modifiers: Vec<ModifierKey>,
    #[serde(default = "default_trigger")]
    pub move_trigger: GestureTrigger,
    #[serde(default = "default_resize_modifiers")]
    pub resize_modifiers: Vec<ModifierKey>,
    #[serde(default = "default_trigger")]
    pub resize_trigger: GestureTrigger,
    #[serde(default = "default_resize_mode")]
    pub resize_mode: ResizeMode,
    pub filter_mode: FilterMode,
    pub filter_list: Vec<String>,
    pub autostart: bool,
    pub allow_nonforeground: bool,
    pub raise_on_grab: bool,
    pub snap_enabled: bool,
    pub snap_threshold: i32,
    #[serde(default = "default_snap_native")]
    pub snap_native: bool,
    pub scroll_opacity: bool,
    #[serde(default = "default_scroll_opacity_modifier")]
    pub scroll_opacity_modifier: ModifierKey,
    #[serde(default = "default_drag_threshold")]
    pub drag_threshold: i32,
}

fn default_move_enabled() -> bool {
    true
}

fn default_resize_enabled() -> bool {
    true
}

fn default_move_modifiers() -> Vec<ModifierKey> {
    vec![ModifierKey::Alt]
}

fn default_resize_modifiers() -> Vec<ModifierKey> {
    vec![ModifierKey::Alt, ModifierKey::Shift]
}

fn default_trigger() -> GestureTrigger {
    GestureTrigger::Click
}

fn default_resize_mode() -> ResizeMode {
    ResizeMode::Quadrant
}

fn default_drag_threshold() -> i32 {
    10
}

fn default_scroll_opacity_modifier() -> ModifierKey {
    ModifierKey::Alt
}

fn default_snap_native() -> bool {
    true
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            move_enabled: true,
            resize_enabled: true,
            move_modifiers: default_move_modifiers(),
            move_trigger: GestureTrigger::Click,
            resize_modifiers: default_resize_modifiers(),
            resize_trigger: GestureTrigger::Click,
            resize_mode: ResizeMode::Quadrant,
            filter_mode: FilterMode::Blacklist,
            filter_list: Vec::new(),
            autostart: false,
            allow_nonforeground: true,
            raise_on_grab: false,
            snap_enabled: true,
            snap_threshold: 20,
            snap_native: true,
            scroll_opacity: true,
            scroll_opacity_modifier: ModifierKey::Alt,
            drag_threshold: 10,
        }
    }
}

impl AppConfig {
    /// Load a stored config, converting the single-key fields written by
    /// Glide 0.2 (`move_modifier`, `resize_modifier_1`/`_2`) into key sets.
    pub fn from_stored(mut value: serde_json::Value) -> Result<Self, serde_json::Error> {
        if let Some(object) = value.as_object_mut() {
            if !object.contains_key("move_modifiers") {
                if let Some(key) = object.remove("move_modifier") {
                    object.insert("move_modifiers".into(), serde_json::Value::Array(vec![key]));
                }
            }
            if !object.contains_key("resize_modifiers") {
                let keys: Vec<_> = ["resize_modifier_1", "resize_modifier_2"]
                    .iter()
                    .filter_map(|name| object.remove(*name))
                    .collect();
                if !keys.is_empty() {
                    object.insert("resize_modifiers".into(), serde_json::Value::Array(keys));
                }
            }
        }
        let mut config: Self = serde_json::from_value(value)?;
        config.move_modifiers = normalize_combo(&config.move_modifiers, default_move_modifiers);
        config.resize_modifiers =
            normalize_combo(&config.resize_modifiers, default_resize_modifiers);
        Ok(config)
    }
}

/// Drop duplicate keys; fall back to the default when the set is empty or too large.
fn normalize_combo(keys: &[ModifierKey], fallback: fn() -> Vec<ModifierKey>) -> Vec<ModifierKey> {
    let mut unique = Vec::with_capacity(keys.len());
    for key in keys {
        if !unique.contains(key) {
            unique.push(*key);
        }
    }
    if unique.is_empty() || unique.len() > MAX_COMBO_KEYS {
        fallback()
    } else {
        unique
    }
}

/// A valid combination has one to three distinct keys.
pub fn is_valid_combo(keys: &[ModifierKey]) -> bool {
    (1..=MAX_COMBO_KEYS).contains(&keys.len())
        && keys
            .iter()
            .enumerate()
            .all(|(i, key)| !keys[..i].contains(key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.enabled, true);
        assert_eq!(config.move_enabled, true);
        assert_eq!(config.resize_enabled, true);
        assert_eq!(config.move_modifiers, vec![ModifierKey::Alt]);
        assert_eq!(config.move_trigger, GestureTrigger::Click);
        assert_eq!(
            config.resize_modifiers,
            vec![ModifierKey::Alt, ModifierKey::Shift]
        );
        assert_eq!(config.resize_trigger, GestureTrigger::Click);
        assert_eq!(config.resize_mode, ResizeMode::Quadrant);
        assert_eq!(config.filter_mode, FilterMode::Blacklist);
        assert_eq!(config.filter_list, Vec::<String>::new());
        assert_eq!(config.autostart, false);
        assert_eq!(config.allow_nonforeground, true);
        assert_eq!(config.raise_on_grab, false);
        assert_eq!(config.snap_enabled, true);
        assert_eq!(config.snap_threshold, 20);
        assert_eq!(config.scroll_opacity, true);
        assert_eq!(config.drag_threshold, 10);
        assert_eq!(config.snap_native, true);
        assert_eq!(config.scroll_opacity_modifier, ModifierKey::Alt);
    }

    #[test]
    fn test_serde_round_trip_default() {
        let original = AppConfig::default();
        let json = serde_json::to_value(&original).expect("serialize failed");
        let deserialized: AppConfig = serde_json::from_value(json).expect("deserialize failed");

        assert_eq!(deserialized.enabled, original.enabled);
        assert_eq!(deserialized.move_enabled, original.move_enabled);
        assert_eq!(deserialized.resize_enabled, original.resize_enabled);
        assert_eq!(deserialized.move_modifiers, original.move_modifiers);
        assert_eq!(deserialized.move_trigger, original.move_trigger);
        assert_eq!(deserialized.resize_modifiers, original.resize_modifiers);
        assert_eq!(deserialized.resize_trigger, original.resize_trigger);
        assert_eq!(deserialized.resize_mode, original.resize_mode);
        assert_eq!(deserialized.filter_mode, original.filter_mode);
        assert_eq!(deserialized.filter_list, original.filter_list);
        assert_eq!(deserialized.autostart, original.autostart);
        assert_eq!(
            deserialized.allow_nonforeground,
            original.allow_nonforeground
        );
        assert_eq!(deserialized.raise_on_grab, original.raise_on_grab);
        assert_eq!(deserialized.snap_enabled, original.snap_enabled);
        assert_eq!(deserialized.snap_threshold, original.snap_threshold);
        assert_eq!(deserialized.scroll_opacity, original.scroll_opacity);
        assert_eq!(deserialized.drag_threshold, original.drag_threshold);
        assert_eq!(deserialized.snap_native, original.snap_native);
        assert_eq!(
            deserialized.scroll_opacity_modifier,
            original.scroll_opacity_modifier
        );
    }

    #[test]
    fn test_serde_round_trip_all_non_default() {
        let original = AppConfig {
            enabled: false,
            move_enabled: false,
            resize_enabled: false,
            move_modifiers: vec![ModifierKey::Ctrl, ModifierKey::Win, ModifierKey::Shift],
            move_trigger: GestureTrigger::Pan,
            resize_modifiers: vec![ModifierKey::Win, ModifierKey::Ctrl],
            resize_trigger: GestureTrigger::Pan,
            resize_mode: ResizeMode::Absolute,
            filter_mode: FilterMode::Whitelist,
            filter_list: vec!["notepad.exe".to_string(), "calc.exe".to_string()],
            autostart: true,
            allow_nonforeground: false,
            raise_on_grab: true,
            snap_enabled: false,
            snap_threshold: 50,
            scroll_opacity: false,
            drag_threshold: 30,
            snap_native: false,
            scroll_opacity_modifier: ModifierKey::Ctrl,
        };
        let json = serde_json::to_value(&original).expect("serialize failed");
        let deserialized: AppConfig = serde_json::from_value(json).expect("deserialize failed");

        assert_eq!(deserialized.enabled, original.enabled);
        assert_eq!(deserialized.move_enabled, original.move_enabled);
        assert_eq!(deserialized.resize_enabled, original.resize_enabled);
        assert_eq!(deserialized.move_modifiers, original.move_modifiers);
        assert_eq!(deserialized.resize_modifiers, original.resize_modifiers);
        assert_eq!(deserialized.resize_mode, original.resize_mode);
        assert_eq!(deserialized.filter_mode, original.filter_mode);
        assert_eq!(deserialized.filter_list, original.filter_list);
        assert_eq!(deserialized.autostart, original.autostart);
        assert_eq!(
            deserialized.allow_nonforeground,
            original.allow_nonforeground
        );
        assert_eq!(deserialized.raise_on_grab, original.raise_on_grab);
        assert_eq!(deserialized.snap_enabled, original.snap_enabled);
        assert_eq!(deserialized.snap_threshold, original.snap_threshold);
        assert_eq!(deserialized.scroll_opacity, original.scroll_opacity);
        assert_eq!(deserialized.drag_threshold, original.drag_threshold);
        assert_eq!(deserialized.snap_native, original.snap_native);
        assert_eq!(
            deserialized.scroll_opacity_modifier,
            original.scroll_opacity_modifier
        );
    }

    #[test]
    fn test_modifier_key_serialization() {
        assert_eq!(serde_json::to_value(ModifierKey::Alt).unwrap(), "alt");
        assert_eq!(serde_json::to_value(ModifierKey::Ctrl).unwrap(), "ctrl");
        assert_eq!(serde_json::to_value(ModifierKey::Shift).unwrap(), "shift");
        assert_eq!(serde_json::to_value(ModifierKey::Win).unwrap(), "win");
    }

    #[test]
    fn test_modifier_key_deserialization() {
        let alt: ModifierKey = serde_json::from_value(serde_json::json!("alt")).unwrap();
        assert_eq!(alt, ModifierKey::Alt);

        let ctrl: ModifierKey = serde_json::from_value(serde_json::json!("ctrl")).unwrap();
        assert_eq!(ctrl, ModifierKey::Ctrl);

        let shift: ModifierKey = serde_json::from_value(serde_json::json!("shift")).unwrap();
        assert_eq!(shift, ModifierKey::Shift);

        let win: ModifierKey = serde_json::from_value(serde_json::json!("win")).unwrap();
        assert_eq!(win, ModifierKey::Win);
    }

    #[test]
    fn test_filter_mode_serialization() {
        assert_eq!(
            serde_json::to_value(FilterMode::Whitelist).unwrap(),
            "whitelist"
        );
        assert_eq!(
            serde_json::to_value(FilterMode::Blacklist).unwrap(),
            "blacklist"
        );
    }

    #[test]
    fn test_filter_mode_deserialization() {
        let whitelist: FilterMode = serde_json::from_value(serde_json::json!("whitelist")).unwrap();
        assert_eq!(whitelist, FilterMode::Whitelist);

        let blacklist: FilterMode = serde_json::from_value(serde_json::json!("blacklist")).unwrap();
        assert_eq!(blacklist, FilterMode::Blacklist);
    }

    #[test]
    fn test_resize_mode_serialization() {
        assert_eq!(
            serde_json::to_value(ResizeMode::Quadrant).unwrap(),
            "quadrant"
        );
        assert_eq!(
            serde_json::to_value(ResizeMode::Absolute).unwrap(),
            "absolute"
        );
    }

    #[test]
    fn test_resize_mode_deserialization() {
        let quadrant: ResizeMode = serde_json::from_value(serde_json::json!("quadrant")).unwrap();
        assert_eq!(quadrant, ResizeMode::Quadrant);

        let absolute: ResizeMode = serde_json::from_value(serde_json::json!("absolute")).unwrap();
        assert_eq!(absolute, ResizeMode::Absolute);
    }

    #[test]
    fn test_invalid_modifier_key_deserialization() {
        let result: Result<ModifierKey, _> = serde_json::from_value(serde_json::json!("invalid"));
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_filter_mode_deserialization() {
        let result: Result<FilterMode, _> = serde_json::from_value(serde_json::json!("invalid"));
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_resize_mode_deserialization() {
        let result: Result<ResizeMode, _> = serde_json::from_value(serde_json::json!("invalid"));
        assert!(result.is_err());
    }

    #[test]
    fn test_extra_fields_deserialization() {
        let json = serde_json::json!({
            "enabled": true,
            "move_modifier": "alt",
            "resize_modifier_1": "alt",
            "resize_modifier_2": "shift",
            "filter_mode": "blacklist",
            "filter_list": [],
            "autostart": false,
            "allow_nonforeground": true,
            "raise_on_grab": false,
            "snap_enabled": true,
            "snap_threshold": 20,
            "scroll_opacity": true,
            "middleclick_topmost": true,
            "drag_threshold": 10,
            "snap_native": true,
            "unknown_field": "should be ignored"
        });
        let result: Result<AppConfig, _> = serde_json::from_value(json);
        assert!(result.is_ok());
    }

    #[test]
    fn test_legacy_config_without_new_fields() {
        // Ensure old configs (without move_enabled, resize_enabled, resize_mode) still load.
        let json = serde_json::json!({
            "enabled": true,
            "move_modifier": "alt",
            "resize_modifier_1": "alt",
            "resize_modifier_2": "shift",
            "filter_mode": "blacklist",
            "filter_list": [],
            "autostart": false,
            "allow_nonforeground": true,
            "raise_on_grab": false,
            "snap_enabled": true,
            "snap_threshold": 20,
            "scroll_opacity": true,
            "drag_threshold": 10,
            "snap_native": true
        });
        let config = AppConfig::from_stored(json).expect("legacy config should load");
        assert_eq!(config.move_enabled, true);
        assert_eq!(config.resize_enabled, true);
        assert_eq!(config.resize_mode, ResizeMode::Quadrant);
    }

    #[test]
    fn test_filter_list_with_multiple_entries() {
        let config = AppConfig {
            enabled: true,
            move_enabled: true,
            resize_enabled: true,
            move_modifiers: vec![ModifierKey::Alt],
            move_trigger: GestureTrigger::Click,
            resize_modifiers: vec![ModifierKey::Alt, ModifierKey::Shift],
            resize_trigger: GestureTrigger::Click,
            resize_mode: ResizeMode::Quadrant,
            filter_mode: FilterMode::Whitelist,
            filter_list: vec![
                "app1.exe".to_string(),
                "app2.exe".to_string(),
                "app3.exe".to_string(),
            ],
            autostart: false,
            allow_nonforeground: true,
            raise_on_grab: false,
            snap_enabled: true,
            snap_threshold: 20,
            scroll_opacity: true,
            drag_threshold: 10,
            snap_native: true,
            scroll_opacity_modifier: ModifierKey::Alt,
        };

        let json = serde_json::to_value(&config).expect("serialize failed");
        let deserialized: AppConfig = serde_json::from_value(json).expect("deserialize failed");

        assert_eq!(deserialized.filter_list.len(), 3);
        assert_eq!(deserialized.filter_list[0], "app1.exe");
        assert_eq!(deserialized.filter_list[1], "app2.exe");
        assert_eq!(deserialized.filter_list[2], "app3.exe");
    }

    #[test]
    fn legacy_single_keys_become_combinations() {
        let json = serde_json::json!({
            "enabled": true,
            "move_modifier": "ctrl",
            "resize_modifier_1": "win",
            "resize_modifier_2": "win",
            "filter_mode": "blacklist",
            "filter_list": [],
            "autostart": false,
            "allow_nonforeground": true,
            "raise_on_grab": false,
            "snap_enabled": true,
            "snap_threshold": 20,
            "scroll_opacity": true
        });
        let config = AppConfig::from_stored(json).expect("legacy config should load");
        assert_eq!(config.move_modifiers, vec![ModifierKey::Ctrl]);
        assert_eq!(config.resize_modifiers, vec![ModifierKey::Win]);
        assert_eq!(config.resize_trigger, GestureTrigger::Click);
    }

    #[test]
    fn combo_validation_limits_size_and_duplicates() {
        use ModifierKey::*;
        assert!(is_valid_combo(&[Alt]));
        assert!(is_valid_combo(&[Ctrl, Alt, Shift]));
        assert!(!is_valid_combo(&[]));
        assert!(!is_valid_combo(&[Alt, Alt]));
        assert!(!is_valid_combo(&[Ctrl, Alt, Shift, Win]));
    }

    #[test]
    fn test_snap_threshold_edge_cases() {
        let config_zero = AppConfig {
            snap_threshold: 0,
            ..Default::default()
        };
        let json = serde_json::to_value(&config_zero).expect("serialize failed");
        let deserialized: AppConfig = serde_json::from_value(json).expect("deserialize failed");
        assert_eq!(deserialized.snap_threshold, 0);

        let config_large = AppConfig {
            snap_threshold: 1000,
            ..Default::default()
        };
        let json = serde_json::to_value(&config_large).expect("serialize failed");
        let deserialized: AppConfig = serde_json::from_value(json).expect("deserialize failed");
        assert_eq!(deserialized.snap_threshold, 1000);
    }
}
