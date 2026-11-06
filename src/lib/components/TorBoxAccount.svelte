<script lang="ts">
  import { untrack } from 'svelte';
  import ArrowClockwise from 'phosphor-svelte/lib/ArrowClockwise';
  import { app } from '../app.svelte';
  import { formatBytes } from '../browser';
  import { languageTag } from '../languages';
  import { t, tm } from '../i18n.svelte';

  const connected = $derived(app.accounts.find((account) => account.provider === 'torBox')?.hasToken ?? false);
  const details = $derived(app.torboxAccount);
  const identity = $derived(app.accounts.find((account) => account.provider === 'torBox')?.user?.username ?? '');
  const bytes = (value: number | null | undefined) => value == null ? '—' : formatBytes(value);
  const usage = $derived(details?.airlockLimitBytes && details.airlockUsedBytes != null ? Math.min(100, details.airlockUsedBytes / details.airlockLimitBytes * 100) : 0);
  $effect(() => {
    connected; identity;
    untrack(() => { if (connected) void app.refreshTorboxAccount(); });
  });
</script>

{#if connected}
  <section class="card torbox-account" aria-label={t('Tài khoản TorBox')}>
    <header>
      <div class="identity"><b>TorBox{#if details}<span class="pill info">{details.plan}</span>{/if}</b><span class="caption ell" title={details?.email ?? details?.account ?? identity}>{details?.email ?? details?.account ?? identity}</span></div>
      <button class="btn icon" disabled={app.torboxAccountLoading} onclick={() => app.refreshTorboxAccount(true)} title={t('Làm mới tài khoản')} aria-label={t('Làm mới tài khoản')}><ArrowClockwise size={16} weight="bold" /></button>
    </header>
    {#if app.torboxAccountLoading && !details}<p class="caption">{t('Đang tải thông tin tài khoản…')}</p>{/if}
    {#if app.torboxAccountError}<p class="error" role="alert">{tm(app.torboxAccountError)}</p>{/if}
    {#if details}
      <div class="metrics">
        <div><span class="caption">{t('Hết hạn gói')}</span><b>{details.expiresAt ? new Date(details.expiresAt).toLocaleDateString(languageTag(app.settings.language)) : '—'}</b></div>
        <div><span class="caption">{t('AirLock còn lại')}</span><b>{bytes(details.airlockRemainingBytes)}</b></div>
        <div><span class="caption">{t('AirLock đã dùng / giới hạn')}</span><b>{bytes(details.airlockUsedBytes)} / {bytes(details.airlockLimitBytes)}</b></div>
        <div><span class="caption">{t('Dữ liệu sử dụng 30 ngày')}</span><b>{bytes(details.bandwidth30dBytes)}</b></div>
      </div>
      {#if details.airlockUsedBytes != null && details.airlockLimitBytes != null}
        <div class="usage" role="progressbar" aria-label={t('AirLock đã dùng / giới hạn')} aria-valuenow={usage} aria-valuemin="0" aria-valuemax="100"><span style:width="{usage}%"></span></div>
      {:else}<p class="caption">{t('TorBox chưa trả thông tin hạn mức AirLock.')}</p>{/if}
      {#if details.warning}<p class="error" role="alert">{tm(details.warning)}</p>{/if}
    {/if}
  </section>
{/if}

<style>
  .torbox-account { padding: 14px 16px; display: flex; flex-direction: column; gap: 12px; flex: none; }
  header { display: flex; justify-content: space-between; align-items: center; gap: 12px; }
  .identity { display: flex; flex-direction: column; gap: 3px; min-width: 0; }
  .identity b { display: flex; align-items: center; gap: 8px; }
  .ell { overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
  .metrics { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 12px; }
  .metrics > div { display: flex; flex-direction: column; gap: 5px; min-width: 0; }
  .metrics b { font-size: 13px; font-variant-numeric: tabular-nums; }
  .usage { height: 4px; border-radius: 999px; background: var(--track); overflow: hidden; }
  .usage span { display: block; height: 100%; background: var(--accent); }
  .error { color: var(--danger); font-size: 12px; line-height: 1.5; }
  @media (max-width: 1050px) { .metrics { grid-template-columns: repeat(2, minmax(0, 1fr)); } }
</style>
