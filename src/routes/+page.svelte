<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";
  import type { AppConfig } from "$lib/config";
  import { MODIFIER_OPTIONS, DEFAULT_CONFIG } from "$lib/config";
  import { saveConfig } from "$lib/store";

  let config = $state<AppConfig>({ ...DEFAULT_CONFIG });
  let newProcess = $state("");
  let runningProcesses = $state<string[]>([]);
  let showSuggestions = $state(false);
  let saveStatus = $state<"idle" | "saving" | "saved" | "error">("idle");
  let autostartEnabled = $state(false);
  let loaded = $state(false);

  onMount(async () => {
    try {
      const loaded_cfg = await invoke<AppConfig>("get_config");
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
    loaded = true;
  });

  const moveLabel = $derived(
    MODIFIER_OPTIONS.find((o) => o.value === config.move_modifier)?.label ?? config.move_modifier
  );
  const resizeLabel1 = $derived(
    MODIFIER_OPTIONS.find((o) => o.value === config.resize_modifier_1)?.label ?? config.resize_modifier_1
  );
  const resizeLabel2 = $derived(
    MODIFIER_OPTIONS.find((o) => o.value === config.resize_modifier_2)?.label ?? config.resize_modifier_2
  );
  const filterHint = $derived(
    config.filter_mode === "whitelist"
      ? "Only affects listed processes"
      : "Affects all processes except listed"
  );

  async function toggleEnabled() {
    config.enabled = !config.enabled;
    try {
      await invoke("set_hook_enabled", { enabled: config.enabled });
    } catch {}
  }

  async function toggleAutostart() {
    autostartEnabled = !autostartEnabled;
    config.autostart = autostartEnabled;
    try {
      if (autostartEnabled) {
        await enable();
      } else {
        await disable();
      }
    } catch {}
  }

  function addProcess() {
    const name = newProcess.trim();
    if (name && !config.filter_list.includes(name)) {
      config.filter_list = [...config.filter_list, name];
    }
    newProcess = "";
    showSuggestions = false;
  }

  function removeProcess(name: string) {
    config.filter_list = config.filter_list.filter((p) => p !== name);
  }

  function addFromSuggestion(name: string) {
    if (!config.filter_list.includes(name)) {
      config.filter_list = [...config.filter_list, name];
    }
    showSuggestions = false;
    newProcess = "";
  }

  async function loadRunning() {
    try {
      runningProcesses = await invoke<string[]>("get_running_processes");
      showSuggestions = runningProcesses.length > 0;
    } catch {
      runningProcesses = [];
    }
  }

  async function save() {
    saveStatus = "saving";
    try {
      await invoke("set_config", { config });
      await saveConfig(config);
      saveStatus = "saved";
      setTimeout(() => {
        saveStatus = "idle";
      }, 2000);
    } catch {
      saveStatus = "error";
      setTimeout(() => {
        saveStatus = "idle";
      }, 3000);
    }
  }

  function onProcessKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") addProcess();
    if (e.key === "Escape") showSuggestions = false;
  }
</script>

<main class:loaded>
  <!-- Header -->
  <header>
    <div class="brand">
      <span class="brand-name">wkgrip</span>
      <span class="brand-ver">v0.1.0</span>
    </div>
    <div class="master-wrap">
      <span class="status-label" class:active={config.enabled}>
        {config.enabled ? "Active" : "Paused"}
      </span>
      <button
        type="button"
        class="toggle"
        class:on={config.enabled}
        onclick={toggleEnabled}
        aria-label="Master enable/disable"
      >
        <span class="thumb"></span>
      </button>
    </div>
  </header>

  <!-- Modifier Keys -->
  <section class="card">
    <h2>Modifier Keys</h2>
    <div class="mod-row">
      <span class="mod-label">Move</span>
      <select bind:value={config.move_modifier}>
        {#each MODIFIER_OPTIONS as opt}
          <option value={opt.value}>{opt.label}</option>
        {/each}
      </select>
      <span class="hint-inline">Hold <kbd>{moveLabel}</kbd> + drag to move</span>
    </div>
    <div class="mod-row">
      <span class="mod-label">Resize</span>
      <div class="resize-pair">
        <select bind:value={config.resize_modifier_1}>
          {#each MODIFIER_OPTIONS as opt}
            <option value={opt.value}>{opt.label}</option>
          {/each}
        </select>
        <span class="plus">+</span>
        <select bind:value={config.resize_modifier_2}>
          {#each MODIFIER_OPTIONS as opt}
            <option value={opt.value}>{opt.label}</option>
          {/each}
        </select>
      </div>
      <span class="hint-inline">Hold <kbd>{resizeLabel1}</kbd>+<kbd>{resizeLabel2}</kbd> + drag</span>
    </div>
  </section>

  <!-- Process Filter -->
  <section class="card">
    <h2>Process Filter</h2>
    <div class="filter-top">
      <div class="radio-tabs">
        <label class="rtab" class:active={config.filter_mode === "whitelist"}>
          <input type="radio" bind:group={config.filter_mode} value="whitelist" />
          Whitelist
        </label>
        <label class="rtab" class:active={config.filter_mode === "blacklist"}>
          <input type="radio" bind:group={config.filter_mode} value="blacklist" />
          Blacklist
        </label>
      </div>
      <span class="filter-hint">{filterHint}</span>
    </div>

    <div class="pill-list">
      {#each config.filter_list as proc}
        <span class="pill">
          {proc}
          <button
            type="button"
            class="pill-x"
            onclick={() => removeProcess(proc)}
            aria-label="Remove {proc}"
          >×</button>
        </span>
      {/each}
      {#if config.filter_list.length === 0}
        <span class="pill-empty">No processes listed</span>
      {/if}
    </div>

    <div class="proc-input-row">
      <input
        type="text"
        class="proc-input"
        bind:value={newProcess}
        onkeydown={onProcessKeydown}
        placeholder="process.exe"
        autocomplete="off"
      />
      <button type="button" class="btn-sm" onclick={addProcess}>Add</button>
      <button type="button" class="btn-sm" onclick={loadRunning}>Load Running</button>
    </div>

    {#if showSuggestions && runningProcesses.length > 0}
      <div class="suggestions">
        <div class="suggestions-header">
          <span>Running processes</span>
          <button type="button" class="sugg-close" onclick={() => (showSuggestions = false)}>×</button>
        </div>
        {#each runningProcesses.slice(0, 5) as proc}
          <button type="button" class="sugg-item" onclick={() => addFromSuggestion(proc)}>
            <span class="sugg-dot"></span>
            {proc}
          </button>
        {/each}
        {#if runningProcesses.length > 5}
          <span class="sugg-more">+{runningProcesses.length - 5} more processes</span>
        {/if}
      </div>
    {/if}
  </section>

  <!-- Autostart -->
  <section class="card card-row">
    <h2>Autostart</h2>
    <div class="row-item">
      <span class="row-label">Start with Windows</span>
      <button
        type="button"
        class="toggle"
        class:on={autostartEnabled}
        onclick={toggleAutostart}
        aria-label="Toggle autostart"
      >
        <span class="thumb"></span>
      </button>
    </div>
  </section>

  <!-- Save -->
  <div class="save-bar">
    <button
      type="button"
      class="save-btn"
      class:saving={saveStatus === "saving"}
      class:saved={saveStatus === "saved"}
      class:error={saveStatus === "error"}
      onclick={save}
      disabled={saveStatus === "saving"}
    >
      {#if saveStatus === "saving"}
        <span class="spinner"></span> Saving…
      {:else if saveStatus === "saved"}
        ✓ Saved
      {:else if saveStatus === "error"}
        ✗ Failed — Retry
      {:else}
        Save Settings
      {/if}
    </button>
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
    width: 540px;
    height: 680px;
    overflow: hidden;
    background: #1a1a2e;
    -webkit-font-smoothing: antialiased;
  }

  /* ─── Layout ─────────────────────────────────────────── */
  main {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 14px 16px 14px;
    height: 680px;
    overflow: hidden;
    font-family: "Segoe UI", system-ui, -apple-system, sans-serif;
    font-size: 13px;
    color: #eee;
    background: #1a1a2e;
    opacity: 0;
    transition: opacity 0.2s ease;
  }

  main.loaded {
    opacity: 1;
  }

  /* ─── Header ─────────────────────────────────────────── */
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 2px 0 6px;
    border-bottom: 1px solid #2a2a4a;
  }

  .brand {
    display: flex;
    align-items: baseline;
    gap: 7px;
  }

  .brand-name {
    font-size: 18px;
    font-weight: 700;
    letter-spacing: -0.5px;
    color: #fff;
    font-variant-numeric: tabular-nums;
  }

  .brand-ver {
    font-size: 11px;
    color: #555;
    font-weight: 400;
    letter-spacing: 0.2px;
  }

  .master-wrap {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .status-label {
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.5px;
    text-transform: uppercase;
    color: #555;
    transition: color 0.2s;
  }

  .status-label.active {
    color: #4ade80;
  }

  /* ─── Toggle Switch ───────────────────────────────────── */
  .toggle {
    position: relative;
    width: 42px;
    height: 23px;
    border-radius: 12px;
    border: none;
    background: #2a2a4a;
    cursor: pointer;
    transition: background 0.2s ease;
    flex-shrink: 0;
    outline: none;
  }

  .toggle:focus-visible {
    box-shadow: 0 0 0 2px #e9456066;
  }

  .toggle.on {
    background: #4ade80;
  }

  .toggle .thumb {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 17px;
    height: 17px;
    border-radius: 50%;
    background: #888;
    transition: left 0.2s ease, background 0.2s ease;
  }

  .toggle.on .thumb {
    left: 22px;
    background: #fff;
  }

  /* ─── Cards ───────────────────────────────────────────── */
  .card {
    background: #16213e;
    border: 1px solid #2a2a4a;
    border-radius: 8px;
    padding: 10px 12px;
  }

  .card h2 {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.8px;
    text-transform: uppercase;
    color: #555;
    margin-bottom: 9px;
  }

  .card-row h2 {
    margin-bottom: 0;
  }

  /* ─── Modifier Keys ───────────────────────────────────── */
  .mod-row {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 30px;
  }

  .mod-row + .mod-row {
    margin-top: 6px;
  }

  .mod-label {
    width: 42px;
    font-size: 12px;
    color: #aaa;
    flex-shrink: 0;
  }

  select {
    appearance: none;
    background: #0d1b36;
    border: 1px solid #2a2a4a;
    border-radius: 5px;
    color: #eee;
    font-size: 12px;
    font-family: inherit;
    padding: 4px 24px 4px 9px;
    height: 28px;
    cursor: pointer;
    outline: none;
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='10' height='6' viewBox='0 0 10 6'%3E%3Cpath fill='%23555' d='M0 0l5 6 5-6z'/%3E%3C/svg%3E");
    background-repeat: no-repeat;
    background-position: right 8px center;
    transition: border-color 0.15s;
  }

  select:focus {
    border-color: #0f3460;
    box-shadow: 0 0 0 2px #0f346044;
  }

  .resize-pair {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .plus {
    color: #444;
    font-size: 14px;
    font-weight: 300;
  }

  .hint-inline {
    font-size: 11px;
    color: #555;
    margin-left: auto;
    white-space: nowrap;
  }

  kbd {
    display: inline-block;
    padding: 1px 5px;
    font-size: 10px;
    font-family: inherit;
    background: #0d1b36;
    border: 1px solid #2a2a4a;
    border-radius: 3px;
    color: #888;
    line-height: 1.5;
  }

  /* ─── Process Filter ──────────────────────────────────── */
  .filter-top {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 8px;
  }

  .radio-tabs {
    display: flex;
    border: 1px solid #2a2a4a;
    border-radius: 5px;
    overflow: hidden;
    flex-shrink: 0;
  }

  .rtab {
    display: flex;
    align-items: center;
    padding: 4px 12px;
    font-size: 12px;
    color: #555;
    cursor: pointer;
    transition: background 0.15s, color 0.15s;
    user-select: none;
  }

  .rtab input {
    display: none;
  }

  .rtab.active {
    background: #0f3460;
    color: #eee;
  }

  .rtab:not(.active):hover {
    background: #1e2d4a;
    color: #888;
  }

  .rtab + .rtab {
    border-left: 1px solid #2a2a4a;
  }

  .filter-hint {
    font-size: 11px;
    color: #555;
  }

  /* ─── Pill List ───────────────────────────────────────── */
  .pill-list {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    min-height: 28px;
    margin-bottom: 8px;
    align-content: flex-start;
  }

  .pill {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    background: #0f3460;
    border: 1px solid #1a4a7a;
    border-radius: 4px;
    padding: 2px 6px 2px 8px;
    font-size: 11px;
    color: #cdd;
    line-height: 1.5;
  }

  .pill-x {
    background: none;
    border: none;
    color: #666;
    cursor: pointer;
    font-size: 14px;
    line-height: 1;
    padding: 0;
    display: flex;
    align-items: center;
    transition: color 0.1s;
  }

  .pill-x:hover {
    color: #e94560;
  }

  .pill-empty {
    font-size: 11px;
    color: #444;
    font-style: italic;
    line-height: 28px;
  }

  /* ─── Process Input Row ───────────────────────────────── */
  .proc-input-row {
    display: flex;
    gap: 6px;
    align-items: center;
  }

  .proc-input {
    flex: 1;
    background: #0d1b36;
    border: 1px solid #2a2a4a;
    border-radius: 5px;
    color: #eee;
    font-size: 12px;
    font-family: inherit;
    padding: 5px 9px;
    height: 28px;
    outline: none;
    transition: border-color 0.15s;
  }

  .proc-input::placeholder {
    color: #3a3a5a;
  }

  .proc-input:focus {
    border-color: #0f3460;
    box-shadow: 0 0 0 2px #0f346044;
  }

  .btn-sm {
    background: #0f3460;
    border: 1px solid #1a4a7a;
    border-radius: 5px;
    color: #aac;
    font-size: 11px;
    font-family: inherit;
    font-weight: 500;
    padding: 4px 10px;
    height: 28px;
    cursor: pointer;
    white-space: nowrap;
    transition: background 0.15s, border-color 0.15s;
    outline: none;
  }

  .btn-sm:hover {
    background: #14437a;
    border-color: #2060aa;
    color: #dde;
  }

  .btn-sm:active {
    background: #0a2840;
  }

  /* ─── Suggestions ─────────────────────────────────────── */
  .suggestions {
    margin-top: 6px;
    background: #0d1b36;
    border: 1px solid #2a2a4a;
    border-radius: 6px;
    overflow: hidden;
  }

  .suggestions-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 5px 10px;
    border-bottom: 1px solid #1e2a4a;
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.5px;
    text-transform: uppercase;
    color: #444;
  }

  .sugg-close {
    background: none;
    border: none;
    color: #444;
    font-size: 14px;
    cursor: pointer;
    padding: 0;
    line-height: 1;
    transition: color 0.1s;
  }

  .sugg-close:hover {
    color: #888;
  }

  .sugg-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    background: none;
    border: none;
    border-bottom: 1px solid #1a2040;
    color: #aaa;
    font-size: 12px;
    font-family: inherit;
    padding: 6px 10px;
    cursor: pointer;
    text-align: left;
    transition: background 0.1s, color 0.1s;
    outline: none;
  }

  .sugg-item:last-of-type {
    border-bottom: none;
  }

  .sugg-item:hover {
    background: #16243e;
    color: #dde;
  }

  .sugg-dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: #0f3460;
    border: 1px solid #2060aa;
    flex-shrink: 0;
  }

  .sugg-more {
    display: block;
    padding: 5px 10px;
    font-size: 10px;
    color: #444;
    border-top: 1px solid #1a2040;
  }

  /* ─── Autostart ───────────────────────────────────────── */
  .card-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .row-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
  }

  .row-label {
    font-size: 13px;
    color: #ccc;
  }

  /* ─── Save Bar ────────────────────────────────────────── */
  .save-bar {
    margin-top: auto;
    padding-top: 2px;
  }

  .save-btn {
    width: 100%;
    height: 38px;
    background: #e94560;
    border: none;
    border-radius: 7px;
    color: #fff;
    font-size: 13px;
    font-family: inherit;
    font-weight: 600;
    letter-spacing: 0.3px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    transition: background 0.15s, transform 0.1s, opacity 0.2s;
    outline: none;
  }

  .save-btn:hover:not(:disabled) {
    background: #ff5070;
  }

  .save-btn:active:not(:disabled) {
    transform: scale(0.99);
  }

  .save-btn:disabled {
    opacity: 0.7;
    cursor: default;
  }

  .save-btn.saved {
    background: #4ade80;
    color: #0a2010;
  }

  .save-btn.error {
    background: #7a1a2e;
    color: #ffaaaa;
  }

  .save-btn.saving {
    opacity: 0.8;
  }

  /* ─── Spinner ─────────────────────────────────────────── */
  .spinner {
    width: 13px;
    height: 13px;
    border: 2px solid rgba(255, 255, 255, 0.3);
    border-top-color: #fff;
    border-radius: 50%;
    animation: spin 0.6s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
