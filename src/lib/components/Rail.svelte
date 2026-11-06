<script lang="ts">
  import Books from 'phosphor-svelte/lib/Books';
  import DownloadSimple from 'phosphor-svelte/lib/DownloadSimple';
  import CloudArrowDown from 'phosphor-svelte/lib/CloudArrowDown';
  import Gear from 'phosphor-svelte/lib/Gear';
  import User from 'phosphor-svelte/lib/User';
  import { app, type Page } from '../app.svelte';
  import { t } from '../i18n.svelte';
  import { providerLabel } from '../types';

  const items: { page: Page; color: string; label: () => string }[] = [
    { page: 'library', color: 'blue', label: () => t('Thư viện') },
    { page: 'queue', color: 'green', label: () => t('Hàng đợi tải') },
    { page: 'remote', color: 'purple', label: () => t('Torrent trên {0}', providerLabel(app.settings.provider)) }
  ];

  const accountLine = $derived(app.user ? `${app.user.username} · ${app.user.kind}` : app.hasToken ? t('Chưa kiểm tra token mới') : t('Chưa kết nối'));
</script>

<nav class="rail" aria-label="Navigation">
  <div class="brand">
    <img src="/app-icon.png" alt="" width="32" height="32" />
    <span>DebridDock</span>
  </div>

  <div class="main">
    {#each items as item (item.page)}
      <button class="nav" class:on={app.page === item.page} aria-current={app.page === item.page ? 'page' : undefined} onclick={() => (app.page = item.page)}>
        <span class="ic {item.color}">
          {#if item.page === 'library'}<Books size={18} weight="duotone" />{:else if item.page === 'queue'}<DownloadSimple size={18} weight="duotone" />{:else}<CloudArrowDown size={18} weight="duotone" />{/if}
        </span>
        <span class="label">{item.label()}</span>
        {#if item.page === 'queue' && app.activeCount > 0}<span class="badge">{app.activeCount}</span>{/if}
      </button>
    {/each}
  </div>

  <div class="foot">
    <button class="account" onclick={() => (app.page = 'settings')} aria-label={t('Tài khoản')}>
      <span class="avatar"><User size={16} weight="duotone" class="ic blue" /></span>
      <span class="who">
        <b>{providerLabel(app.settings.provider)}</b>
        <small>{accountLine}</small>
      </span>
    </button>
    <button class="nav" class:on={app.page === 'settings'} aria-current={app.page === 'settings' ? 'page' : undefined} onclick={() => (app.page = 'settings')}>
      <span class="ic orange"><Gear size={18} weight="duotone" /></span>
      <span class="label">{t('Cài đặt')}</span>
    </button>
  </div>
</nav>

<style>
  .rail { display: flex; flex-direction: column; padding: 22px 8px 14px 16px; height: 100%; }
  .brand { display: flex; align-items: center; gap: 10px; margin: 0 0 30px 4px; font: 600 15px var(--font-display); }
  .brand img { border-radius: 8px; }
  .main { display: flex; flex-direction: column; gap: 4px; flex: 1; }
  .foot { display: flex; flex-direction: column; gap: 10px; }
  .nav { position: relative; display: flex; align-items: center; gap: 10px; height: 42px; padding: 0 14px; border-radius: 999px; color: var(--muted); font-weight: 500; text-align: left; transition: background 0.18s, box-shadow 0.18s, color 0.18s; }
  .nav:hover { background: var(--hover); color: var(--ink); }
  /* The current page is a raised glass droplet. */
  .nav.on { background: var(--sheen), var(--raised); box-shadow: var(--rim), var(--lift); color: var(--ink); font-weight: 600; }
  .label { flex: 1; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .badge { background: var(--gloss); box-shadow: var(--gloss-shadow); color: var(--on-accent); font-size: 11px; font-weight: 600; min-width: 20px; height: 18px; padding: 0 6px; border-radius: 9px; display: grid; place-content: center; }
  .account { display: flex; align-items: center; gap: 10px; padding: 10px; background: var(--sheen), var(--glass); box-shadow: var(--rim), var(--lift); backdrop-filter: blur(24px) saturate(170%); border-radius: 20px; text-align: left; width: 100%; transition: background 0.18s; }
  .account:hover { background: var(--sheen), var(--raised); }
  .avatar { width: 32px; height: 32px; border-radius: 16px; background: radial-gradient(120% 90% at 30% 15%, rgb(255 255 255 / 0.7), transparent 55%), var(--info-soft); box-shadow: inset 0 1px 0 rgb(255 255 255 / 0.6); display: grid; place-content: center; flex: none; }
  .who { display: flex; flex-direction: column; min-width: 0; line-height: 1.3; }
  .who b { font-size: 12px; }
  .who small { color: var(--muted); font-size: 11px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
</style>
