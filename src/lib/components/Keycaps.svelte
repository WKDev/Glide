<script lang="ts">
  import type { ModifierKey } from '../config';
  import { MODIFIER_OPTIONS, MAX_COMBO_KEYS, toggleComboKey } from '../config';

  let {
    value = $bindable(),
    held = [],
    single = false,
    disabled = false,
    label,
  }: {
    /** Selected keys; `single` keeps exactly one. */
    value: ModifierKey[];
    /** Keys the user is physically holding right now. */
    held?: ModifierKey[];
    single?: boolean;
    disabled?: boolean;
    label: string;
  } = $props();

  const full = $derived(!single && value.length >= MAX_COMBO_KEYS);

  function press(key: ModifierKey) {
    value = single ? [key] : toggleComboKey(value, key);
  }

  function hint(key: ModifierKey, selected: boolean): string | undefined {
    if (single) return undefined;
    if (selected && value.length === 1) return 'At least one key is needed';
    if (!selected && full) return `Up to ${MAX_COMBO_KEYS} keys`;
    return undefined;
  }
</script>

<div class="keycaps" role="group" aria-label={label}>
  {#each MODIFIER_OPTIONS as opt (opt.value)}
    {@const selected = value.includes(opt.value)}
    {@const blocked =
      !single && ((selected && value.length === 1) || (!selected && full))}
    <button
      type="button"
      class="keycap"
      class:selected
      class:held={held.includes(opt.value)}
      class:blocked
      aria-pressed={selected}
      aria-disabled={blocked || undefined}
      title={hint(opt.value, selected)}
      {disabled}
      onclick={() => !blocked && press(opt.value)}
    >
      {#if opt.value === 'win'}
        <svg class="win-logo" viewBox="0 0 16 16" aria-hidden="true">
          <path
            d="M1 1h6.5v6.5H1zM8.5 1H15v6.5H8.5zM1 8.5h6.5V15H1zM8.5 8.5H15V15H8.5z"
          />
        </svg>
      {/if}
      <span>{opt.label}</span>
    </button>
  {/each}
</div>

<style>
  .keycaps {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 6px;
  }

  .keycap {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    height: 34px;
    padding: 0 8px 2px;
    border: 1px solid var(--key-stroke);
    border-bottom-width: 3px;
    border-radius: 6px;
    background: var(--key-bg);
    color: var(--text);
    font: inherit;
    font-size: 12.5px;
    font-weight: 600;
    cursor: pointer;
    transition:
      background 120ms ease,
      border-color 120ms ease,
      transform 90ms ease;
  }

  .keycap:hover:not(:disabled):not(.blocked) {
    background: var(--key-hover);
  }

  .keycap:active:not(:disabled):not(.blocked) {
    transform: translateY(1px);
  }

  .keycap.selected {
    background: var(--accent);
    border-color: var(--accent-deep);
    color: var(--on-accent);
  }

  .keycap.selected:hover:not(:disabled):not(.blocked) {
    background: var(--accent-hover);
  }

  .keycap.held {
    transform: translateY(2px);
    box-shadow: 0 0 0 2px var(--held-ring);
  }

  .keycap.blocked {
    cursor: not-allowed;
  }

  .keycap.blocked:not(.selected) {
    color: var(--text-disabled);
  }

  .keycap:disabled {
    cursor: default;
  }

  .keycap:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }

  .win-logo {
    width: 11px;
    height: 11px;
    fill: currentColor;
  }
</style>
