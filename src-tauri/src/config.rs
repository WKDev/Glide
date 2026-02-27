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
    pub scroll_opacity: bool,
    pub middleclick_topmost: bool,
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
            scroll_opacity: true,
            middleclick_topmost: true,
        }
    }
}
