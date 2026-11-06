<script lang="ts">
  import ArrowClockwise from 'phosphor-svelte/lib/ArrowClockwise';
  import CloudArrowDown from 'phosphor-svelte/lib/CloudArrowDown';
  import CheckSquare from 'phosphor-svelte/lib/CheckSquare';
  import Trash from 'phosphor-svelte/lib/Trash';
  import X from 'phosphor-svelte/lib/X';
  import { api, toMsg } from '../api';
  import { app } from '../app.svelte';
  import { formatBytes } from '../browser';
  import { languageTag } from '../languages';
  import { t, tm } from '../i18n.svelte';
  import { providerLabel, type Msg, type RemoteTorrent } from '../types';
  import VirtualList from './VirtualList.svelte';

  let torrents = $state.raw<RemoteTorrent[]>([]);
  let account = $state('');
  let loading = $state(false);
  let loadedFor = $state('');
  let error = $state<Msg | null>(null);
  let selected = $state.raw(new Set<string>());
  let reviewing = $state<string[] | null>(null);
  let deleting = $state(false);
  let failures = $state<Record<string, Msg>>({});
  let token = 0;

  const provider = $derived(app.settings.provider);
  const queuedIds = $derived(new Set(app.jobs.filter((job) => job.provider === provider).map((job) => job.torrentId).filter((id): id is string => !!id)));

  async function load() {
    const mine = ++token;
    loading = true;
    error = null;
    try {
      const result = await api.remoteList();
      if (mine !== token) return;
      torrents = result.torrents;
      account = result.account;
      loadedFor = provider;
      selected = new Set();
      failures = {};
    } catch (e) {
      if (mine === token) { error = toMsg(e); torrents = []; }
    } finally {
      if (mine === token) loading = false;
    }
  }

  // Load when the page opens and again when the provider changes; a token change clears the list.
  $effect(() => {
    provider;
    if (loadedFor !== provider && app.hasToken) void load();
    else if (!app.hasToken) { torrents = []; error = { key: 'Dán API token để xem torrent trên dịch vụ đã chọn.', args: [] }; loadedFor = ''; }
  });

  const tone = (status: string) => (status === 'downloaded' ? 'success' : ['error', 'magnet_error', 'virus', 'dead'].includes(status) ? 'danger' : ['downloading', 'queued', 'magnet_conversion', 'compressing', 'uploading', 'waiting_files_selection'].includes(status) ? 'info' : '');
  const statusText = (torrent: RemoteTorrent) => `${torrent.status.replaceAll('_', ' ')}${torrent.status === 'downloaded' ? '' : ` ${torrent.progress.toFixed(0)}%`}`;

  function toggle(id: string) {
    const next = new Set(selected);
    if (!next.delete(id)) next.add(id);
    selected = next;
  }

  function review() {
    reviewing = torrents.filter((torrent) => selected.has(torrent.id)).map((torrent) => torrent.id);
  }

  async function confirmDelete() {
    if (!reviewing) return;
    deleting = true;
    const ids = reviewing;
    try {
      const outcomes = await api.remoteDelete(ids, account);
      const done = new Set(outcomes.filter((o) => o.ok).map((o) => o.id));
      failures = Object.fromEntries(outcomes.filter((o) => !o.ok && o.message).map((o) => [o.id, o.message as Msg]));
      torrents = torrents.filter((torrent) => !done.has(torrent.id));
      selected = new Set([...selected].filter((id) => !done.has(id)));
      if (done.size) app.toast({ key: 'Đã xóa {0} torrent trên {1}.', args: [String(done.size), providerLabel(provider)] }, 'success');
    } catch (e) {
      app.toast(toMsg(e), 'error');
    } finally {
      deleting = false;
      reviewing = null;
    }
  }
</script>

<section class="page">
  <header class="head">
    <div>
      <h1 class="title">{t('Torrent trên {0}', providerLabel(provider))}</h1>
      <p class="caption">{t('{0} torrent trên tài khoản · {1} đã chọn', torrents.length, selected.size)}</p>
    </div>
    <div class="actions">
      <button class="btn" disabled={loading || reviewing !== null || deleting} onclick={load}><span class="ic blue"><ArrowClockwise size={16} weight="bold" /></span>{t('Tải lại')}</button>
    </div>
  </header>

  <div class="card table">
    <p class="note caption">{t('Torrent trên tài khoản {0}. Xóa tại đây sẽ xóa trên dịch vụ; giữ file trên máy.', providerLabel(provider))}</p>
    {#if loading}<div class="indeterminate" role="progressbar"></div>{/if}
    {#if reviewing}
      <div class="review">{t('Xóa {0} torrent ID liệt kê bên dưới khỏi {1}. Dừng các tác vụ liên quan và bỏ dòng trong hàng đợi; giữ file trên máy. Xóa từ xa không thể hoàn tác.', reviewing.length, providerLabel(provider))}</div>
    {/if}
    <div class="cols head-row"><span></span><span>{t('Torrent / ID')}</span><span>{t('Dung lượng')}</span><span>{t('Trạng thái')}</span><span>{t('Ngày thêm')}</span></div>
    <div class="body">
      {#if torrents.length === 0}
        <div class="empty">
          <span class="chip ic purple" style="width:48px;height:48px;border-radius:14px"><CloudArrowDown size={22} weight="duotone" /></span>
          <p class="caption">{error ? tm(error) : loading ? t('Đang tải danh sách…') : t('Không có torrent nào trên tài khoản.')}</p>
        </div>
      {:else}
        <VirtualList items={torrents} rowHeight={60} key={(torrent) => torrent.id} label={t('Danh sách torrent trên tài khoản dịch vụ')}>
          {#snippet row(torrent: RemoteTorrent)}
            <div class="cols item row-glass" class:on={selected.has(torrent.id)}>
              <input type="checkbox" checked={selected.has(torrent.id)} disabled={reviewing !== null || deleting} onchange={() => toggle(torrent.id)} aria-label={torrent.filename} />
              <div class="names">
                <b class="ell" title={torrent.filename}>{torrent.filename}</b>
                <span class="line"><span class="mono id">{torrent.id}</span>{#if queuedIds.has(torrent.id)}<span class="pill info">{t('Trong hàng đợi')}</span>{/if}{#if failures[torrent.id]}<span class="pill danger">{tm(failures[torrent.id])}</span>{/if}</span>
              </div>
              <span>{formatBytes(torrent.bytes)}</span>
              <span><span class="pill {tone(torrent.status)}" title={torrent.status}>{statusText(torrent)}</span></span>
              <span class="caption">{torrent.added ? new Date(torrent.added).toLocaleString(languageTag(app.settings.language), { dateStyle: 'short', timeStyle: 'short' }) : '—'}</span>
            </div>
          {/snippet}
        </VirtualList>
      {/if}
    </div>
    <footer class="foot">
      <button class="btn" disabled={torrents.length === 0 || reviewing !== null} onclick={() => (selected = selected.size === torrents.length ? new Set() : new Set(torrents.map((x) => x.id)))}><span class="ic teal"><CheckSquare size={16} weight="duotone" /></span>{t('Chọn / bỏ chọn tất cả')}</button>
      <div class="actions">
        {#if reviewing}
          <button class="btn" disabled={deleting} onclick={() => (reviewing = null)}><span class="ic red"><X size={16} weight="bold" /></span>{t('Hủy')}</button>
          <button class="btn danger" disabled={deleting} onclick={confirmDelete}><span class="ic red"><Trash size={16} weight="duotone" /></span>{t('Xác nhận xóa từ xa')}</button>
        {:else}
          <button class="btn danger" disabled={selected.size === 0} onclick={review}><span class="ic red"><Trash size={16} weight="duotone" /></span>{t('Xóa torrent đã chọn…')}</button>
        {/if}
      </div>
    </footer>
  </div>
</section>

<style>
  .page { display: flex; flex-direction: column; height: 100%; min-height: 0; }
  .head { display: flex; justify-content: space-between; align-items: flex-end; gap: 16px; margin-bottom: 16px; }
  .actions { display: flex; align-items: center; gap: 4px; }
  .table { position: relative; flex: 1; min-height: 0; display: flex; flex-direction: column; overflow: hidden; }
  .note { padding: 12px 16px; }
  .review { margin: 0 16px 12px; padding: 12px 16px; background: var(--danger-soft); border-radius: 16px; line-height: 1.5; }
  .cols { display: grid; grid-template-columns: 52px minmax(180px, 1fr) 100px 190px 136px; align-items: center; padding-right: 16px; }
  .head-row { height: 36px; margin: 0 8px; color: var(--muted); font-size: 11px; font-weight: 600; }
  .body { flex: 1; min-height: 0; position: relative; }
  .item { height: 56px; }
  .item input { justify-self: center; }
  .names { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
  .ell { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .line { display: flex; gap: 8px; align-items: center; }
  .id { font-size: 11px; color: var(--muted); }
  .empty { position: absolute; inset: 0; display: flex; flex-direction: column; gap: 12px; align-items: center; justify-content: center; text-align: center; padding: 24px; }
  .foot { display: flex; align-items: center; justify-content: space-between; padding: 12px 16px; }
  .indeterminate { position: absolute; left: 0; right: 0; top: 40px; height: 3px; background: linear-gradient(90deg, transparent, var(--accent), transparent); background-size: 40% 100%; background-repeat: no-repeat; animation: slide 1.2s linear infinite; }
  @keyframes slide { from { background-position: -40% 0; } to { background-position: 140% 0; } }
</style>
