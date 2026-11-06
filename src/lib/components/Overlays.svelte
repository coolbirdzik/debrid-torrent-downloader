<script lang="ts">
  import { app } from '../app.svelte';
  import { answer, dialog } from '../confirm.svelte';
  import { t, tm } from '../i18n.svelte';

  let cancelButton: HTMLButtonElement | undefined = $state();
  $effect(() => {
    if (dialog.open) cancelButton?.focus();
  });
</script>

<svelte:window onkeydown={(event) => dialog.open && event.key === 'Escape' && answer(false)} />

{#if dialog.open && dialog.request}
  <div class="scrim" role="presentation">
    <div class="dialog card" role="alertdialog" aria-modal="true" aria-labelledby="dialog-title">
      <h2 id="dialog-title" class="section">{tm(dialog.request.title)}</h2>
      <p>{tm(dialog.request.body)}</p>
      <div class="actions">
        <button class="btn outline" bind:this={cancelButton} onclick={() => answer(false)}>{t('Hủy')}</button>
        <button class="btn primary" class:danger-fill={dialog.request.danger} onclick={() => answer(true)}>{tm(dialog.request.confirm)}</button>
      </div>
    </div>
  </div>
{/if}

<div class="toasts" aria-live="polite">
  {#each app.toasts as toast (toast.id)}
    <div class="toast card {toast.tone}">{tm(toast.message)}</div>
  {/each}
</div>

<style>
  .scrim { position: fixed; inset: 0; background: rgb(10 12 20 / 0.28); backdrop-filter: blur(6px); display: grid; place-content: center; z-index: 50; }
  .dialog { width: min(480px, 90vw); padding: 24px 26px; display: flex; flex-direction: column; gap: 12px; background: var(--sheen), var(--glass-strong); border-radius: 26px; }
  .dialog p { color: var(--muted); line-height: 1.55; user-select: text; }
  .actions { display: flex; justify-content: flex-end; gap: 10px; margin-top: 8px; }
  .danger-fill, .danger-fill:hover:not(:disabled) { background: linear-gradient(180deg, rgb(255 255 255 / 0.3), transparent 55%), var(--icon-red); color: #fff; }
  .toasts { position: fixed; right: 20px; bottom: 48px; display: flex; flex-direction: column; gap: 8px; z-index: 40; max-width: 420px; }
  .toast { padding: 11px 16px; border-radius: 18px; background: var(--sheen), var(--glass-strong); }
  .toast.error { color: var(--danger); }
  .toast.success { color: var(--success); }
</style>
