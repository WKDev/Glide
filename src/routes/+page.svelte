<script lang="ts">
  import { onMount } from 'svelte';
  import { Switch, RadioGroup, Select } from 'bits-ui';
  import { invoke } from '@tauri-apps/api/core';
  import { enable, disable, isEnabled } from '@tauri-apps/plugin-autostart';
  import type { AppConfig } from '$lib/config';
  import { MODIFIER_OPTIONS, DEFAULT_CONFIG } from '$lib/config';
  import { saveConfig } from '$lib/store';
  import { check } from '@tauri-apps/plugin-updater';
  import { relaunch } from '@tauri-apps/plugin-process';

  let hasChecked = false;

  let config = $state<AppConfig>({ ...DEFAULT_CONFIG });
  let runningProcesses = $state<string[]>([]);
  let runningProcessValue = $state('');
  let autostartEnabled = $state(false);
  let loaded = $state(false);
  let updateAvailable = $state<{ version: string } | null>(null);
  let isUpdating = $state(false);
  let activeSection = $state<'general' | 'process-filter' | 'about'>('general');
  let lastSavedSnapshot = $state('');
  let saveTimer: ReturnType<typeof setTimeout> | null = null;

  onMount(async () => {
    try {
      const loaded_cfg = await invoke<AppConfig>('get_config');
      config = loaded_cfg;
    } catch {
      config = { ...DEFAULT_CONFIG };
    }
    try {
      autostartEnabled = await isEnabled();
      config.autostart = autostartEnabled;
    } catch {
      autostartEnabled = false;
    }
    await loadRunning();
    config.enabled = true;
    try {
      await invoke('set_hook_enabled', { enabled: true });
    } catch {}
    lastSavedSnapshot = '';
    loaded = true;
  });

  onMount(async () => {
    if (hasChecked) return;
    hasChecked = true;
    try {
      const update = await check();
      if (update) updateAvailable = { version: update.version };
    } catch (e) {
      console.error('Update check failed:', e);
    }
  });

  const moveLabel = $derived(
    MODIFIER_OPTIONS.find((o) => o.value === config.move_modifier)?.label ??
      config.move_modifier
  );
  const resizeLabel1 = $derived(
    MODIFIER_OPTIONS.find((o) => o.value === config.resize_modifier_1)?.label ??
      config.resize_modifier_1
  );
  const resizeLabel2 = $derived(
    MODIFIER_OPTIONS.find((o) => o.value === config.resize_modifier_2)?.label ??
      config.resize_modifier_2
  );
  const filterHint = $derived(
    config.filter_mode === 'whitelist'
      ? 'Only affects listed processes'
      : 'Affects all processes except listed'
  );

  const selectableRunningProcesses = $derived(
    runningProcesses.filter((name) => !config.filter_list.includes(name))
  );

  const selectableRunningProcessItems = $derived(
    selectableRunningProcesses.map((name) => ({ value: name, label: name }))
  );

  function addProcessName(name: string) {
    const normalized = name.trim();
    if (normalized && !config.filter_list.includes(normalized)) {
      config.filter_list = [...config.filter_list, normalized];
    }
  }

  function onRunningProcessSelect(value: string) {
    if (!value) return;
    addProcessName(value);
    runningProcessValue = '';
  }

  async function loadRunning() {
    try {
      runningProcesses = await invoke<string[]>('get_running_processes');
    } catch {
      runningProcesses = [];
    }
  }

  async function toggleAutostart(next: boolean) {
    autostartEnabled = next;
    config.autostart = next;
    try {
      if (next) {
        await enable();
      } else {
        await disable();
      }
    } catch {}
  }

  function removeProcess(name: string) {
    config.filter_list = config.filter_list.filter((p) => p !== name);
  }

  async function installUpdate() {
    if (isUpdating) return;
    isUpdating = true;
    try {
      const update = await check();
      if (update) {
        await update.downloadAndInstall();
        await relaunch();
      }
    } catch (e) {
      console.error('Update install failed:', e);
      isUpdating = false;
    }
  }

  $effect(() => {
    if (!loaded) return;

    const snapshot = JSON.stringify(config);
    if (snapshot === lastSavedSnapshot) return;

    if (saveTimer) {
      clearTimeout(saveTimer);
    }

    saveTimer = setTimeout(async () => {
      try {
        await invoke('set_config', { config });
        await saveConfig(config);
        lastSavedSnapshot = JSON.stringify(config);
      } catch {
        // Ignore autosave errors to keep interactions uninterrupted.
      }
    }, 220);

    return () => {
      if (saveTimer) {
        clearTimeout(saveTimer);
        saveTimer = null;
      }
    };
  });
</script>

<main class:loaded>
  <div class="app-shell">
    <aside class="sidebar">
      <div class="side-brand">
        <span class="side-brand-name">wkgrip</span>
        <span class="side-brand-ver">v0.1.0</span>
      </div>
      <nav class="side-nav" aria-label="Settings sections">
        <button
          type="button"
          class="side-link"
          class:active={activeSection === 'general'}
          onclick={() => (activeSection = 'general')}
        >
          General
        </button>
        <button
          type="button"
          class="side-link"
          class:active={activeSection === 'process-filter'}
          onclick={() => (activeSection = 'process-filter')}
        >
          Process Filter
        </button>
        <button
          type="button"
          class="side-link"
          class:active={activeSection === 'about'}
          onclick={() => (activeSection = 'about')}
        >
          About
        </button>
      </nav>
      <p class="side-note">Settings are saved automatically.</p>
    </aside>

    <div class="content">
      {#if activeSection === 'general'}
        <section id="general" class="group">
          <h1 class="group-title">General</h1>

          <div class="panel">
            <h2 class="panel-title">Modifier Keys</h2>
            <section class="card">
              <div class="mod-row">
                <span class="mod-label">Move</span>
                <Select.Root type="single" bind:value={config.move_modifier}>
                  <Select.Trigger
                    class="select-trigger"
                    aria-label="Move modifier"
                  >
                    <span class="select-value">{moveLabel}</span>
                    <span class="select-caret">▾</span>
                  </Select.Trigger>
                  <Select.Content class="select-content" sideOffset={4}>
                    {#each MODIFIER_OPTIONS as opt}
                      <Select.Item
                        class="select-item"
                        value={opt.value}
                        label={opt.label}>{opt.label}</Select.Item
                      >
                    {/each}
                  </Select.Content>
                </Select.Root>
                <span class="hint-inline"
                  >Hold <kbd>{moveLabel}</kbd> + drag to move</span
                >
              </div>
              <div class="mod-row">
                <span class="mod-label">Resize</span>
                <div class="resize-pair">
                  <Select.Root
                    type="single"
                    bind:value={config.resize_modifier_1}
                  >
                    <Select.Trigger
                      class="select-trigger"
                      aria-label="Resize modifier one"
                    >
                      <span class="select-value">{resizeLabel1}</span>
                      <span class="select-caret">▾</span>
                    </Select.Trigger>
                    <Select.Content class="select-content" sideOffset={4}>
                      {#each MODIFIER_OPTIONS as opt}
                        <Select.Item
                          class="select-item"
                          value={opt.value}
                          label={opt.label}>{opt.label}</Select.Item
                        >
                      {/each}
                    </Select.Content>
                  </Select.Root>
                  <span class="plus">+</span>
                  <Select.Root
                    type="single"
                    bind:value={config.resize_modifier_2}
                  >
                    <Select.Trigger
                      class="select-trigger"
                      aria-label="Resize modifier two"
                    >
                      <span class="select-value">{resizeLabel2}</span>
                      <span class="select-caret">▾</span>
                    </Select.Trigger>
                    <Select.Content class="select-content" sideOffset={4}>
                      {#each MODIFIER_OPTIONS as opt}
                        <Select.Item
                          class="select-item"
                          value={opt.value}
                          label={opt.label}>{opt.label}</Select.Item
                        >
                      {/each}
                    </Select.Content>
                  </Select.Root>
                </div>
                <span class="hint-inline"
                  >Hold <kbd>{resizeLabel1}</kbd>+<kbd>{resizeLabel2}</kbd> + drag</span
                >
              </div>
            </section>
          </div>

          <div class="panel">
            <h2 class="panel-title">Behavior</h2>
            <section class="card">
              <div class="row-item">
                <span class="row-label">Allow non-foreground windows</span>
                <Switch.Root
                  class="toggle"
                  bind:checked={config.allow_nonforeground}
                  aria-label="Toggle allow non-foreground windows"
                >
                  <Switch.Thumb class="thumb" />
                </Switch.Root>
              </div>
              <div class="row-item">
                <span class="row-label">Raise window on grab</span>
                <Switch.Root
                  class="toggle"
                  bind:checked={config.raise_on_grab}
                  aria-label="Toggle raise window on grab"
                >
                  <Switch.Thumb class="thumb" />
                </Switch.Root>
              </div>
              <div class="row-item">
                <span class="row-label">Edge snapping</span>
                <Switch.Root
                  class="toggle"
                  bind:checked={config.snap_enabled}
                  aria-label="Toggle edge snapping"
                >
                  <Switch.Thumb class="thumb" />
                </Switch.Root>
              </div>
              <div class="row-item">
                <span class="row-label">Scroll to change opacity</span>
                <Switch.Root
                  class="toggle"
                  bind:checked={config.scroll_opacity}
                  aria-label="Toggle scroll opacity"
                >
                  <Switch.Thumb class="thumb" />
                </Switch.Root>
              </div>
              <div class="row-item">
                <span class="row-label">Middle-click always-on-top</span>
                <Switch.Root
                  class="toggle"
                  bind:checked={config.middleclick_topmost}
                  aria-label="Toggle middle-click always-on-top"
                >
                  <Switch.Thumb class="thumb" />
                </Switch.Root>
              </div>
            </section>
          </div>

          <div class="panel">
            <h2 class="panel-title">Autostart</h2>
            <section class="card card-row">
              <div class="row-item">
                <span class="row-label">Start with Windows</span>
                <Switch.Root
                  class="toggle"
                  checked={autostartEnabled}
                  onCheckedChange={toggleAutostart}
                  aria-label="Toggle autostart"
                >
                  <Switch.Thumb class="thumb" />
                </Switch.Root>
              </div>
            </section>
          </div>
        </section>
      {/if}

      {#if activeSection === 'process-filter'}
        <section id="process-filter" class="group">
          <h1 class="group-title">Process Filter</h1>
          <div class="panel">
            <h2 class="panel-title">Rules</h2>
            <section class="card">
              <div class="filter-top">
                <RadioGroup.Root
                  class="radio-tabs"
                  bind:value={config.filter_mode}
                >
                  <RadioGroup.Item class="rtab" value="whitelist"
                    >Whitelist</RadioGroup.Item
                  >
                  <RadioGroup.Item class="rtab" value="blacklist"
                    >Blacklist</RadioGroup.Item
                  >
                </RadioGroup.Root>
                <span class="filter-hint">{filterHint}</span>
              </div>

              <div class="process-picker-row">
                <Select.Root
                  type="single"
                  bind:value={runningProcessValue}
                  items={selectableRunningProcessItems}
                  onValueChange={onRunningProcessSelect}
                >
                  <Select.Trigger
                    class="select-trigger process-select"
                    aria-label="Select running process"
                  >
                    <span class="select-value"
                      >{runningProcessValue || 'Select running process'}</span
                    >
                    <span class="select-caret">▾</span>
                  </Select.Trigger>
                  <Select.Content class="select-content" sideOffset={4}>
                    {#if selectableRunningProcesses.length === 0}
                      <div class="running-empty">
                        No running process available
                      </div>
                    {:else}
                      {#each selectableRunningProcesses as proc}
                        <Select.Item
                          class="select-item"
                          value={proc}
                          label={proc}>{proc}</Select.Item
                        >
                      {/each}
                    {/if}
                  </Select.Content>
                </Select.Root>
                <button
                  type="button"
                  class="refresh-running-btn"
                  onclick={loadRunning}>Refresh</button
                >
              </div>

              <div class="pill-list">
                {#each config.filter_list as proc}
                  <span class="pill">
                    {proc}
                    <button
                      type="button"
                      class="pill-x"
                      onclick={() => removeProcess(proc)}
                      aria-label="Remove {proc}">×</button
                    >
                  </span>
                {/each}
                {#if config.filter_list.length === 0}
                  <span class="pill-empty">No processes listed</span>
                {/if}
              </div>
            </section>
          </div>
        </section>
      {/if}

      {#if activeSection === 'about'}
        <section id="about" class="group">
          <h1 class="group-title">About</h1>
          <div class="panel">
            <h2 class="panel-title">wkgrip</h2>
            <section class="card about-card">
              <p>Keyboard-assisted window move/resize utility.</p>
              <p>Version: <strong>v0.1.0</strong></p>
              <p>
                Persistence: settings auto-save to Tauri runtime config and
                local store.
              </p>
            </section>
            {#if updateAvailable !== null}
              <section class="card update-card">
                <p>
                  새 버전 <strong>{updateAvailable.version}</strong> 사용 가능
                </p>
                <button
                  class="update-btn"
                  onclick={installUpdate}
                  disabled={isUpdating}
                >
                  {isUpdating ? '설치 중...' : '업데이트'}
                </button>
              </section>
            {/if}
          </div>
        </section>
      {/if}
    </div>
  </div>
</main>

<style>
  :global(*),
  :global(*::before),
  :global(*::after) {
    box-sizing: border-box;
    margin: 0;
    padding: 0;
  }

  :global(html),
  :global(body) {
    width: 560px;
    height: 640px;
    overflow: hidden;
    background: transparent;
    color: #111827;
    color-scheme: light dark;
    -webkit-font-smoothing: antialiased;
    text-rendering: optimizeLegibility;
  }

  main {
    --bg-base: #eef3fb;
    --bg-tint: #f9fbff;
    --surface: rgb(255 255 255 / 0.76);
    --surface-strong: rgb(255 255 255 / 0.9);
    --surface-alt: rgb(247 250 255 / 0.72);
    --line: rgb(125 145 176 / 0.26);
    --line-hover: rgb(86 113 153 / 0.42);
    --text: #18243a;
    --muted: #4a5d7a;
    --muted-2: #667899;
    --accent: #005fb8;
    --accent-strong: #155fb7;
    --toggle-off: #d9e4f4;
    --thumb-off: #60708a;
    --thumb-on: #ffffff;
    --field-bg: rgb(255 255 255 / 0.72);
    --field-line: rgb(106 132 170 / 0.34);
    --chip-bg: rgb(238 246 255 / 0.8);
    --chip-line: rgb(106 132 170 / 0.35);
    --chip-text: #2f4668;
    --chip-x: #496086;
    --chip-x-hover: #142743;
    --seg-bg: rgb(230 239 252 / 0.84);
    --seg-line: rgb(106 132 170 / 0.34);
    --seg-active: rgb(255 255 255 / 0.84);
    --seg-hover: rgb(244 248 255 / 0.9);
    --btn-bg: rgb(255 255 255 / 0.78);
    --btn-line: rgb(106 132 170 / 0.34);
    --btn-hover: rgb(245 250 255 / 0.94);
    --scroll-thumb: rgb(114 137 169 / 0.55);
    --scroll-thumb-hover: rgb(79 106 143 / 0.72);

    position: relative;
    display: block;
    height: 640px;
    overflow: hidden;
    background: transparent;
    color: var(--text);
    font-family:
      'Segoe UI Variable Text', 'Segoe UI', 'Noto Sans KR', sans-serif;
    font-size: 13px;
    line-height: 1.36;
    opacity: 0;
    transform: translateY(6px);
    transition:
      opacity 180ms ease,
      transform 220ms ease;
  }

  main::before {
    content: none;
  }

  main.loaded {
    opacity: 1;
    transform: translateY(0);
  }

  .app-shell {
    position: relative;
    z-index: 1;
    height: 100%;
    display: grid;
    grid-template-columns: 148px minmax(0, 1fr);
    background: rgb(255 255 255 / 0.08);
    backdrop-filter: blur(18px) saturate(130%);
    border: 1px solid rgb(255 255 255 / 0.35);
    box-shadow:
      inset 0 1px 0 rgb(255 255 255 / 0.58),
      0 20px 48px rgb(31 60 100 / 0.18),
      0 1px 1px rgb(51 66 91 / 0.15);
  }

  .sidebar {
    border-right: 1px solid var(--line);
    background: linear-gradient(
      180deg,
      rgb(255 255 255 / 0.44),
      rgb(240 246 255 / 0.28)
    );
    padding: 16px 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .side-brand {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 0 4px;
  }

  .side-brand-name {
    color: var(--text);
    font-size: 16px;
    font-weight: 600;
    letter-spacing: 0.2px;
    line-height: 1.1;
  }

  .side-brand-ver {
    color: var(--muted-2);
    font-size: 11px;
    line-height: 1.2;
  }

  .side-nav {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .side-link {
    border: 1px solid transparent;
    background: transparent;
    text-align: left;
    color: var(--muted);
    font-size: 12.5px;
    padding: 8px 9px;
    border-radius: 8px;
    cursor: pointer;
    transition:
      background 0.14s ease,
      border-color 0.14s ease,
      color 0.14s ease;
  }

  .side-link:hover {
    background: rgb(255 255 255 / 0.48);
    border-color: rgb(106 132 170 / 0.22);
    color: var(--text);
  }

  .side-link.active {
    background: color-mix(in oklab, var(--accent) 20%, rgb(255 255 255 / 0.78));
    border-color: color-mix(in oklab, var(--accent) 30%, transparent);
    color: #15396f;
    box-shadow: inset 0 1px 0 rgb(255 255 255 / 0.44);
  }

  .side-link:focus-visible {
    outline: 2px solid color-mix(in oklab, var(--accent) 26%, transparent);
    outline-offset: 1px;
  }

  .side-note {
    margin-top: auto;
    color: var(--muted-2);
    font-size: 10.5px;
    line-height: 1.4;
    padding: 0 4px;
  }

  .content {
    overflow-y: auto;
    padding: 14px 14px 12px;
  }

  .content::-webkit-scrollbar {
    width: 11px;
  }

  .content::-webkit-scrollbar-track {
    background: transparent;
  }

  .content::-webkit-scrollbar-thumb {
    background: var(--scroll-thumb);
    border: 2px solid transparent;
    background-clip: content-box;
    border-radius: 999px;
  }

  .content::-webkit-scrollbar-thumb:hover {
    background: var(--scroll-thumb-hover);
    background-clip: content-box;
  }

  .group-title {
    color: var(--muted);
    font-size: 13px;
    font-weight: 600;
    margin-bottom: 9px;
    letter-spacing: 0.1px;
  }

  .group > .panel + .panel {
    margin-top: 11px;
  }

  .panel-title {
    color: var(--muted);
    margin-bottom: 7px;
    font-size: 12.5px;
    font-weight: 600;
    line-height: 1.2;
  }

  .card {
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: 10px;
    padding: 11px;
    backdrop-filter: blur(10px) saturate(115%);
    box-shadow:
      inset 0 1px 0 rgb(255 255 255 / 0.62),
      0 7px 22px rgb(31 60 100 / 0.08);
  }

  .update-card {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 10px 14px;
  }

  .update-btn {
    flex-shrink: 0;
    padding: 5px 14px;
    border-radius: 6px;
    border: 1px solid var(--accent);
    background: var(--accent);
    color: #fff;
    font-size: 0.8rem;
    cursor: pointer;
    transition: opacity 0.15s;
  }

  .update-btn:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }

  .card-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .mod-row {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 30px;
  }

  .mod-row + .mod-row {
    margin-top: 9px;
  }

  .mod-label {
    width: 44px;
    flex-shrink: 0;
    color: var(--text);
    font-size: 13px;
  }

  .resize-pair {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .plus {
    color: var(--muted);
    font-size: 13px;
    line-height: 1;
  }

  .hint-inline {
    margin-left: auto;
    color: var(--muted);
    font-size: 11px;
    white-space: nowrap;
  }

  kbd {
    display: inline-block;
    border: 1px solid rgb(97 122 161 / 0.28);
    border-radius: 5px;
    padding: 1px 6px;
    background: rgb(255 255 255 / 0.64);
    color: var(--muted);
    font-size: 10px;
    line-height: 1.45;
    font-family: 'Consolas', 'Cascadia Mono', ui-monospace, monospace;
  }

  .row-item {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }

  .row-item + .row-item {
    margin-top: 7px;
  }

  .row-label {
    color: var(--text);
    font-size: 13px;
    line-height: 1.3;
  }

  :global(.toggle) {
    position: relative;
    width: 38px;
    height: 20px;
    flex-shrink: 0;
    border: 1px solid var(--line);
    border-radius: 999px;
    background: var(--toggle-off);
    cursor: pointer;
    outline: none;
    transition:
      background 0.15s ease,
      border-color 0.15s ease;
  }

  :global(.toggle .thumb) {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 14px;
    height: 14px;
    border-radius: 999px;
    background: var(--thumb-off);
    transition:
      left 0.15s ease,
      background 0.15s ease;
    box-shadow: 0 1px 3px rgb(0 0 0 / 0.18);
  }

  :global(.toggle[data-state='checked']) {
    background: var(--accent);
    border-color: var(--accent-strong);
  }

  :global(.toggle[data-state='checked'] .thumb) {
    left: 20px;
    background: var(--thumb-on);
  }

  :global(.toggle:focus-visible) {
    box-shadow: 0 0 0 2px color-mix(in oklab, var(--accent) 22%, transparent);
  }

  :global(.select-trigger) {
    height: 29px;
    min-width: 76px;
    padding: 4px 9px;
    border: 1px solid var(--field-line);
    border-radius: 7px;
    background: var(--field-bg);
    color: var(--text);
    font-family: inherit;
    font-size: 12px;
    display: inline-flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    cursor: pointer;
    outline: none;
    transition:
      border-color 0.15s ease,
      background 0.15s ease;
  }

  :global(.select-trigger:hover) {
    border-color: var(--line-hover);
    background: rgb(255 255 255 / 0.86);
  }

  :global(.select-trigger:focus-visible) {
    border-color: var(--accent);
    box-shadow: 0 0 0 2px color-mix(in oklab, var(--accent) 20%, transparent);
  }

  :global(.select-value) {
    white-space: nowrap;
  }

  :global(.select-caret) {
    color: var(--muted);
    font-size: 11px;
    line-height: 1;
  }

  :global(.select-content) {
    z-index: 40;
    border: 1px solid var(--line);
    border-radius: 9px;
    background: var(--surface-strong);
    backdrop-filter: blur(14px) saturate(125%);
    padding: 4px;
    box-shadow:
      0 14px 30px rgb(36 57 90 / 0.24),
      0 2px 8px rgb(36 57 90 / 0.14);
    min-width: 96px;
    max-height: 220px;
    overflow: auto;
    transform-origin: var(--bits-select-content-transform-origin);
  }

  :global(.select-content[data-state='open']) {
    animation: select-pop-in 120ms cubic-bezier(0.2, 0.8, 0.2, 1);
  }

  :global(.select-content[data-side='top'][data-state='open']) {
    animation-name: select-pop-in-up;
  }

  :global(.select-content::-webkit-scrollbar) {
    width: 8px;
  }

  :global(.select-content::-webkit-scrollbar-thumb) {
    background: var(--scroll-thumb);
    border-radius: 999px;
  }

  :global(.select-content::-webkit-scrollbar-thumb:hover) {
    background: var(--scroll-thumb-hover);
  }

  :global(.select-item) {
    border-radius: 6px;
    padding: 6px 8px;
    font-size: 12px;
    color: var(--text);
    cursor: pointer;
    outline: none;
  }

  :global(.select-item[data-highlighted]) {
    background: rgb(226 238 255 / 0.9);
  }

  :global(.select-item[data-state='checked']) {
    font-weight: 600;
  }

  .filter-top {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 8px;
  }

  :global(.radio-tabs) {
    display: inline-flex;
    border: 1px solid var(--seg-line);
    border-radius: 8px;
    background: var(--seg-bg);
    padding: 2px;
    gap: 2px;
  }

  :global(.rtab) {
    display: flex;
    align-items: center;
    padding: 5px 11px;
    border-radius: 6px;
    color: var(--muted);
    font-size: 12px;
    cursor: pointer;
    user-select: none;
    transition:
      background 0.15s ease,
      color 0.15s ease;
  }

  :global(.rtab[data-state='checked']) {
    background: var(--seg-active);
    color: var(--text);
    box-shadow: inset 0 1px 0 rgb(255 255 255 / 0.5);
  }

  :global(.rtab[data-state='unchecked']:hover) {
    background: var(--seg-hover);
  }

  .filter-hint {
    color: var(--muted);
    font-size: 11px;
  }

  .process-picker-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 8px;
  }

  :global(.process-select) {
    flex: 1;
    min-width: 0;
  }

  .refresh-running-btn {
    height: 29px;
    padding: 4px 11px;
    border: 1px solid var(--btn-line);
    border-radius: 7px;
    background: var(--btn-bg);
    color: var(--text);
    font-family: inherit;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    transition:
      background 0.14s ease,
      border-color 0.14s ease;
  }

  .refresh-running-btn:hover {
    background: var(--btn-hover);
    border-color: var(--line-hover);
  }

  .refresh-running-btn:focus-visible {
    outline: 2px solid color-mix(in oklab, var(--accent) 22%, transparent);
    outline-offset: 1px;
  }

  .running-empty {
    padding: 7px 8px;
    color: var(--muted-2);
    font-size: 11px;
  }

  .pill-list {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    min-height: 28px;
    margin-bottom: 8px;
    align-content: flex-start;
  }

  .pill {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 2px 7px 2px 9px;
    border: 1px solid var(--chip-line);
    border-radius: 999px;
    background: var(--chip-bg);
    color: var(--chip-text);
    font-size: 11px;
    line-height: 1.4;
    font-family: 'Consolas', 'Cascadia Mono', ui-monospace, monospace;
  }

  .pill-x {
    display: flex;
    align-items: center;
    border: none;
    background: none;
    padding: 0;
    color: var(--chip-x);
    font-size: 14px;
    line-height: 1;
    cursor: pointer;
    transition: color 0.15s ease;
  }

  .pill-x:hover {
    color: var(--chip-x-hover);
  }

  .pill-empty {
    color: var(--muted-2);
    font-size: 11px;
    line-height: 28px;
    font-style: italic;
  }

  .about-card p {
    color: var(--muted);
    font-size: 12px;
    line-height: 1.45;
    margin-top: 6px;
  }

  .about-card strong {
    color: var(--text);
    font-weight: 600;
  }

  @keyframes select-pop-in {
    from {
      opacity: 0;
      transform: translateY(-2px) scale(0.985);
    }

    to {
      opacity: 1;
      transform: translateY(0) scale(1);
    }
  }

  @keyframes select-pop-in-up {
    from {
      opacity: 0;
      transform: translateY(2px) scale(0.985);
    }

    to {
      opacity: 1;
      transform: translateY(0) scale(1);
    }
  }

  @media (prefers-color-scheme: dark) {
    :global(html),
    :global(body) {
      color: #e6edf9;
      background: transparent;
    }

    main {
      --bg-base: #0f1725;
      --bg-tint: #172134;
      --surface: rgb(29 40 59 / 0.7);
      --surface-strong: rgb(23 33 49 / 0.9);
      --surface-alt: rgb(33 44 66 / 0.72);
      --line: rgb(167 194 235 / 0.2);
      --line-hover: rgb(184 211 249 / 0.35);
      --text: #e6edf9;
      --muted: #b0c2de;
      --muted-2: #8ea2c1;
      --accent: #66b4ff;
      --accent-strong: #5aa7f3;
      --toggle-off: #33425e;
      --thumb-off: #c2d2e8;
      --thumb-on: #f6f9ff;
      --field-bg: rgb(24 34 52 / 0.82);
      --field-line: rgb(168 193 230 / 0.28);
      --chip-bg: rgb(45 59 85 / 0.74);
      --chip-line: rgb(168 193 230 / 0.3);
      --chip-text: #d7e5fa;
      --chip-x: #abc5e6;
      --chip-x-hover: #f3f7ff;
      --seg-bg: rgb(34 47 69 / 0.82);
      --seg-line: rgb(168 193 230 / 0.28);
      --seg-active: rgb(51 66 95 / 0.84);
      --seg-hover: rgb(58 74 105 / 0.84);
      --btn-bg: rgb(30 43 64 / 0.82);
      --btn-line: rgb(168 193 230 / 0.28);
      --btn-hover: rgb(42 58 84 / 0.84);
      --scroll-thumb: rgb(141 168 206 / 0.52);
      --scroll-thumb-hover: rgb(173 198 235 / 0.75);

      background: transparent;
    }

    .app-shell {
      background: rgb(17 25 39 / 0.2);
      border-color: rgb(145 171 209 / 0.22);
      box-shadow:
        inset 0 1px 0 rgb(196 215 245 / 0.16),
        0 24px 52px rgb(2 8 18 / 0.5),
        0 1px 1px rgb(8 15 30 / 0.4);
    }

    .sidebar {
      background: linear-gradient(
        180deg,
        rgb(30 42 62 / 0.42),
        rgb(21 31 47 / 0.3)
      );
    }

    .side-link.active {
      color: #deebff;
    }

    :global(.select-item[data-highlighted]) {
      background: rgb(72 96 134 / 0.75);
    }

    .update-btn {
      background: var(--accent-strong);
      border-color: var(--accent-strong);
    }
  }

  @media (max-width: 560px) {
    :global(html),
    :global(body),
    main {
      width: 100%;
      min-width: 360px;
    }

    .app-shell {
      grid-template-columns: 130px minmax(0, 1fr);
    }
  }
</style>
