export type ModifierKey = "alt" | "ctrl" | "shift" | "win";
export type FilterMode = "whitelist" | "blacklist";

export interface AppConfig {
  enabled: boolean;
  move_modifier: ModifierKey;
  resize_modifier_1: ModifierKey;
  resize_modifier_2: ModifierKey;
  filter_mode: FilterMode;
  filter_list: string[];
  autostart: boolean;
}

export const DEFAULT_CONFIG: AppConfig = {
  enabled: true,
  move_modifier: "alt",
  resize_modifier_1: "alt",
  resize_modifier_2: "shift",
  filter_mode: "blacklist",
  filter_list: [],
  autostart: false,
};

export const MODIFIER_OPTIONS: { value: ModifierKey; label: string }[] = [
  { value: "alt", label: "Alt" },
  { value: "ctrl", label: "Ctrl" },
  { value: "shift", label: "Shift" },
  { value: "win", label: "Win" },
];
