<script lang="ts">
  import { open as pickFolder } from '@tauri-apps/plugin-dialog';
  import Check from 'phosphor-svelte/lib/Check';
  import FolderSimple from 'phosphor-svelte/lib/FolderSimple';
  import { api } from '../api';
  import { app } from '../app.svelte';
  import { t } from '../i18n.svelte';
  import { PROVIDERS, type Provider, type Settings } from '../types';

  let token = $state('');
  let folder = $state(app.settings.outputDirectory);
  $effect(() => { folder = app.settings.outputDirectory; });

  const status = $derived(app.user ? `${app.user.username} · ${app.user.kind}` : app.hasToken ? t('Chưa kiểm tra token mới') : t('Chưa kết nối'));

  async function commitToken() {
    if (token.trim() === '' && app.hasToken) return;
    await app.saveToken(token);
    token = '';
  }

  async function connect() {
    if (token.trim()) await commitToken();
    await app.connect();
  }

  async function browse() {
    const picked = await pickFolder({ directory: true, multiple: false, title: t('Chọn nơi lưu toàn bộ torrent'), defaultPath: app.settings.outputDirectory || undefined });
    if (typeof picked === 'string') await app.setSettings({ outputDirectory: picked });
  }
</script>

<section class="page">
  <header class="head">
    <h1 class="title">{t('Cài đặt')}</h1>
    <p class="caption">{t('Tài khoản, thư mục lưu và giao diện.')}</p>
  </header>

  <div class="scroll">
    <div class="stack">
      <div class="card block">
        <h2 class="section">{t('Tài khoản')}</h2>
        <label class="field">
          <b>{t('Dịch vụ tải')}</b>
          <select class="narrow" value={app.settings.provider} onchange={(e) => app.switchProvider(e.currentTarget.value as Provider)}>
            {#each PROVIDERS as provider (provider.id)}<option value={provider.id}>{provider.label}</option>{/each}
          </select>
        </label>
        <div class="field">
          <label for="token"><b>API token</b></label>
          <div class="pair">
            <input id="token" type="password" bind:value={token} placeholder={app.hasToken ? '••••••••••••' : ''} autocomplete="off" spellcheck="false" onchange={commitToken} title={t('Dán private API token của bạn')} />
            <button class="btn outline" disabled={app.connecting || (!app.hasToken && !token.trim())} onclick={connect}><span class="ic green"><Check size={16} weight="bold" /></span>{t('Kiểm tra')}</button>
          </div>
        </div>
        <div class="between">
          <label class="check"><input type="checkbox" checked={app.settings.rememberToken} onchange={(e) => app.setSettings({ rememberToken: e.currentTarget.checked })} />{t('Nhớ token trên máy này')}</label>
          <button class="btn link" onclick={() => api.openTokenPage(app.settings.provider)}>{t('Lấy API token')}</button>
        </div>
        <p class="caption">{status}</p>
      </div>

      <div class="card block">
        <h2 class="section">{t('Thư mục tải xuống')}</h2>
        <div class="pair">
          <input type="text" bind:value={folder} aria-label={t('Thư mục lưu')} onchange={() => app.setSettings({ outputDirectory: folder })} />
          <button class="btn outline icon" onclick={browse} title={t('Chọn thư mục')} aria-label={t('Chọn thư mục')}><span class="ic amber"><FolderSimple size={17} weight="duotone" /></span></button>
        </div>
        <label class="check" title={t('Bỏ chọn để tạo thư mục riêng và giữ cấu trúc torrent. Áp dụng cho torrent chưa bắt đầu lưu file; torrent tải dở giữ đường dẫn cũ để tiếp tục.')}>
          <input type="checkbox" checked={app.settings.flattenFiles} onchange={(e) => app.setSettings({ flattenFiles: e.currentTarget.checked })} />{t('Lưu tất cả file cùng thư mục')}
        </label>
        <p class="caption">{t('File tải dở giữ nguyên đường dẫn để tiếp tục.')}</p>
        <label class="field">
          <b>{t('Số torrent tải song song')}</b>
          <select class="tiny" value={String(app.settings.concurrency)} onchange={(e) => app.setSettings({ concurrency: Number(e.currentTarget.value) })}>
            {#each [1, 2, 3, 4, 5] as n (n)}<option value={String(n)}>{n}</option>{/each}
          </select>
        </label>
      </div>

      <div class="card block">
        <h2 class="section">{t('Giao diện và ngôn ngữ')}</h2>
        <div class="two">
          <label class="field">
            <b>{t('Giao diện')}</b>
            <select value={app.settings.appearance} onchange={(e) => app.setSettings({ appearance: e.currentTarget.value as Settings['appearance'] })}>
              <option value="system">{t('Theo hệ thống')}</option>
              <option value="light">{t('Sáng')}</option>
              <option value="dark">{t('Tối')}</option>
            </select>
          </label>
          <label class="field">
            <b>{t('Ngôn ngữ')}</b>
            <select value={app.settings.language} onchange={(e) => app.setSettings({ language: e.currentTarget.value as 'en' | 'vi' })}>
              <option value="vi">Tiếng Việt</option>
              <option value="en">English</option>
            </select>
          </label>
        </div>
      </div>
      <p class="caption data" title={app.dataFile}>{app.dataFile}</p>
    </div>
  </div>
</section>

<style>
  .page { display: flex; flex-direction: column; height: 100%; min-height: 0; }
  .head { margin-bottom: 16px; }
  .scroll { flex: 1; overflow-y: auto; min-height: 0; }
  .stack { width: 680px; max-width: 100%; display: flex; flex-direction: column; gap: 16px; padding-bottom: 24px; }
  .block { padding: 20px 22px; display: flex; flex-direction: column; gap: 14px; }
  .field { display: flex; flex-direction: column; gap: 6px; }
  .narrow { max-width: 260px; } .tiny { width: 96px; }
  .pair { display: flex; gap: 8px; }
  .between { display: flex; justify-content: space-between; align-items: center; }
  .check { display: flex; align-items: center; gap: 10px; cursor: pointer; }
  .two { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; }
  .data { user-select: text; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
