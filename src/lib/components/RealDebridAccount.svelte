<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import ArrowClockwise from 'phosphor-svelte/lib/ArrowClockwise';
  import { app } from '../app.svelte';
  import { languageTag } from '../languages';
  import { t, tm } from '../i18n.svelte';

  let { compact = false }: { compact?: boolean } = $props();
  const account = $derived(app.accounts.find((account) => account.provider === 'realDebrid'));
  const user = $derived(account?.user);
  const connected = $derived(account?.hasToken ?? false);
  const identity = $derived(user?.username ?? '');
  const loading = $derived(app.accountRefreshing.realDebrid ?? false);
  const error = $derived(app.accountErrors.realDebrid);
  let now = $state(Date.now());
  const received = $derived({ user, at: Date.now() });
  const expiration = $derived(user?.expiration ? Date.parse(user.expiration) : NaN);
  const remaining = $derived(user?.kind === 'premium' && user.premium > 0 ? Math.max(0, Number.isFinite(expiration) ? (expiration - now) / 1000 : user.premium - Math.max(0, now - received.at) / 1000) : 0);
  const plan = $derived(user?.kind === 'premium' ? 'Premium' : user?.kind === 'free' ? 'Free' : user?.kind ?? '');

  function duration(seconds: number) {
    const minutes = Math.ceil(seconds / 60);
    const days = Math.floor(minutes / 1440);
    const hours = Math.floor(minutes % 1440 / 60);
    if (days > 0) return t('{0} ngày {1} giờ', days, hours);
    if (hours > 0) return t('{0} giờ {1} phút', hours, minutes % 60);
    return t('{0} phút', minutes);
  }

  $effect(() => {
    const needsVerification = connected && !identity;
    untrack(() => { if (needsVerification) void app.refreshAccount('realDebrid'); });
  });
  onMount(() => { if (connected && identity) void app.refreshAccount('realDebrid'); });
  $effect(() => {
    const timer = setInterval(() => { now = Date.now(); }, 60_000);
    return () => clearInterval(timer);
  });
</script>

{#if connected}
  <section class="card realdebrid-account" aria-label={t('Tài khoản Real-Debrid')}>
    <header>
      <div class="identity"><b>Real-Debrid{#if plan}<span class="pill info">{plan}</span>{/if}</b><span class="caption ell" title={user?.email ?? identity}>{user?.email ?? identity}</span></div>
      <button class="btn icon" disabled={loading} onclick={() => app.refreshAccount('realDebrid')} title={t('Làm mới tài khoản')} aria-label={t('Làm mới tài khoản')}><ArrowClockwise size={16} weight="bold" /></button>
    </header>
    {#if loading && !user}<p class="caption">{t('Đang tải thông tin tài khoản…')}</p>{/if}
    {#if error}<p class="error" role="alert">{tm(error)}</p>{/if}
    {#if user}
      <div class="metrics" class:compact>
        <div><span class="caption">{t('Tài khoản')}</span><b class="ell" title={user.username}>{user.username}</b></div>
        <div><span class="caption">{t('Hết hạn gói')}</span><b>{Number.isFinite(expiration) ? new Date(expiration).toLocaleDateString(languageTag(app.settings.language)) : '—'}</b></div>
        <div><span class="caption">{t('Premium còn lại')}</span><b>{duration(remaining)}</b></div>
        <div><span class="caption">{t('Điểm Fidelity')}</span><b>{user.points == null ? '—' : user.points.toLocaleString(languageTag(app.settings.language))}</b></div>
      </div>
    {/if}
  </section>
{/if}

<style>
  .realdebrid-account { padding: 14px 16px; display: flex; flex-direction: column; gap: 12px; flex: none; min-width: 0; }
  header { display: flex; justify-content: space-between; align-items: center; gap: 12px; }
  .identity { display: flex; flex-direction: column; gap: 3px; min-width: 0; }
  .identity b { display: flex; align-items: center; gap: 8px; }
  .ell { overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
  .metrics { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 12px; }
  .metrics.compact { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .metrics > div { display: flex; flex-direction: column; gap: 5px; min-width: 0; }
  .metrics b { font-size: 13px; font-variant-numeric: tabular-nums; }
  .error { color: var(--danger); font-size: 12px; line-height: 1.5; }
  @media (max-width: 1050px) { .metrics { grid-template-columns: repeat(2, minmax(0, 1fr)); } }
</style>
