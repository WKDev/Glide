export type ModifierKey = 'alt' | 'ctrl' | 'shift' | 'win';
export type FilterMode = 'whitelist' | 'blacklist';
export type ResizeMode = 'quadrant' | 'absolute';

export interface AppConfig {
  enabled: boolean;
  move_enabled: boolean;
  resize_enabled: boolean;
  move_modifier: ModifierKey;
  resize_modifier_1: ModifierKey;
  resize_modifier_2: ModifierKey;
  resize_mode: ResizeMode;
  filter_mode: FilterMode;
  filter_list: string[];
  autostart: boolean;
  allow_nonforeground: boolean;
  raise_on_grab: boolean;
  snap_enabled: boolean;
  snap_threshold: number;
  scroll_opacity: boolean;
  scroll_opacity_modifier: ModifierKey;
  drag_threshold: number;
  snap_native: boolean;
}

export const DEFAULT_CONFIG: AppConfig = {
  enabled: true,
  move_enabled: true,
  resize_enabled: true,
  move_modifier: 'alt',
  resize_modifier_1: 'alt',
  resize_modifier_2: 'shift',
  resize_mode: 'quadrant',
  filter_mode: 'blacklist',
  filter_list: [],
  autostart: false,
  allow_nonforeground: true,
  raise_on_grab: false,
  snap_enabled: true,
  snap_threshold: 20,
  scroll_opacity: true,
  scroll_opacity_modifier: 'alt',
  drag_threshold: 10,
  snap_native: true,
};

export const MODIFIER_OPTIONS: { value: ModifierKey; label: string }[] = [
  { value: 'alt', label: 'Alt' },
  { value: 'ctrl', label: 'Ctrl' },
  { value: 'shift', label: 'Shift' },
  { value: 'win', label: 'Win' },
];
