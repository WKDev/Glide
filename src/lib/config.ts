export type ModifierKey = 'alt' | 'ctrl' | 'shift' | 'win';
export type FilterMode = 'whitelist' | 'blacklist';
export type ResizeMode = 'quadrant' | 'absolute';
export type GestureTrigger = 'click' | 'pan';

export interface AppConfig {
  enabled: boolean;
  move_enabled: boolean;
  resize_enabled: boolean;
  move_modifiers: ModifierKey[];
  move_trigger: GestureTrigger;
  resize_modifiers: ModifierKey[];
  resize_trigger: GestureTrigger;
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
  move_modifiers: ['alt'],
  move_trigger: 'click',
  resize_modifiers: ['alt', 'shift'],
  resize_trigger: 'click',
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

export const MAX_COMBO_KEYS = 3;

export const MODIFIER_OPTIONS: { value: ModifierKey; label: string }[] = [
  { value: 'ctrl', label: 'Ctrl' },
  { value: 'alt', label: 'Alt' },
  { value: 'shift', label: 'Shift' },
  { value: 'win', label: 'Win' },
];

/** Windows shortcut order: Win + Ctrl + Alt + Shift. */
const COMBO_ORDER: ModifierKey[] = ['win', 'ctrl', 'alt', 'shift'];

export function orderCombo(keys: ModifierKey[]): ModifierKey[] {
  return [...new Set(keys)].sort(
    (a, b) => COMBO_ORDER.indexOf(a) - COMBO_ORDER.indexOf(b)
  );
}

export const sortCombo = orderCombo;

export function modifierLabel(key: ModifierKey): string {
  return MODIFIER_OPTIONS.find((o) => o.value === key)?.label ?? key;
}

export const keyLabel = modifierLabel;

export function comboLabel(keys: ModifierKey[]): string {
  return sortCombo(keys).map(keyLabel).join(' + ');
}

export function sameCombo(a: ModifierKey[], b: ModifierKey[]): boolean {
  const x = sortCombo(a);
  const y = sortCombo(b);
  return x.length === y.length && x.every((key, i) => key === y[i]);
}

/**
 * Add or remove one key, keeping one to three keys.
 * Returns the unchanged combination when the change is not allowed.
 */
export function toggleComboKey(
  keys: ModifierKey[],
  key: ModifierKey
): ModifierKey[] {
  if (keys.includes(key)) {
    return keys.length > 1 ? sortCombo(keys.filter((k) => k !== key)) : keys;
  }
  return keys.length < MAX_COMBO_KEYS ? sortCombo([...keys, key]) : keys;
}
