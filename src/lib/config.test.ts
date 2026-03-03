import { describe, it, expect } from 'vitest';
import { DEFAULT_CONFIG, MODIFIER_OPTIONS } from './config';
import type { AppConfig, ModifierKey, FilterMode } from './config';

describe('DEFAULT_CONFIG', () => {
  it('has all required fields defined', () => {
    const keys: (keyof AppConfig)[] = [
      'enabled',
      'move_modifier',
      'resize_modifier_1',
      'resize_modifier_2',
      'filter_mode',
      'filter_list',
      'autostart',
      'allow_nonforeground',
      'raise_on_grab',
      'snap_enabled',
      'snap_threshold',
      'scroll_opacity',
      'scroll_opacity_modifier',
      'middleclick_topmost',
      'drag_threshold',
      'snap_native',
    ];
    for (const key of keys) {
      expect(DEFAULT_CONFIG).toHaveProperty(key);
    }
  });

  it('starts enabled', () => {
    expect(DEFAULT_CONFIG.enabled).toBe(true);
  });

  it('uses Alt as the default move modifier', () => {
    expect(DEFAULT_CONFIG.move_modifier).toBe('alt');
  });

  it('uses Alt+Shift as the default resize modifiers', () => {
    expect(DEFAULT_CONFIG.resize_modifier_1).toBe('alt');
    expect(DEFAULT_CONFIG.resize_modifier_2).toBe('shift');
  });

  it('defaults to blacklist filter mode', () => {
    expect(DEFAULT_CONFIG.filter_mode).toBe('blacklist');
  });

  it('starts with an empty filter list', () => {
    expect(DEFAULT_CONFIG.filter_list).toEqual([]);
  });

  it('does not autostart by default', () => {
    expect(DEFAULT_CONFIG.autostart).toBe(false);
  });

  it('allows non-foreground windows by default', () => {
    expect(DEFAULT_CONFIG.allow_nonforeground).toBe(true);
  });

  it('does not raise on grab by default', () => {
    expect(DEFAULT_CONFIG.raise_on_grab).toBe(false);
  });

  it('enables snapping by default', () => {
    expect(DEFAULT_CONFIG.snap_enabled).toBe(true);
  });

  it('has a reasonable default snap threshold', () => {
    expect(DEFAULT_CONFIG.snap_threshold).toBeGreaterThan(0);
    expect(DEFAULT_CONFIG.snap_threshold).toBeLessThanOrEqual(100);
  });

  it('enables scroll opacity by default', () => {
    expect(DEFAULT_CONFIG.scroll_opacity).toBe(true);
  });

  it('uses Alt as scroll opacity modifier by default', () => {
    expect(DEFAULT_CONFIG.scroll_opacity_modifier).toBe('alt');
  });

  it('enables middle-click topmost by default', () => {
    expect(DEFAULT_CONFIG.middleclick_topmost).toBe(true);
  });

  it('has a non-negative drag threshold', () => {
    expect(DEFAULT_CONFIG.drag_threshold).toBeGreaterThanOrEqual(0);
  });

  it('enables native snap groups by default', () => {
    expect(DEFAULT_CONFIG.snap_native).toBe(true);
  });
});

describe('MODIFIER_OPTIONS', () => {
  const validModifiers: ModifierKey[] = ['alt', 'ctrl', 'shift', 'win'];

  it('contains exactly four options', () => {
    expect(MODIFIER_OPTIONS).toHaveLength(4);
  });

  it('covers all valid modifier keys', () => {
    const values = MODIFIER_OPTIONS.map((o) => o.value);
    for (const mod of validModifiers) {
      expect(values).toContain(mod);
    }
  });

  it('each option has a non-empty label', () => {
    for (const opt of MODIFIER_OPTIONS) {
      expect(opt.label.length).toBeGreaterThan(0);
    }
  });

  it('each option value is a valid ModifierKey', () => {
    for (const opt of MODIFIER_OPTIONS) {
      expect(validModifiers).toContain(opt.value);
    }
  });

  it('has no duplicate values', () => {
    const values = MODIFIER_OPTIONS.map((o) => o.value);
    const unique = new Set(values);
    expect(unique.size).toBe(values.length);
  });
});

describe('type compatibility', () => {
  it('DEFAULT_CONFIG satisfies AppConfig type', () => {
    // Compile-time assertion: if this type-checks, the config is valid
    const cfg: AppConfig = DEFAULT_CONFIG;
    expect(cfg).toBeDefined();
  });

  it('filter_mode type accepts only valid values', () => {
    const modes: FilterMode[] = ['whitelist', 'blacklist'];
    expect(modes).toContain(DEFAULT_CONFIG.filter_mode);
  });

  it('modifier types accept only valid values', () => {
    const modifiers: ModifierKey[] = ['alt', 'ctrl', 'shift', 'win'];
    expect(modifiers).toContain(DEFAULT_CONFIG.move_modifier);
    expect(modifiers).toContain(DEFAULT_CONFIG.resize_modifier_1);
    expect(modifiers).toContain(DEFAULT_CONFIG.resize_modifier_2);
    expect(modifiers).toContain(DEFAULT_CONFIG.scroll_opacity_modifier);
  });
});
