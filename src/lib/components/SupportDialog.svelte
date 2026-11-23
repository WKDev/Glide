<script lang="ts">
  let {
    open,
    onDonate,
    onLater,
    onHaveCode,
  }: {
    open: boolean;
    onDonate: (target: 'sponsors' | 'coffee') => void;
    onLater: () => void;
    /** Omitted where supporter codes are unavailable (Microsoft Store). */
    onHaveCode?: () => void;
  } = $props();

  const HEART = '';

  let dialog: globalThis.HTMLDialogElement;

  $effect(() => {
    if (open && !dialog.open) dialog.showModal();
    if (!open && dialog.open) dialog.close();
  });
</script>

<dialog
  bind:this={dialog}
  class="support-dialog"
  aria-labelledby="support-title"
  oncancel={(event) => {
    event.preventDefault();
    onLater();
  }}
>
  <div class="content">
    <img class="mark" src="/glide-mark.png" alt="" />
    <h2 id="support-title">Support Glide?</h2>
    <p>
      Glide is free, and one person builds it in their spare time. If it saves
      you a few clicks every day, a small donation helps keep it going.
    </p>
    <div class="donate">
      <button
        type="button"
        class="button accent"
        onclick={() => onDonate('sponsors')}
      >
        <span class="glyph" aria-hidden="true">{HEART}</span>
        Sponsor on GitHub
      </button>
      <button type="button" class="button" onclick={() => onDonate('coffee')}>
        Buy me a coffee
      </button>
    </div>
  </div>
  <footer>
    {#if onHaveCode}
      <button type="button" class="link-button" onclick={onHaveCode}>
        I have a supporter code
      </button>
    {/if}
    <button type="button" class="button later" onclick={onLater}>Not now</button
    >
  </footer>
</dialog>

<style>
  .support-dialog {
    margin: auto;
    width: 380px;
    max-width: calc(100vw - 48px);
    padding: 0;
    border: 1px solid var(--surface-stroke);
    border-radius: 8px;
    background: var(--dialog);
    color: var(--text);
    box-shadow:
      0 32px 64px rgb(0 0 0 / 0.19),
      0 2px 21px rgb(0 0 0 / 0.15);
    overflow: hidden;
  }

  .support-dialog::backdrop {
    background: var(--smoke);
  }

  .support-dialog[open] {
    animation: rise 220ms cubic-bezier(0.16, 1, 0.3, 1);
  }

  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(12px) scale(0.98);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .support-dialog[open] {
      animation: none;
    }
  }

  .content {
    padding: 24px 24px 20px;
  }

  .mark {
    display: block;
    width: 40px;
    height: 40px;
    margin-bottom: 14px;
  }

  h2 {
    margin-bottom: 8px;
    font-size: 20px;
    font-weight: 600;
    line-height: 28px;
  }

  p {
    color: var(--text-secondary);
    font-size: 14px;
    line-height: 20px;
  }

  .donate {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 18px;
  }

  .later {
    margin-left: auto;
  }

  footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 16px 24px;
    border-top: 1px solid var(--divider);
    background: var(--dialog-footer);
  }
</style>
