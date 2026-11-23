<script lang="ts">
  /** Which part of the mouse the gesture uses. */
  let { action }: { action: 'left' | 'right' | 'move' | 'wheel' } = $props();
</script>

<svg class="mouse" viewBox="0 0 28 28" aria-hidden="true">
  <g transform={action === 'move' ? 'translate(-3 0)' : undefined}>
    <rect class="body" x="7.5" y="3.5" width="13" height="21" rx="6.5" />
    <path class="split" d="M14 3.5v8M7.5 11.5h13" />
    {#if action === 'left'}
      <path class="hot" d="M14 4v7H8v-1a6 6 0 0 1 6-6z" />
    {:else if action === 'right'}
      <path class="hot" d="M14 4v7h6v-1a6 6 0 0 0-6-6z" />
    {:else if action === 'wheel'}
      <rect class="hot" x="12.6" y="6" width="2.8" height="5" rx="1.4" />
    {/if}
  </g>
  {#if action === 'move'}
    <path class="trail" d="M21 9.5l4 4.5-4 4.5M19 14h6" />
  {/if}
</svg>

<style>
  .mouse {
    width: 22px;
    height: 22px;
    flex: none;
  }

  .body {
    fill: var(--key-bg);
    stroke: var(--text-secondary);
    stroke-width: 1.3;
  }

  .split {
    fill: none;
    stroke: var(--text-secondary);
    stroke-width: 1.1;
  }

  .hot {
    fill: var(--accent);
  }

  .trail {
    fill: none;
    stroke: var(--accent);
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
</style>
