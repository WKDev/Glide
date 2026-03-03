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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub enabled: bool,
    pub move_modifier: ModifierKey,
    pub resize_modifier_1: ModifierKey,
    pub resize_modifier_2: ModifierKey,
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
    pub middleclick_topmost: bool,
    #[serde(default = "default_drag_threshold")]
    pub drag_threshold: i32,
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
            move_modifier: ModifierKey::Alt,
            resize_modifier_1: ModifierKey::Alt,
            resize_modifier_2: ModifierKey::Shift,
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
            middleclick_topmost: true,
            drag_threshold: 10,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.enabled, true);
        assert_eq!(config.move_modifier, ModifierKey::Alt);
        assert_eq!(config.resize_modifier_1, ModifierKey::Alt);
        assert_eq!(config.resize_modifier_2, ModifierKey::Shift);
        assert_eq!(config.filter_mode, FilterMode::Blacklist);
        assert_eq!(config.filter_list, Vec::<String>::new());
        assert_eq!(config.autostart, false);
        assert_eq!(config.allow_nonforeground, true);
        assert_eq!(config.raise_on_grab, false);
        assert_eq!(config.snap_enabled, true);
        assert_eq!(config.snap_threshold, 20);
        assert_eq!(config.scroll_opacity, true);
        assert_eq!(config.middleclick_topmost, true);
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
        assert_eq!(deserialized.move_modifier, original.move_modifier);
        assert_eq!(deserialized.resize_modifier_1, original.resize_modifier_1);
        assert_eq!(deserialized.resize_modifier_2, original.resize_modifier_2);
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
        assert_eq!(
            deserialized.middleclick_topmost,
            original.middleclick_topmost
        );
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
            move_modifier: ModifierKey::Ctrl,
            resize_modifier_1: ModifierKey::Win,
            resize_modifier_2: ModifierKey::Ctrl,
            filter_mode: FilterMode::Whitelist,
            filter_list: vec!["notepad.exe".to_string(), "calc.exe".to_string()],
            autostart: true,
            allow_nonforeground: false,
            raise_on_grab: true,
            snap_enabled: false,
            snap_threshold: 50,
            scroll_opacity: false,
            middleclick_topmost: false,
            drag_threshold: 30,
            snap_native: false,
            scroll_opacity_modifier: ModifierKey::Ctrl,
        };
        let json = serde_json::to_value(&original).expect("serialize failed");
        let deserialized: AppConfig = serde_json::from_value(json).expect("deserialize failed");

        assert_eq!(deserialized.enabled, original.enabled);
        assert_eq!(deserialized.move_modifier, original.move_modifier);
        assert_eq!(deserialized.resize_modifier_1, original.resize_modifier_1);
        assert_eq!(deserialized.resize_modifier_2, original.resize_modifier_2);
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
        assert_eq!(
            deserialized.middleclick_topmost,
            original.middleclick_topmost
        );
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
    fn test_filter_list_with_multiple_entries() {
        let config = AppConfig {
            enabled: true,
            move_modifier: ModifierKey::Alt,
            resize_modifier_1: ModifierKey::Alt,
            resize_modifier_2: ModifierKey::Shift,
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
            middleclick_topmost: true,
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
