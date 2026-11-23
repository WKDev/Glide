<script lang="ts">
  import { onMount } from 'svelte';
  import { Switch, RadioGroup, Select } from 'bits-ui';
  import { invoke } from '@tauri-apps/api/core';
  import { enable, disable, isEnabled } from '@tauri-apps/plugin-autostart';
  import type { AppConfig, GestureTrigger, ModifierKey } from '../lib/config';
  import {
    DEFAULT_CONFIG,
    modifierLabel,
    orderCombo,
    sameCombo,
  } from '../lib/config';
  import Keycaps from '../lib/components/Keycaps.svelte';
  import MouseGlyph from '../lib/components/MouseGlyph.svelte';
  import SupportDialog from '../lib/components/SupportDialog.svelte';
  import { check } from '@tauri-apps/plugin-updater';
  import { getVersion } from '@tauri-apps/api/app';
  import { relaunch } from '@tauri-apps/plugin-process';

  type Section = 'gestures' | 'behavior' | 'apps' | 'about';

  /** Segoe Fluent Icons code points. */
  const GLYPH = {
    error: '\uEA39',
    warning: '\uE7BA',
    move: '\uE7C2',
    resize: '\uE740',
    opacity: '\uE790',
    add: '\uE710',
    chevron: '\uE70D',
    refresh: '\uE72C',
    app: '\uE737',
    remove: '\uE711',
    heart: '\uEB52',
    external: '\uE8A7',
  };

  const SECTIONS: { id: Section; label: string; glyph: string }[] = [
    { id: 'gestures', label: 'Gestures', glyph: '\uE962' },
    { id: 'behavior', label: 'Behavior', glyph: '\uE713' },
    { id: 'apps', label: 'Apps', glyph: '\uE71D' },
    { id: 'about', label: 'About', glyph: '\uE946' },
  ];

  let appVersion = $state('');

  let config = $state<AppConfig>({ ...DEFAULT_CONFIG });
  let runningProcesses = $state<string[]>([]);
  let runningProcessValue = $state('');
  let autostartEnabled = $state(false);
  let changingAutostart = $state(false);
  let loaded = $state(false);
  let hookReady = $state<boolean | null>(null);
  let storeBuild = $state(false);
  let checkingUpdate = $state(false);
  let updateAvailable = $state<{ version: string } | null>(null);
  let isUpdating = $state(false);
  let autostartError = $state<string | null>(null);
  let updateError = $state<string | null>(null);
  let updateCheckFailed = $state(false);
  let activeSection = $state<Section>('gestures');
  let lastSavedSnapshot = $state('');
  let saving = $state(false);
  let failedSnapshot = $state<string | null>(null);
  let loadError = $state(false);
  let heldKeys = $state<ModifierKey[]>([]);
  let supporter = $state<string | null>(null);
  let supportPrompt = $state(false);
  let supportCode = $state('');
  let redeeming = $state(false);
  let redeemError = $state<string | null>(null);
  let linkError = $state<string | null>(null);
  let codeInput = $state<globalThis.HTMLInputElement>();
  const snapshot = $derived(JSON.stringify(config));
  const hasUnsavedChanges = $derived(loaded && snapshot !== lastSavedSnapshot);

  onMount(async () => {
    const getStatus = invoke<{ hook_ready: boolean; store_build: boolean }>(
      'get_app_status'
    )
      .then((status) => {
        hookReady = status.hook_ready;
        storeBuild = status.store_build;
        if (!storeBuild) void checkForUpdate();
      })
      .catch((error) => {
        console.error('Failed to read Glide status:', error);
        updateCheckFailed = true;
      });

    try {
      config = await invoke<AppConfig>('get_config');
      try {
        await getStatus;
        autostartEnabled = storeBuild
          ? await invoke<boolean>('store_autostart')
          : await isEnabled();
        config.autostart = autostartEnabled;
      } catch (error) {
        console.error('Failed to read autostart state:', error);
        autostartEnabled = config.autostart;
      }
      lastSavedSnapshot = JSON.stringify(config);
      loaded = true;
    } catch (error) {
      console.error('Failed to load settings:', error);
      loadError = true;
    }

    void loadRunning();
    void loadSupport();
    appVersion = await getVersion().catch(() => '');
    await getStatus;
  });

  // ---- Held-key preview: keycaps press down while the user holds them. ----
  const KEY_NAMES: Record<string, ModifierKey> = {
    Control: 'ctrl',
    Alt: 'alt',
    Shift: 'shift',
    Meta: 'win',
    OS: 'win',
  };

  function readHeld(event: globalThis.KeyboardEvent) {
    const next: ModifierKey[] = [];
    if (event.metaKey) next.push('win');
    if (event.ctrlKey) next.push('ctrl');
    if (event.altKey) next.push('alt');
    if (event.shiftKey) next.push('shift');
    const own = KEY_NAMES[event.key];
    if (own && event.type === 'keydown' && !next.includes(own)) next.push(own);
    if (own && event.type === 'keyup') {
      heldKeys = orderCombo(next.filter((k) => k !== own));
    } else {
      heldKeys = orderCombo(next);
    }
    // Alt alone would otherwise move focus to the (absent) menu bar.
    if (event.key === 'Alt') event.preventDefault();
  }

  // ---- Gesture wording ----
  const comboText = (keys: ModifierKey[]) =>
    keys.map(modifierLabel).join(' + ');

  function recipe(trigger: GestureTrigger, button: 'left' | 'right') {
    if (trigger === 'pan') return 'then move the mouse';
    return button === 'left'
      ? 'then left-click and drag'
      : 'then right-click and drag';
  }

  const moveReady = $derived(
    config.move_enabled && sameCombo(heldKeys, config.move_modifiers)
  );
  const resizeReady = $derived(
    config.resize_enabled && sameCombo(heldKeys, config.resize_modifiers)
  );

  // ---- Conflicts: surfaced, never silent ----
  const conflict = $derived.by(() => {
    if (!config.move_enabled || !config.resize_enabled) return null;
    if (!sameCombo(config.move_modifiers, config.resize_modifiers)) return null;
    const keys = comboText(config.move_modifiers);
    const m = config.move_trigger;
    const r = config.resize_trigger;
    if (m === 'click' && r === 'click') return null;
    if (m === 'pan' && r === 'pan') {
      return `Move and Resize both use ${keys} with “Just move”. Holding ${keys} will always move the window, so Resize never starts. Change one of the key sets.`;
    }
    const panner = m === 'pan' ? 'Move' : 'Resize';
    const clicker = m === 'pan' ? 'Resize' : 'Move';
    return `${panner} starts as soon as the mouse moves with ${keys} held, so ${clicker} rarely gets a chance. Use different keys for one of them.`;
  });

  function panNote(keys: ModifierKey[], trigger: GestureTrigger) {
    if (trigger !== 'pan' || keys.length !== 1) return null;
    if (keys[0] === 'ctrl' || keys[0] === 'shift') {
      return `Many apps use ${modifierLabel(keys[0])} while moving the mouse. Adding a second key avoids moving windows by accident.`;
    }
    return null;
  }

  const filterHint = $derived(
    config.filter_mode === 'whitelist'
      ? 'Glide works only in the apps listed below.'
      : 'Glide works everywhere except the apps listed below.'
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
    } catch (e) {
      console.error('Failed to load running processes:', e);
      runningProcesses = [];
    }
  }

  async function toggleAutostart(next: boolean) {
    if (changingAutostart) return;
    changingAutostart = true;
    const previous = autostartEnabled;
    autostartEnabled = next;
    autostartError = null;
    try {
      if (storeBuild) {
        autostartEnabled = await invoke<boolean>('store_autostart', {
          enabled: next,
        });
        if (autostartEnabled !== next) {
          autostartError = 'Change the Glide setting in Windows Startup Apps.';
        }
      } else {
        if (next) await enable();
        else await disable();
        autostartEnabled = next;
      }
      config.autostart = autostartEnabled;
    } catch (error) {
      autostartEnabled = previous;
      console.error('Could not change autostart:', error);
      autostartError = storeBuild
        ? 'Could not change autostart. Check Windows Startup Apps.'
        : 'Could not change autostart.';
    } finally {
      changingAutostart = false;
    }
  }

  function removeProcess(name: string) {
    config.filter_list = config.filter_list.filter((p) => p !== name);
  }

  async function checkForUpdate() {
    checkingUpdate = true;
    updateCheckFailed = false;
    try {
      const update = await check();
      updateAvailable = update ? { version: update.version } : null;
      await update?.close();
    } catch (error) {
      console.error('Update check failed:', error);
      updateCheckFailed = true;
    } finally {
      checkingUpdate = false;
    }
  }

  async function installUpdate() {
    if (isUpdating || saving || hasUnsavedChanges || changingAutostart) return;
    isUpdating = true;
    updateError = null;
    try {
      const update = await check();
      if (!update) {
        updateAvailable = null;
        return;
      }
      await update.downloadAndInstall();
      await relaunch();
    } catch (error) {
      console.error('Update install failed:', error);
      updateError = 'Update failed. Please try again.';
    } finally {
      isUpdating = false;
    }
  }

  // ---- Support ----
  type LinkTarget = 'sponsors' | 'coffee' | 'author' | 'repo';

  async function loadSupport() {
    try {
      const status = await invoke<{
        supporter: string | null;
        prompt: boolean;
      }>('get_support_status');
      supporter = status.supporter;
      supportPrompt = status.prompt;
    } catch (error) {
      console.error('Failed to read support status:', error);
    }
  }

  async function openLink(target: LinkTarget) {
    linkError = null;
    try {
      await invoke('open_link', { target });
    } catch (error) {
      console.error('Could not open link:', error);
      linkError = 'Could not open your browser.';
    }
  }

  async function answerPrompt(choice: 'donate' | 'later') {
    supportPrompt = false;
    try {
      await invoke('answer_support_prompt', { choice });
    } catch (error) {
      console.error('Could not save support choice:', error);
    }
  }

  function donateFromPrompt(target: 'sponsors' | 'coffee') {
    void openLink(target);
    void answerPrompt('donate');
  }

  function enterCodeFromPrompt() {
    // Closing for a code is not a decision; the prompt returns until it is redeemed.
    supportPrompt = false;
    activeSection = 'about';
    globalThis.requestAnimationFrame(() => codeInput?.focus());
  }

  async function redeemCode(event: globalThis.SubmitEvent) {
    event.preventDefault();
    if (redeeming || !supportCode.trim()) return;
    redeeming = true;
    redeemError = null;
    try {
      supporter = await invoke<string>('redeem_supporter_code', {
        code: supportCode,
      });
      supportCode = '';
    } catch (error) {
      redeemError =
        typeof error === 'string' ? error : 'Could not check the code.';
    } finally {
      redeeming = false;
    }
  }

  async function saveSettings(sentSnapshot: string) {
    saving = true;
    try {
      await invoke('set_config', {
        config: JSON.parse(sentSnapshot) as AppConfig,
      });
      lastSavedSnapshot = sentSnapshot;
      failedSnapshot = null;
    } catch (error) {
      console.error('Autosave failed:', error);
      failedSnapshot = sentSnapshot;
    } finally {
      saving = false;
    }
  }

  $effect(() => {
    if (!hasUnsavedChanges || saving || snapshot === failedSnapshot) return;
    const pendingSnapshot = snapshot;
    const timer = setTimeout(() => void saveSettings(pendingSnapshot), 220);
    return () => clearTimeout(timer);
  });
</script>

<svelte:window
  onkeydown={readHeld}
  onkeyup={readHeld}
  onblur={() => (heldKeys = [])}
/>

{#snippet keyChips(keys: ModifierKey[])}
  {#each keys as key, i (key)}
    {#if i > 0}<span class="plus" aria-hidden="true">+</span>{/if}
    <kbd>{modifierLabel(key)}</kbd>
  {/each}
{/snippet}

{#snippet triggerPicker(
  value: GestureTrigger,
  onChange: (v: GestureTrigger) => void,
  button: 'left' | 'right',
  disabled: boolean,
  label: string
)}
  <RadioGroup.Root
    class="segmented"
    {value}
    onValueChange={(v) => onChange(v as GestureTrigger)}
    {disabled}
    aria-label={label}
  >
    <RadioGroup.Item class="segment" value="click">
      <MouseGlyph action={button} />
      {button === 'left' ? 'Click and drag' : 'Right-click and drag'}
    </RadioGroup.Item>
    <RadioGroup.Item class="segment" value="pan">
      <MouseGlyph action="move" />
      Just move
    </RadioGroup.Item>
  </RadioGroup.Root>
{/snippet}

<main class:loaded>
  <aside class="nav-pane">
    <div class="brand">
      <img class="brand-mark" src="/glide-mark.png" alt="" />
      <div class="brand-text">
        <span class="brand-name">Glide</span>
        {#if supporter}
          <span
            class="brand-sub supporter-badge"
            title="Thank you, {supporter}"
          >
            <span class="glyph" aria-hidden="true">{GLYPH.heart}</span>
            Supporter
          </span>
        {:else}
          <span class="brand-sub">Settings</span>
        {/if}
      </div>
    </div>

    <nav aria-label="Settings sections">
      {#each SECTIONS as s (s.id)}
        <button
          type="button"
          class="nav-item"
          class:active={activeSection === s.id}
          aria-current={activeSection === s.id ? 'page' : undefined}
          onclick={() => (activeSection = s.id)}
        >
          <span class="glyph" aria-hidden="true">{s.glyph}</span>
          {s.label}
        </button>
      {/each}
    </nav>

    <div class="save-state" aria-live="polite">
      {#if loadError}
        <p class="error-text" role="alert">
          Could not load settings. Restart Glide to try again.
        </p>
      {:else if failedSnapshot === snapshot && hasUnsavedChanges}
        <p class="error-text" role="alert">Could not save settings.</p>
        <button class="button" onclick={() => (failedSnapshot = null)}
          >Try again</button
        >
      {:else}
        <p class="caption">
          <span class="dot" class:busy={saving || hasUnsavedChanges}></span>
          {saving || hasUnsavedChanges
            ? 'Saving…'
            : 'Changes save automatically'}
        </p>
      {/if}
    </div>
  </aside>

  <div class="page" inert={!loaded || isUpdating}>
    {#if hookReady === false}
      <div class="infobar error" role="alert">
        <span class="glyph" aria-hidden="true">{GLYPH.error}</span>
        <p>
          Window controls failed to start. Restart Glide, and check its log if
          this keeps happening.
        </p>
      </div>
    {/if}

    {#if activeSection === 'gestures'}
      <h1>Gestures</h1>
      <p class="lede">
        Hold the keys, then use the mouse anywhere on a window. Try a
        combination here: the keys light up as you hold them.
      </p>

      {#if conflict}
        <div class="infobar warning" role="alert">
          <span class="glyph" aria-hidden="true">{GLYPH.warning}</span>
          <p>{conflict}</p>
        </div>
      {/if}

      <section class="card gesture" class:off={!config.move_enabled}>
        <header class="card-head">
          <span class="card-icon glyph" aria-hidden="true">{GLYPH.move}</span>
          <div class="card-title">
            <h2>Move windows</h2>
            <p class="recipe" class:ready={moveReady}>
              {#if moveReady}
                Keys held. Now {config.move_trigger === 'pan'
                  ? 'move the mouse'
                  : 'click and drag'}.
              {:else}
                Hold {@render keyChips(config.move_modifiers)}
                {recipe(config.move_trigger, 'left')}
              {/if}
            </p>
          </div>
          <Switch.Root
            class="toggle"
            bind:checked={config.move_enabled}
            aria-label="Move windows"
          >
            <Switch.Thumb class="thumb" />
          </Switch.Root>
        </header>
        <div class="card-body" inert={!config.move_enabled}>
          <div class="field">
            <span class="field-label">Keys <em>pick 1 to 3</em></span>
            <Keycaps
              bind:value={config.move_modifiers}
              held={heldKeys}
              label="Keys for moving windows"
            />
          </div>
          <div class="field">
            <span class="field-label">Mouse</span>
            {@render triggerPicker(
              config.move_trigger,
              (v) => (config.move_trigger = v),
              'left',
              !config.move_enabled,
              'Mouse action for moving windows'
            )}
          </div>
          {#if panNote(config.move_modifiers, config.move_trigger)}
            <p class="note">
              {panNote(config.move_modifiers, config.move_trigger)}
            </p>
          {/if}
        </div>
      </section>

      <section class="card gesture" class:off={!config.resize_enabled}>
        <header class="card-head">
          <span class="card-icon glyph" aria-hidden="true">{GLYPH.resize}</span>
          <div class="card-title">
            <h2>Resize windows</h2>
            <p class="recipe" class:ready={resizeReady}>
              {#if resizeReady}
                Keys held. Now {config.resize_trigger === 'pan'
                  ? 'move the mouse'
                  : 'right-click and drag'}.
              {:else}
                Hold {@render keyChips(config.resize_modifiers)}
                {recipe(config.resize_trigger, 'right')}
              {/if}
            </p>
          </div>
          <Switch.Root
            class="toggle"
            bind:checked={config.resize_enabled}
            aria-label="Resize windows"
          >
            <Switch.Thumb class="thumb" />
          </Switch.Root>
        </header>
        <div class="card-body" inert={!config.resize_enabled}>
          <div class="field">
            <span class="field-label">Keys <em>pick 1 to 3</em></span>
            <Keycaps
              bind:value={config.resize_modifiers}
              held={heldKeys}
              label="Keys for resizing windows"
            />
          </div>
          <div class="field">
            <span class="field-label">Mouse</span>
            {@render triggerPicker(
              config.resize_trigger,
              (v) => (config.resize_trigger = v),
              'right',
              !config.resize_enabled,
              'Mouse action for resizing windows'
            )}
          </div>
          {#if panNote(config.resize_modifiers, config.resize_trigger)}
            <p class="note">
              {panNote(config.resize_modifiers, config.resize_trigger)}
            </p>
          {/if}
          <div class="row nested">
            <div class="row-text">
              <span class="row-label">Resize from the nearest corner</span>
              <span class="row-desc"
                >Off: the bottom-right corner always follows the mouse.</span
              >
            </div>
            <Switch.Root
              class="toggle"
              checked={config.resize_mode === 'quadrant'}
              onCheckedChange={(v) =>
                (config.resize_mode = v ? 'quadrant' : 'absolute')}
              aria-label="Resize from the nearest corner"
            >
              <Switch.Thumb class="thumb" />
            </Switch.Root>
          </div>
        </div>
      </section>

      <section class="card gesture" class:off={!config.scroll_opacity}>
        <header class="card-head">
          <span class="card-icon glyph" aria-hidden="true">{GLYPH.opacity}</span
          >
          <div class="card-title">
            <h2>Change transparency</h2>
            <p class="recipe">
              Hold <kbd>{modifierLabel(config.scroll_opacity_modifier)}</kbd>
              then scroll the wheel
            </p>
          </div>
          <Switch.Root
            class="toggle"
            bind:checked={config.scroll_opacity}
            aria-label="Change transparency"
          >
            <Switch.Thumb class="thumb" />
          </Switch.Root>
        </header>
        <div class="card-body" inert={!config.scroll_opacity}>
          <div class="field">
            <span class="field-label">Key</span>
            <Keycaps
              single
              bind:value={
                () => [config.scroll_opacity_modifier],
                (keys) => (config.scroll_opacity_modifier = keys[0])
              }
              held={heldKeys}
              label="Key for changing transparency"
            />
          </div>
        </div>
      </section>
    {/if}

    {#if activeSection === 'behavior'}
      <h1>Behavior</h1>

      <h2 class="group-title">Dragging</h2>
      <section class="card list">
        <div class="row">
          <div class="row-text">
            <span class="row-label">Start distance</span>
            <span class="row-desc"
              >How far the mouse travels before a window starts to follow. Raise
              it if “Just move” feels too eager.</span
            >
          </div>
          <div class="slider-group">
            <input
              type="range"
              class="slider"
              min="0"
              max="200"
              bind:value={config.drag_threshold}
              aria-label="Start distance in pixels"
            />
            <output class="slider-value">{config.drag_threshold} px</output>
          </div>
        </div>
        <div class="row">
          <div class="row-text">
            <span class="row-label">Work on background windows</span>
            <span class="row-desc"
              >Move or resize a window without clicking it first.</span
            >
          </div>
          <Switch.Root
            class="toggle"
            bind:checked={config.allow_nonforeground}
            aria-label="Work on background windows"
          >
            <Switch.Thumb class="thumb" />
          </Switch.Root>
        </div>
        <div class="row">
          <div class="row-text">
            <span class="row-label">Bring to front when grabbed</span>
            <span class="row-desc"
              >Raise the window above others as soon as it starts moving.</span
            >
          </div>
          <Switch.Root
            class="toggle"
            bind:checked={config.raise_on_grab}
            aria-label="Bring to front when grabbed"
          >
            <Switch.Thumb class="thumb" />
          </Switch.Root>
        </div>
      </section>

      <h2 class="group-title">Snapping</h2>
      <section class="card list">
        <div class="row">
          <div class="row-text">
            <span class="row-label">Snap to screen edges</span>
            <span class="row-desc"
              >Drop a window at an edge or corner to fill that part of the
              screen.</span
            >
          </div>
          <Switch.Root
            class="toggle"
            bind:checked={config.snap_enabled}
            aria-label="Snap to screen edges"
          >
            <Switch.Thumb class="thumb" />
          </Switch.Root>
        </div>
        <div class="row" class:off={!config.snap_enabled}>
          <div class="row-text">
            <span class="row-label">Use Windows snap groups</span>
            <span class="row-desc"
              >Let go of the keys before the mouse to join a Windows 11 snap
              group.</span
            >
          </div>
          <Switch.Root
            class="toggle"
            bind:checked={config.snap_native}
            disabled={!config.snap_enabled}
            aria-label="Use Windows snap groups"
          >
            <Switch.Thumb class="thumb" />
          </Switch.Root>
        </div>
      </section>

      <h2 class="group-title">Startup</h2>
      <section class="card list">
        <div class="row">
          <div class="row-text">
            <span class="row-label">Start with Windows</span>
            <span class="row-desc">Glide waits in the system tray.</span>
          </div>
          <Switch.Root
            class="toggle"
            checked={autostartEnabled}
            disabled={changingAutostart}
            onCheckedChange={toggleAutostart}
            aria-label="Start with Windows"
          >
            <Switch.Thumb class="thumb" />
          </Switch.Root>
        </div>
        {#if autostartError}
          <p class="error-text row-error">{autostartError}</p>
        {/if}
      </section>
    {/if}

    {#if activeSection === 'apps'}
      <h1>Apps</h1>
      <p class="lede">Choose where Glide's gestures work.</p>

      <section class="card">
        <RadioGroup.Root
          class="segmented wide"
          bind:value={config.filter_mode}
          aria-label="Where Glide works"
        >
          <RadioGroup.Item class="segment" value="blacklist"
            >Everywhere except…</RadioGroup.Item
          >
          <RadioGroup.Item class="segment" value="whitelist"
            >Only in…</RadioGroup.Item
          >
        </RadioGroup.Root>
        <p class="row-desc filter-hint">{filterHint}</p>

        <div class="picker-row">
          <Select.Root
            type="single"
            bind:value={runningProcessValue}
            items={selectableRunningProcessItems}
            onValueChange={onRunningProcessSelect}
          >
            <Select.Trigger
              class="select-trigger"
              aria-label="Add a running app"
            >
              <span class="glyph" aria-hidden="true">{GLYPH.add}</span>
              <span class="select-value">Add a running app</span>
              <span class="glyph caret" aria-hidden="true">{GLYPH.chevron}</span
              >
            </Select.Trigger>
            <Select.Content class="select-content" sideOffset={4}>
              {#if selectableRunningProcesses.length === 0}
                <div class="select-empty">No other apps are running</div>
              {:else}
                {#each selectableRunningProcesses as proc (proc)}
                  <Select.Item class="select-item" value={proc} label={proc}
                    >{proc}</Select.Item
                  >
                {/each}
              {/if}
            </Select.Content>
          </Select.Root>
          <button
            type="button"
            class="button icon-button"
            onclick={loadRunning}
            aria-label="Refresh running apps"
            title="Refresh running apps"
          >
            <span class="glyph" aria-hidden="true">{GLYPH.refresh}</span>
          </button>
        </div>

        {#if config.filter_list.length > 0}
          <ul class="app-list">
            {#each config.filter_list as proc (proc)}
              <li>
                <span class="glyph app-glyph" aria-hidden="true"
                  >{GLYPH.app}</span
                >
                <span class="app-name">{proc}</span>
                <button
                  type="button"
                  class="icon-button subtle"
                  onclick={() => removeProcess(proc)}
                  aria-label="Remove {proc}"
                  title="Remove"
                >
                  <span class="glyph" aria-hidden="true">{GLYPH.remove}</span>
                </button>
              </li>
            {/each}
          </ul>
        {:else if config.filter_mode === 'whitelist'}
          <div class="infobar warning">
            <span class="glyph" aria-hidden="true">{GLYPH.warning}</span>
            <p>No apps are listed, so Glide works nowhere. Add an app above.</p>
          </div>
        {:else}
          <p class="empty">No apps excluded. Glide works in every window.</p>
        {/if}
      </section>
    {/if}

    {#if activeSection === 'about'}
      <h1>About</h1>

      <section class="card about">
        <img class="about-mark" src="/glide-mark.png" alt="" />
        <div>
          <h2>Glide</h2>
          <p class="row-desc">
            Move and resize any window with a few keys and the mouse.
          </p>
        </div>
        <span class="version">{appVersion ? `v${appVersion}` : ''}</span>
      </section>

      <section class="card list">
        <div class="row">
          <div class="row-text">
            <span class="row-label">Made by Chanhyeok Son</span>
            <span class="row-desc">Glide is free and open source.</span>
          </div>
          <div class="row-actions">
            <button
              type="button"
              class="button"
              onclick={() => openLink('author')}
            >
              GitHub
              <span class="glyph small" aria-hidden="true"
                >{GLYPH.external}</span
              >
            </button>
            <button
              type="button"
              class="button"
              onclick={() => openLink('repo')}
            >
              Source code
              <span class="glyph small" aria-hidden="true"
                >{GLYPH.external}</span
              >
            </button>
          </div>
        </div>
      </section>

      <h2 class="group-title">Support</h2>
      {#if supporter}
        <section class="card thanks">
          <span class="thanks-heart glyph" aria-hidden="true"
            >{GLYPH.heart}</span
          >
          <div>
            <h3>Thank you, {supporter}</h3>
            <p class="row-desc">
              You support Glide's development. It keeps the project going.
            </p>
          </div>
        </section>
      {:else}
        <section class="card list">
          <div class="row">
            <div class="row-text">
              <span class="row-label">Donate</span>
              <span class="row-desc"
                >If Glide saves you time, a small donation helps.</span
              >
            </div>
            <div class="row-actions">
              <button
                type="button"
                class="button accent"
                onclick={() => openLink('sponsors')}
              >
                <span class="glyph" aria-hidden="true">{GLYPH.heart}</span>
                GitHub Sponsors
              </button>
              <button
                type="button"
                class="button"
                onclick={() => openLink('coffee')}
              >
                Buy me a coffee
              </button>
            </div>
          </div>
          {#if !storeBuild}
            <form class="row code-row" onsubmit={redeemCode}>
              <div class="row-text">
                <label class="row-label" for="supporter-code"
                  >Supporter code</label
                >
                <span class="row-desc"
                  >Supporters get a code from the developer. Enter it to show
                  your supporter badge.</span
                >
              </div>
              <div class="code-field">
                <input
                  id="supporter-code"
                  class="text-input"
                  bind:this={codeInput}
                  bind:value={supportCode}
                  placeholder="GLIDE1-…"
                  spellcheck="false"
                  autocomplete="off"
                  aria-invalid={redeemError ? 'true' : undefined}
                  aria-describedby={redeemError ? 'code-error' : undefined}
                />
                <button
                  type="submit"
                  class="button"
                  disabled={redeeming || !supportCode.trim()}
                >
                  {redeeming ? 'Checking…' : 'Redeem'}
                </button>
              </div>
            </form>
            {#if redeemError}
              <p id="code-error" class="error-text row-error" role="alert">
                {redeemError}
              </p>
            {/if}
          {/if}
        </section>
      {/if}
      {#if linkError}
        <p class="error-text link-error" role="alert">{linkError}</p>
      {/if}

      <h2 class="group-title">Updates</h2>
      <section class="card list">
        <div class="row">
          <div class="row-text">
            {#if storeBuild}
              <span class="row-label">Managed by Microsoft Store</span>
              <span class="row-desc">The Store keeps Glide up to date.</span>
            {:else if checkingUpdate}
              <span class="row-label">Checking for updates…</span>
            {:else if updateAvailable !== null}
              <span class="row-label"
                >Version {updateAvailable.version} is available</span
              >
              <span class="row-desc">Glide restarts after installing.</span>
            {:else if updateCheckFailed}
              <span class="row-label">Couldn't check for updates</span>
              <span class="row-desc">Check your internet connection.</span>
            {:else}
              <span class="row-label">You're up to date</span>
            {/if}
          </div>
          {#if !storeBuild && updateAvailable !== null}
            <button
              class="button accent"
              onclick={installUpdate}
              disabled={isUpdating ||
                saving ||
                hasUnsavedChanges ||
                changingAutostart}
            >
              {isUpdating ? 'Installing…' : 'Install and restart'}
            </button>
          {:else if !storeBuild && updateCheckFailed}
            <button class="button" onclick={checkForUpdate}>Try again</button>
          {/if}
        </div>
        {#if updateError}
          <p class="error-text row-error">{updateError}</p>
        {/if}
      </section>
    {/if}
  </div>

  <SupportDialog
    open={supportPrompt && loaded}
    onDonate={donateFromPrompt}
    onLater={() => answerPrompt('later')}
    onHaveCode={storeBuild ? undefined : enterCodeFromPrompt}
  />
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
    width: 100%;
    height: 100%;
    overflow: hidden;
    background: transparent;
    color-scheme: light dark;
    -webkit-font-smoothing: antialiased;
  }

  :global(:root) {
    /* Fluent 2, light theme over Mica */
    --text: rgb(0 0 0 / 0.896);
    --text-secondary: rgb(0 0 0 / 0.62);
    --text-disabled: rgb(0 0 0 / 0.36);
    --card: rgb(255 255 255 / 0.7);
    --card-hover: rgb(249 249 249 / 0.6);
    --card-stroke: rgb(0 0 0 / 0.0578);
    --card-shadow: 0 1px 2px rgb(0 0 0 / 0.05);
    --divider: rgb(0 0 0 / 0.0803);
    --subtle-hover: rgb(0 0 0 / 0.0373);
    --subtle-press: rgb(0 0 0 / 0.0241);
    --control: rgb(255 255 255 / 0.7);
    --control-hover: rgb(249 249 249 / 0.5);
    --control-stroke: rgb(0 0 0 / 0.0578);
    --control-stroke-bottom: rgb(0 0 0 / 0.1622);
    --accent: #005fb8;
    --accent-hover: #196ebf;
    --accent-deep: #003e92;
    --on-accent: #ffffff;
    --focus: rgb(0 0 0 / 0.896);
    --key-bg: #fbfbfb;
    --key-hover: #f3f3f3;
    --key-stroke: rgb(0 0 0 / 0.16);
    --held-ring: rgb(0 95 184 / 0.45);
    --switch-off-stroke: rgb(0 0 0 / 0.6);
    --caution-bg: rgb(255 244 206 / 0.9);
    --caution-fg: #9d5d00;
    --critical-bg: rgb(253 231 233 / 0.9);
    --critical-fg: #c42b1c;
    --success: #0f7b0f;
    --scroll-thumb: rgb(0 0 0 / 0.35);
    --surface-stroke: rgb(117 117 117 / 0.4);
    --dialog: #ffffff;
    --dialog-footer: #f3f3f3;
    --smoke: rgb(0 0 0 / 0.3);
    --input: rgb(255 255 255 / 0.7);
    --input-focus: #ffffff;
  }

  @media (prefers-color-scheme: dark) {
    :global(:root) {
      --text: #ffffff;
      --text-secondary: rgb(255 255 255 / 0.786);
      --text-disabled: rgb(255 255 255 / 0.3628);
      --card: rgb(255 255 255 / 0.0512);
      --card-hover: rgb(255 255 255 / 0.0837);
      --card-stroke: rgb(0 0 0 / 0.1);
      --card-shadow: 0 1px 2px rgb(0 0 0 / 0.2);
      --divider: rgb(255 255 255 / 0.0837);
      --subtle-hover: rgb(255 255 255 / 0.0605);
      --subtle-press: rgb(255 255 255 / 0.0419);
      --control: rgb(255 255 255 / 0.0605);
      --control-hover: rgb(255 255 255 / 0.0837);
      --control-stroke: rgb(255 255 255 / 0.0698);
      --control-stroke-bottom: rgb(255 255 255 / 0.0465);
      --accent: #60cdff;
      --accent-hover: #5bbae8;
      --accent-deep: #3aa0d4;
      --on-accent: #000000;
      --focus: #ffffff;
      --key-bg: #3a3a3a;
      --key-hover: #444444;
      --key-stroke: rgb(0 0 0 / 0.55);
      --held-ring: rgb(96 205 255 / 0.5);
      --switch-off-stroke: rgb(255 255 255 / 0.786);
      --caution-bg: rgb(67 53 25 / 0.85);
      --caution-fg: #fce100;
      --critical-bg: rgb(68 39 38 / 0.85);
      --critical-fg: #ff99a4;
      --success: #6ccb5f;
      --scroll-thumb: rgb(255 255 255 / 0.35);
      --surface-stroke: rgb(117 117 117 / 0.4);
      --dialog: #2b2b2b;
      --dialog-footer: #202020;
      --smoke: rgb(0 0 0 / 0.3);
      --input: rgb(255 255 255 / 0.0605);
      --input-focus: rgb(30 30 30 / 0.7);
    }
  }

  :global(::selection) {
    background: var(--accent);
    color: var(--on-accent);
  }

  main {
    display: grid;
    grid-template-columns: 208px minmax(0, 1fr);
    height: 100vh;
    color: var(--text);
    font-family: 'Segoe UI Variable Text', 'Segoe UI', sans-serif;
    font-size: 14px;
    line-height: 20px;
    opacity: 0;
    transition: opacity 160ms cubic-bezier(0.1, 0.9, 0.2, 1);
  }

  main.loaded {
    opacity: 1;
  }

  :global(.glyph) {
    font-family: 'Segoe Fluent Icons', 'Segoe MDL2 Assets';
    font-style: normal;
    font-size: 16px;
    line-height: 1;
  }

  /* ---------- Navigation pane ---------- */

  .nav-pane {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 12px 8px 14px;
    min-height: 0;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 6px 10px 18px;
  }

  .brand-mark {
    width: 36px;
    height: 36px;
    border-radius: 8px;
  }

  .brand-text {
    display: flex;
    flex-direction: column;
  }

  .brand-name {
    font-family: 'Segoe UI Variable Display', 'Segoe UI', sans-serif;
    font-size: 16px;
    font-weight: 600;
    line-height: 20px;
  }

  .brand-sub {
    color: var(--text-secondary);
    font-size: 12px;
    line-height: 16px;
  }

  nav {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .nav-item {
    position: relative;
    display: flex;
    align-items: center;
    gap: 14px;
    height: 36px;
    padding: 0 12px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--text);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .nav-item:hover {
    background: var(--subtle-hover);
  }

  .nav-item:active {
    background: var(--subtle-press);
    color: var(--text-secondary);
  }

  .nav-item.active {
    background: var(--subtle-hover);
    font-weight: 600;
  }

  .nav-item.active::before {
    content: '';
    position: absolute;
    left: 0;
    top: 50%;
    width: 3px;
    height: 16px;
    border-radius: 2px;
    background: var(--accent);
    transform: translateY(-50%);
    animation: pill 220ms cubic-bezier(0.1, 0.9, 0.2, 1);
  }

  @keyframes pill {
    from {
      height: 4px;
    }
  }

  .save-state {
    margin-top: auto;
    padding: 0 12px;
    display: grid;
    gap: 8px;
    justify-items: start;
  }

  .caption {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text-secondary);
    font-size: 12px;
    line-height: 16px;
  }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--success);
    transition: background 160ms ease;
  }

  .dot.busy {
    background: var(--accent);
  }

  /* ---------- Page ---------- */

  .page {
    min-height: 0;
    overflow-y: auto;
    padding: 22px 24px 28px 12px;
    scrollbar-gutter: stable;
  }

  .page::-webkit-scrollbar {
    width: 10px;
  }

  .page::-webkit-scrollbar-thumb {
    border: 3px solid transparent;
    border-radius: 6px;
    background: var(--scroll-thumb);
    background-clip: content-box;
  }

  h1 {
    font-family: 'Segoe UI Variable Display', 'Segoe UI', sans-serif;
    font-size: 28px;
    font-weight: 600;
    line-height: 36px;
    margin-bottom: 6px;
  }

  .lede {
    color: var(--text-secondary);
    max-width: 52ch;
    margin-bottom: 18px;
  }

  h1 + .card,
  h1 + .group-title {
    margin-top: 14px;
  }

  .group-title {
    font-size: 14px;
    font-weight: 600;
    line-height: 20px;
    margin: 22px 0 8px 2px;
  }

  .card {
    border: 1px solid var(--card-stroke);
    border-radius: 8px;
    background: var(--card);
    box-shadow: var(--card-shadow);
    padding: 16px;
  }

  .card + .card {
    margin-top: 8px;
  }

  /* ---------- Gesture cards ---------- */

  .card-head {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 14px;
  }

  .card-icon {
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    border-radius: 6px;
    background: var(--accent);
    color: var(--on-accent);
    font-size: 18px;
    transition:
      background 160ms ease,
      color 160ms ease;
  }

  .gesture.off .card-icon {
    background: var(--subtle-hover);
    color: var(--text-secondary);
  }

  h2 {
    font-size: 14px;
    font-weight: 600;
    line-height: 20px;
  }

  .recipe {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 3px 4px;
    margin-top: 2px;
    color: var(--text-secondary);
    font-size: 12px;
    line-height: 20px;
  }

  .recipe.ready {
    color: var(--accent);
    font-weight: 600;
  }

  .gesture.off .recipe {
    color: var(--text-disabled);
  }

  kbd {
    display: inline-flex;
    align-items: center;
    height: 20px;
    padding: 0 6px;
    border: 1px solid var(--key-stroke);
    border-bottom-width: 2px;
    border-radius: 4px;
    background: var(--key-bg);
    color: var(--text);
    font: inherit;
    font-size: 11.5px;
    font-weight: 600;
  }

  .plus {
    color: var(--text-secondary);
  }

  .card-body {
    display: grid;
    gap: 14px;
    margin-top: 16px;
    padding-top: 16px;
    border-top: 1px solid var(--divider);
    transition: opacity 160ms ease;
  }

  .gesture.off .card-body {
    opacity: 0.45;
  }

  .field {
    display: grid;
    gap: 8px;
  }

  .field-label {
    font-size: 12px;
    font-weight: 600;
    line-height: 16px;
  }

  .field-label em {
    margin-left: 6px;
    color: var(--text-secondary);
    font-style: normal;
    font-weight: 400;
  }

  .note {
    color: var(--caution-fg);
    font-size: 12px;
    line-height: 16px;
  }

  /* ---------- Segmented control ---------- */

  :global(.segmented) {
    display: grid;
    grid-auto-columns: minmax(0, 1fr);
    grid-auto-flow: column;
    gap: 2px;
    padding: 3px;
    border: 1px solid var(--control-stroke);
    border-radius: 6px;
    background: var(--subtle-press);
  }

  :global(.segment) {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    height: 32px;
    padding: 0 10px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--text-secondary);
    font: inherit;
    font-size: 13px;
    cursor: pointer;
    transition:
      background 120ms ease,
      color 120ms ease;
  }

  :global(.segment:hover) {
    background: var(--subtle-hover);
    color: var(--text);
  }

  :global(.segment[data-state='checked']) {
    background: var(--control);
    color: var(--text);
    font-weight: 600;
    box-shadow:
      0 0 0 1px var(--control-stroke),
      0 1px 2px rgb(0 0 0 / 0.08);
  }

  :global(.segment:focus-visible) {
    outline: 2px solid var(--focus);
    outline-offset: 1px;
  }

  /* ---------- Rows ---------- */

  .list {
    padding: 0;
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    min-height: 64px;
    padding: 12px 16px;
  }

  .list .row + .row {
    border-top: 1px solid var(--divider);
  }

  .row.nested {
    min-height: 0;
    padding: 12px 0 0;
    border-top: 1px solid var(--divider);
  }

  .row.off {
    opacity: 0.45;
  }

  .row-text {
    display: grid;
    gap: 2px;
    min-width: 0;
  }

  .row-label {
    line-height: 20px;
  }

  .row-desc {
    color: var(--text-secondary);
    font-size: 12px;
    line-height: 16px;
  }

  .row-error {
    padding: 0 16px 12px;
  }

  /* ---------- Toggle switch (Fluent) ---------- */

  :global(.toggle) {
    position: relative;
    flex: none;
    width: 40px;
    height: 20px;
    border: 1px solid var(--switch-off-stroke);
    border-radius: 10px;
    background: transparent;
    cursor: pointer;
    transition:
      background 120ms ease,
      border-color 120ms ease;
  }

  :global(.toggle:hover) {
    background: var(--subtle-hover);
  }

  :global(.toggle[data-state='checked']) {
    border-color: var(--accent);
    background: var(--accent);
  }

  :global(.toggle[data-state='checked']:hover) {
    background: var(--accent-hover);
  }

  :global(.toggle:disabled) {
    opacity: 0.4;
    cursor: default;
  }

  :global(.toggle:focus-visible) {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }

  :global(.thumb) {
    position: absolute;
    top: 50%;
    left: 4px;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--switch-off-stroke);
    transform: translateY(-50%);
    transition:
      left 160ms cubic-bezier(0.1, 0.9, 0.2, 1),
      width 120ms ease,
      height 120ms ease;
  }

  :global(.toggle:hover .thumb) {
    width: 14px;
    height: 14px;
  }

  :global(.thumb[data-state='checked']) {
    left: 22px;
    background: var(--on-accent);
  }

  :global(.toggle:hover .thumb[data-state='checked']) {
    left: 21px;
  }

  /* ---------- Slider ---------- */

  .slider-group {
    display: flex;
    align-items: center;
    gap: 10px;
    flex: none;
  }

  .slider {
    width: 120px;
    accent-color: var(--accent);
  }

  .slider-value {
    min-width: 46px;
    color: var(--text-secondary);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    text-align: right;
  }

  /* ---------- Buttons, infobars ---------- */

  :global(.button) {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    height: 32px;
    padding: 0 12px;
    border: 1px solid var(--control-stroke);
    border-bottom-color: var(--control-stroke-bottom);
    border-radius: 4px;
    background: var(--control);
    color: var(--text);
    font: inherit;
    cursor: pointer;
    white-space: nowrap;
  }

  :global(.button:hover:not(:disabled)) {
    background: var(--control-hover);
  }

  :global(.button:disabled) {
    color: var(--text-disabled);
    cursor: default;
  }

  :global(.button.accent) {
    border-color: var(--accent-deep);
    background: var(--accent);
    color: var(--on-accent);
  }

  :global(.button.accent:hover:not(:disabled)) {
    background: var(--accent-hover);
  }

  :global(.button:focus-visible),
  .icon-button:focus-visible,
  .nav-item:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 1px;
  }

  .icon-button {
    width: 32px;
    padding: 0;
  }

  .icon-button.subtle {
    display: grid;
    place-items: center;
    height: 28px;
    width: 28px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--text-secondary);
    cursor: pointer;
  }

  .icon-button.subtle:hover {
    background: var(--subtle-hover);
    color: var(--text);
  }

  .icon-button.subtle .glyph {
    font-size: 12px;
  }

  .infobar {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    margin-bottom: 12px;
    padding: 12px 14px;
    border: 1px solid var(--card-stroke);
    border-radius: 6px;
    font-size: 13px;
    line-height: 18px;
  }

  .infobar .glyph {
    margin-top: 1px;
  }

  .infobar.warning {
    background: var(--caution-bg);
  }

  .infobar.warning .glyph {
    color: var(--caution-fg);
  }

  .infobar.error {
    background: var(--critical-bg);
  }

  .infobar.error .glyph {
    color: var(--critical-fg);
  }

  .card .infobar {
    margin: 12px 0 0;
  }

  :global(.link-button) {
    padding: 4px 2px;
    border: 0;
    background: none;
    color: var(--accent);
    font: inherit;
    cursor: pointer;
  }

  :global(.link-button:hover) {
    text-decoration: underline;
    text-underline-offset: 3px;
  }

  :global(.link-button:focus-visible) {
    outline: 2px solid var(--focus);
    outline-offset: 1px;
    border-radius: 2px;
  }

  .error-text {
    color: var(--critical-fg);
    font-size: 12px;
    line-height: 16px;
  }

  /* ---------- Apps ---------- */

  :global(.segmented.wide) {
    margin-bottom: 8px;
  }

  .filter-hint {
    margin: 0 2px 14px;
  }

  .picker-row {
    display: flex;
    gap: 8px;
  }

  :global(.select-trigger) {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 10px;
    height: 32px;
    padding: 0 10px;
    border: 1px solid var(--control-stroke);
    border-bottom-color: var(--control-stroke-bottom);
    border-radius: 4px;
    background: var(--control);
    color: var(--text);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  :global(.select-trigger:hover) {
    background: var(--control-hover);
  }

  :global(.select-trigger:focus-visible) {
    outline: 2px solid var(--focus);
    outline-offset: 1px;
  }

  :global(.select-trigger .glyph) {
    font-size: 12px;
  }

  :global(.select-value) {
    flex: 1;
  }

  :global(.select-content) {
    z-index: 20;
    width: var(--bits-select-anchor-width);
    max-height: 240px;
    overflow-y: auto;
    padding: 4px;
    border: 1px solid var(--card-stroke);
    border-radius: 8px;
    background: Canvas;
    box-shadow: 0 8px 16px rgb(0 0 0 / 0.14);
    font-family: 'Segoe UI Variable Text', 'Segoe UI', sans-serif;
    font-size: 14px;
  }

  :global(.select-item) {
    padding: 7px 10px;
    border-radius: 4px;
    color: var(--text);
    cursor: pointer;
  }

  :global(.select-item[data-highlighted]) {
    background: var(--subtle-hover);
  }

  :global(.select-empty) {
    padding: 8px 10px;
    color: var(--text-secondary);
  }

  .app-list {
    list-style: none;
    margin-top: 12px;
    border: 1px solid var(--divider);
    border-radius: 6px;
  }

  .app-list li {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 6px 6px 6px 12px;
  }

  .app-list li + li {
    border-top: 1px solid var(--divider);
  }

  .app-glyph {
    color: var(--text-secondary);
    font-size: 14px;
  }

  .app-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .empty {
    margin-top: 14px;
    color: var(--text-secondary);
    font-size: 13px;
  }

  /* ---------- About ---------- */

  .about {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 16px;
  }

  .about-mark {
    width: 48px;
    height: 48px;
    border-radius: 10px;
  }

  .version {
    color: var(--text-secondary);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }

  .glyph.small {
    font-size: 11px;
  }

  .row-actions {
    display: flex;
    flex: none;
    gap: 8px;
  }

  .code-row {
    flex-wrap: wrap;
  }

  .code-field {
    display: flex;
    gap: 8px;
    width: 100%;
  }

  .text-input {
    flex: 1;
    min-width: 0;
    height: 32px;
    padding: 0 10px;
    border: 1px solid var(--control-stroke);
    border-bottom-color: var(--control-stroke-bottom);
    border-radius: 4px;
    background: var(--input);
    color: var(--text);
    font: inherit;
    font-size: 13px;
    caret-color: var(--accent);
  }

  .text-input::placeholder {
    color: var(--text-secondary);
  }

  .text-input:focus {
    outline: none;
    background: var(--input-focus);
    border-bottom: 2px solid var(--accent);
  }

  .text-input[aria-invalid='true'] {
    border-bottom: 2px solid var(--critical-fg);
  }

  .thanks {
    display: flex;
    align-items: center;
    gap: 16px;
  }

  .thanks-heart {
    display: grid;
    place-items: center;
    flex: none;
    width: 40px;
    height: 40px;
    border-radius: 50%;
    background: var(--accent);
    color: var(--on-accent);
    font-size: 18px;
  }

  .thanks h3 {
    font-size: 14px;
    font-weight: 600;
  }

  .supporter-badge {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: var(--accent);
  }

  .supporter-badge .glyph {
    font-size: 10px;
  }

  .link-error {
    margin-top: 8px;
  }

  @media (prefers-reduced-motion: reduce) {
    main,
    .nav-item.active::before,
    :global(.thumb) {
      transition: none;
      animation: none;
    }
  }
</style>
