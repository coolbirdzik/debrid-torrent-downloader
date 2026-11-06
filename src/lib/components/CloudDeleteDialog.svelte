<script lang="ts">
  import ArrowClockwise from 'phosphor-svelte/lib/ArrowClockwise';
  import CloudArrowDown from 'phosphor-svelte/lib/CloudArrowDown';
  import Trash from 'phosphor-svelte/lib/Trash';
  import X from 'phosphor-svelte/lib/X';
  import { formatBytes } from '../browser';
  import { t } from '../i18n.svelte';
  import { remoteKey, type RemoteRow } from '../remote';
  import { cloudKindLabel, providerLabel } from '../types';
  import VirtualList from './VirtualList.svelte';

  let { rows, deleting, oncancel, onconfirm }: {
    rows: RemoteRow[];
    deleting: boolean;
    oncancel: () => void;
    onconfirm: () => void;
  } = $props();

  let modal: HTMLDialogElement | undefined = $state();
  let cancelButton: HTMLButtonElement | undefined = $state();
  const bytes = $derived(rows.reduce((sum, row) => sum + row.bytes, 0));
  const identity = (row: RemoteRow) => `${providerLabel(row.provider)} · ${row.account} · ${cloudKindLabel(row.kind ?? 'torrent')} · ${row.id}`;

  $effect(() => {
    const element = modal;
    if (element && !element.open) {
      const previousFocus = document.activeElement;
      element.showModal();
      cancelButton?.focus();
      return () => {
        element.close();
        queueMicrotask(() => {
          if (previousFocus instanceof HTMLElement && previousFocus.isConnected) previousFocus.focus();
        });
      };
    }
  });
</script>

<dialog bind:this={modal} class="cloud-delete card" aria-labelledby="cloud-delete-title" aria-describedby="cloud-delete-description cloud-delete-impact" aria-busy={deleting} oncancel={(event) => { event.preventDefault(); if (!deleting) oncancel(); }}>
  <header>
    <span class="delete-icon"><Trash size={22} weight="duotone" /></span>
    <div class="heading">
      <h2 id="cloud-delete-title">{t('Xóa dữ liệu cloud?')}</h2>
      <p class="caption">{t('Đã chọn {0} mục · {1}', rows.length, formatBytes(bytes))}</p>
    </div>
    <button class="btn icon close" aria-label={t('Đóng')} title={t('Đóng')} disabled={deleting} onclick={oncancel}><X size={18} /></button>
  </header>

  <div class="contents">
    <p id="cloud-delete-description">{t('Các mục này và file bên trong sẽ bị xóa khỏi dịch vụ. Không thể hoàn tác.')}</p>
    <div class="review-list" style:height="{Math.min(rows.length, 4) * 60}px">
      <VirtualList items={rows} rowHeight={60} key={remoteKey} role="list" label={t('Danh sách dữ liệu cloud')}>
        {#snippet row(item: RemoteRow)}
          <div class="review-item" role="listitem">
            <span class="file-icon"><CloudArrowDown size={18} weight="duotone" /></span>
            <div class="names">
              <b title={item.filename}>{item.filename}</b>
              <span class="caption" title={identity(item)}>{identity(item)}</span>
            </div>
            <span class="size caption">{formatBytes(item.bytes)}</span>
          </div>
        {/snippet}
      </VirtualList>
    </div>
    <p id="cloud-delete-impact" class="caption">{t('Tác vụ tải liên quan sẽ dừng và bị bỏ khỏi hàng đợi. File trên máy được giữ lại.')}</p>
  </div>

  <footer>
    <button class="btn outline cancel" bind:this={cancelButton} disabled={deleting} onclick={oncancel}>{t('Hủy')}</button>
    <button class="btn primary confirm" disabled={deleting} onclick={onconfirm}>
      {#if deleting}<ArrowClockwise size={16} weight="bold" class="spin" />{:else}<Trash size={16} weight="duotone" />{/if}
      {t(deleting ? 'Đang xóa từ xa…' : 'Xác nhận xóa từ xa')}
    </button>
  </footer>
</dialog>

<style>
  .cloud-delete { width: min(600px, calc(100vw - 32px)); max-height: calc(100dvh - 48px); padding: 0; border: 0; border-radius: 24px; color: var(--ink); background: var(--glass-strong); box-shadow: var(--float); backdrop-filter: blur(20px); overflow: hidden; }
  .cloud-delete[open] { display: flex; flex-direction: column; }
  .cloud-delete::backdrop { background: rgb(10 10 11 / 0.36); backdrop-filter: blur(4px); }
  header { display: flex; align-items: center; gap: 14px; padding: 24px 24px 20px; flex: none; }
  .delete-icon { display: grid; place-items: center; width: 44px; height: 44px; border-radius: 14px; background: var(--danger-soft); color: var(--danger); flex: none; }
  .heading { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
  h2 { font: 600 20px/1.25 var(--font-display); letter-spacing: -0.01em; text-wrap: balance; }
  .close { margin-left: auto; align-self: flex-start; flex: none; }
  .contents { display: flex; flex-direction: column; gap: 16px; padding: 0 24px 24px; overflow-y: auto; min-height: 0; }
  p { line-height: 1.55; text-wrap: pretty; }
  .review-list { min-height: 60px; flex: none; background: var(--subtle); border-radius: 14px; overflow: hidden; }
  .review-item { display: flex; align-items: center; gap: 12px; height: 60px; padding: 10px 12px; }
  .file-icon { color: var(--muted); display: grid; place-items: center; width: 32px; height: 32px; border-radius: 10px; background: var(--surface); flex: none; }
  .names { display: flex; flex-direction: column; gap: 3px; min-width: 0; flex: 1; }
  .names b, .names .caption { white-space: nowrap; text-overflow: ellipsis; overflow: hidden; }
  .names b { font-weight: 600; font-size: 12px; }
  .names .caption { font-size: 11px; }
  .size { flex: none; font-variant-numeric: tabular-nums; }
  footer { display: flex; justify-content: flex-end; align-items: center; gap: 10px; padding: 16px 24px 20px; background: var(--subtle); flex: none; }
  .cancel { min-height: 40px; padding-inline: 20px; background: var(--surface); }
  .confirm, .confirm:hover:not(:disabled) { background: var(--icon-red); color: #fff; }
  .confirm:hover:not(:disabled) { filter: brightness(0.94); }
  .confirm:disabled { background: var(--track); color: var(--muted); }
  :global(.spin) { animation: spin 1s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
  @media (prefers-reduced-motion: reduce) { :global(.spin) { animation: none; } }
  @media (max-width: 520px) { header { padding: 20px 20px 16px; } .contents { padding: 0 20px 20px; } footer { padding: 16px 20px; flex-wrap: wrap; } h2 { font-size: 18px; } }
</style>
