<script lang="ts">
  import { open as pickFolder } from '@tauri-apps/plugin-dialog';
  import ArrowsClockwise from 'phosphor-svelte/lib/ArrowsClockwise';
  import Check from 'phosphor-svelte/lib/Check';
  import DownloadSimple from 'phosphor-svelte/lib/DownloadSimple';
  import FolderSimple from 'phosphor-svelte/lib/FolderSimple';
  import { api } from '../api';
  import { app } from '../app.svelte';
  import { t, tm } from '../i18n.svelte';
  import { LANGUAGES } from '../languages';
  import { PROVIDERS, providerLabel, type Provider, type Settings } from '../types';
  import Select from './Select.svelte';
  import TorBoxAccount from './TorBoxAccount.svelte';

  let token = $state('');
  let folder = $state(app.settings.outputDirectory);
  $effect(() => { folder = app.settings.outputDirectory; });

  const status = $derived(app.user ? `${app.user.username} · ${app.user.kind}` : app.hasToken ? t('Chưa kiểm tra token mới') : t('Chưa kết nối'));
  const accountLine = (account: (typeof app.accounts)[number]) =>
    account.user ? `${account.user.username} · ${account.user.kind}` : account.hasToken ? t('Chưa kiểm tra token mới') : t('Chưa kết nối');
  const savedChoice = $derived(app.settings.libraryProviders.map(providerLabel).join(', '));

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
        <div class="field">
          <b>{t('Dịch vụ tải')}</b>
          <Select width="260px" label={t('Dịch vụ tải')} value={app.settings.provider} options={PROVIDERS.map((p) => ({ value: p.id, label: p.label }))} onchange={(v: Provider) => app.switchProvider(v)} />
        </div>
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
        <div class="accounts" aria-label={t('Tài khoản')}>
          {#each app.accounts as account (account.provider)}
            <button class="account" class:on={account.provider === app.settings.provider} onclick={() => app.switchProvider(account.provider)}>
              <b>{providerLabel(account.provider)}</b><small>{accountLine(account)}</small>
            </button>
          {/each}
        </div>
        <label class="check" title={t('Khi có token của nhiều dịch vụ, hỏi thêm torrent vào dịch vụ nào (một, vài hoặc tất cả).')}>
          <input type="checkbox" checked={!app.settings.rememberLibraryProviders} onchange={(e) => app.setSettings({ rememberLibraryProviders: !e.currentTarget.checked })} />{t('Hỏi chọn dịch vụ mỗi khi thêm vào thư viện')}
        </label>
        {#if app.settings.rememberLibraryProviders && savedChoice}<p class="caption">{t('Đang dùng: {0}', savedChoice)}</p>{/if}
      </div>

      <TorBoxAccount />

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
        <div class="field">
          <b>{t('Số torrent tải song song')}</b>
          <Select width="96px" label={t('Số torrent tải song song')} value={app.settings.concurrency} options={[1, 2, 3, 4, 5].map((n) => ({ value: n, label: String(n) }))} onchange={(n: number) => app.setSettings({ concurrency: n })} />
        </div>
      </div>

      <div class="card block">
        <h2 class="section">{t('Giao diện và ngôn ngữ')}</h2>
        <div class="two">
          <div class="field">
            <b>{t('Giao diện')}</b>
            <Select
              label={t('Giao diện')}
              value={app.settings.appearance}
              options={[{ value: 'system', label: t('Theo hệ thống') }, { value: 'light', label: t('Sáng') }, { value: 'dark', label: t('Tối') }]}
              onchange={(v: Settings['appearance']) => app.setSettings({ appearance: v })}
            />
          </div>
          <div class="field">
            <b>{t('Ngôn ngữ')}</b>
            <Select
              label={t('Ngôn ngữ')}
              value={app.settings.language}
              options={LANGUAGES.map((language) => ({ value: language.code, label: language.label }))}
              onchange={(v: Settings['language']) => app.setSettings({ language: v })}
            />
          </div>
        </div>
      </div>
      <div class="card block">
        <h2 class="section">{t('Cập nhật')}</h2>
        <div class="between">
          <span>{t('Phiên bản hiện tại: {0}', app.version || '—')}</span>
          <button class="btn outline" disabled={app.checkingUpdate || app.updating} onclick={() => app.checkUpdate()}>
            <span class="ic blue"><ArrowsClockwise size={16} weight="bold" /></span>{app.checkingUpdate ? t('Đang kiểm tra…') : t('Kiểm tra cập nhật')}
          </button>
        </div>
        {#if app.update?.available}
          <div class="found">
            <div class="between">
              <b>{t('Có phiên bản mới: {0}', app.update.latest ?? '')}</b>
              <button class="btn primary" disabled={app.updating || app.checkingUpdate || !app.update.canInstall} onclick={() => app.installUpdate()}><DownloadSimple size={16} weight="bold" />{app.updating ? t('Đang cập nhật ứng dụng…') : t('Cập nhật và khởi động lại')}</button>
            </div>
            <p class="caption">{t('Ứng dụng tự tải, cài đặt và khởi động lại. Hàng đợi được lưu trước khi cài.')}</p>
            {#if !app.update.canInstall}<p class="caption">{t('Bản phát hành này chưa hỗ trợ cập nhật trong ứng dụng.')}</p>{/if}
            {#if app.updateProgress}
              <div role="status" aria-live="polite">
                <p class="caption">{app.updateProgress.phase === 'installing' ? t('Đang cài đặt và khởi động lại…') : t('Đang tải bản cập nhật…')}</p>
                {#if app.updateProgress.phase === 'downloading'}
                  {#if app.updateProgress.total}
                    <progress max={app.updateProgress.total} value={app.updateProgress.downloaded}></progress>
                    <span class="caption">{Math.min(100, Math.floor(app.updateProgress.downloaded / app.updateProgress.total * 100))}%</span>
                  {:else}<progress></progress>{/if}
                {/if}
              </div>
            {/if}
            {#if app.updateError}<p class="caption" role="alert">{tm(app.updateError)}</p>{/if}
            {#if app.update.notes}
              <div class="caption notes" aria-label={t('Ghi chú phát hành')}>{app.update.notes}</div>
            {/if}
            {#if app.update.pageUrl}
              <button class="btn link" onclick={() => app.run(() => api.openRelease(app.update!.pageUrl!))}>{t('Xem trên GitHub')}</button>
            {/if}
          </div>
        {:else if app.update}
          <p class="caption">{app.update.latest ? t('Bạn đang dùng bản mới nhất.') : t('Chưa có bản phát hành nào trên GitHub.')}</p>
        {/if}
        <label class="check"><input type="checkbox" checked={app.settings.checkUpdates} onchange={(e) => app.setSettings({ checkUpdates: e.currentTarget.checked })} />{t('Tự kiểm tra khi mở ứng dụng')}</label>
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
  .pair { display: flex; gap: 8px; }
  .between { display: flex; justify-content: space-between; align-items: center; }
  .check { display: flex; align-items: center; gap: 10px; cursor: pointer; }
  .accounts { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 6px; }
  .account { display: flex; flex-direction: column; align-items: flex-start; gap: 2px; min-width: 0; padding: 9px 12px; border-radius: 14px; background: var(--subtle); text-align: left; transition: background 0.15s; }
  .account:hover { background: var(--hover); }
  .account.on { background: var(--selected); }
  .account small { max-width: 100%; color: var(--muted); font-size: 11px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .two { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; }
  .found { display: flex; flex-direction: column; gap: 10px; padding: 12px 14px; border-radius: 14px; background: var(--info-soft); }
  .notes { white-space: pre-wrap; max-height: 160px; overflow-y: auto; user-select: text; }
  .data { user-select: text; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
