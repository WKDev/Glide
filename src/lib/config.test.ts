import { describe, it, expect } from 'vitest';
import {
  DEFAULT_CONFIG,
  MODIFIER_OPTIONS,
  orderCombo,
  sameCombo,
  toggleComboKey,
} from './config';
import type { AppConfig, ModifierKey, FilterMode, ResizeMode } from './config';

describe('DEFAULT_CONFIG', () => {
  it('has all required fields defined', () => {
    const keys: (keyof AppConfig)[] = [
      'enabled',
      'move_enabled',
      'resize_enabled',
      'move_modifiers',
      'move_trigger',
      'resize_modifiers',
      'resize_trigger',
      'resize_mode',
      'filter_mode',
      'filter_list',
      'autostart',
      'allow_nonforeground',
      'raise_on_grab',
      'snap_enabled',
      'snap_threshold',
      'scroll_opacity',
      'scroll_opacity_modifier',
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
    expect(DEFAULT_CONFIG.move_modifiers).toEqual(['alt']);
  });

  it('uses Alt+Shift as the default resize modifiers', () => {
    expect(DEFAULT_CONFIG.resize_modifiers).toEqual(['alt', 'shift']);
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

  it('enables move by default', () => {
    expect(DEFAULT_CONFIG.move_enabled).toBe(true);
  });

  it('enables resize by default', () => {
    expect(DEFAULT_CONFIG.resize_enabled).toBe(true);
  });

  it('uses quadrant resize mode by default', () => {
    expect(DEFAULT_CONFIG.resize_mode).toBe('quadrant');
    expect(DEFAULT_CONFIG.move_trigger).toBe('click');
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
    for (const key of [
      ...DEFAULT_CONFIG.move_modifiers,
      ...DEFAULT_CONFIG.resize_modifiers,
    ]) {
      expect(modifiers).toContain(key);
    }
    expect(modifiers).toContain(DEFAULT_CONFIG.scroll_opacity_modifier);
  });

  it('resize_mode type accepts only valid values', () => {
    const modes: ResizeMode[] = ['quadrant', 'absolute'];
    expect(modes).toContain(DEFAULT_CONFIG.resize_mode);
  });
});

describe('modifier combinations', () => {
  it('adds keys in Windows shortcut order', () => {
    expect(toggleComboKey(['shift'], 'ctrl')).toEqual(['ctrl', 'shift']);
    expect(orderCombo(['shift', 'win', 'alt'])).toEqual([
      'win',
      'alt',
      'shift',
    ]);
  });

  it('keeps at least one key', () => {
    expect(toggleComboKey(['alt'], 'alt')).toEqual(['alt']);
  });

  it('allows at most three keys', () => {
    expect(toggleComboKey(['ctrl', 'alt', 'shift'], 'win')).toEqual([
      'ctrl',
      'alt',
      'shift',
    ]);
  });

  it('compares combinations regardless of order', () => {
    expect(sameCombo(['alt', 'shift'], ['shift', 'alt'])).toBe(true);
    expect(sameCombo(['alt'], ['alt', 'shift'])).toBe(false);
  });
});
