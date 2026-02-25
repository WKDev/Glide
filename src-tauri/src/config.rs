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
        }
    }
}
