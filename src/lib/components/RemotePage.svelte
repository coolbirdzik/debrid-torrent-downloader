<script lang="ts">
  import ArrowClockwise from 'phosphor-svelte/lib/ArrowClockwise';
  import CloudArrowDown from 'phosphor-svelte/lib/CloudArrowDown';
  import CheckSquare from 'phosphor-svelte/lib/CheckSquare';
  import CaretUp from 'phosphor-svelte/lib/CaretUp';
  import CaretDown from 'phosphor-svelte/lib/CaretDown';
  import Trash from 'phosphor-svelte/lib/Trash';
  import { onDestroy, untrack } from 'svelte';
  import { api } from '../api';
  import { app } from '../app.svelte';
  import { formatBytes } from '../browser';
  import { languageTag } from '../languages';
  import { t, tm } from '../i18n.svelte';
  import { emptyRemoteState, nextRemoteSort, parseRemoteScope, RemoteBrowser, remoteKey, remoteQueuedKeys, sortRemoteRows, type RemoteRow, type RemoteSort, type RemoteSortColumn } from '../remote';
  import { CLOUD_KINDS, cloudKindLabel, providerLabel, type CloudFilter, type Provider, type RemoteTorrent } from '../types';
  import TorBoxAccount from './TorBoxAccount.svelte';
  import RealDebridAccount from './RealDebridAccount.svelte';
  import Select from './Select.svelte';
  import VirtualList from './VirtualList.svelte';
  import { selectionMouseDown } from '../selection';
  import CloudDeleteDialog from './CloudDeleteDialog.svelte';

  let view = $state.raw(emptyRemoteState());
  let filter = $state<Provider | 'all'>('all');
  let kind = $state<CloudFilter>('all');
  let sort = $state<RemoteSort>(null);
  let listRef: { reveal: (index: number) => void } | undefined = $state();
  const remote = new RemoteBrowser(api, (next) => (view = next));
  const torrents = $derived(sortRemoteRows(view.rows.filter((row) => filter === 'all' || row.provider === filter), sort, languageTag(app.settings.language)));
  const queuedKeys = $derived(remoteQueuedKeys(app.jobs));
  const visibleErrors = $derived(Object.entries(view.errors).filter(([scope]) => filter === 'all' || parseRemoteScope(scope).provider === filter));
  const reviewTargets = $derived([...new Set(view.reviewing?.map((row) => `${providerLabel(row.provider)} · ${row.account} · ${cloudKindLabel(row.kind ?? 'torrent')}`) ?? [])].join(', '));
  const totalBytes = $derived(torrents.reduce((sum, row) => sum + row.bytes, 0));
  const showRealDebrid = $derived((filter === 'all' || filter === 'realDebrid') && app.accounts.some((account) => account.provider === 'realDebrid' && account.hasToken));
  const showTorBox = $derived((filter === 'all' || filter === 'torBox') && app.accounts.some((account) => account.provider === 'torBox' && account.hasToken));

  function load() { return remote.load(app.usableProviders, kind); }

  function sortBy(column: RemoteSortColumn) {
    if (view.reviewing || view.deleting) return;
    sort = nextRemoteSort(sort, column);
    listRef?.reveal(0);
  }

  function keydown(event: KeyboardEvent) {
    const result = remote.keydown(torrents, event);
    if (!result) return;
    event.preventDefault();
    if (result.index !== undefined) {
      listRef?.reveal(result.index);
      (event.currentTarget as HTMLElement).focus();
    }
  }

  // Connecting/disconnecting a service or changing a known identity invalidates the old list.
  $effect(() => {
    const accounts = app.accounts.map((a) => ({ ...a, user: a.user ? { ...a.user } : null }));
    const currentKind = kind;
    untrack(() => { void remote.syncAccounts(accounts, currentKind); });
  });
  onDestroy(() => remote.invalidate());

  const tone = (status: string) => (status === 'downloaded' ? 'success' : ['error', 'magnet_error', 'virus', 'dead'].includes(status) ? 'danger' : ['downloading', 'queued', 'magnet_conversion', 'compressing', 'uploading', 'waiting_files_selection'].includes(status) ? 'info' : '');
  const statusText = (torrent: RemoteTorrent) => `${torrent.status.replaceAll('_', ' ')}${torrent.status === 'downloaded' ? '' : ` ${torrent.progress.toFixed(0)}%`}`;

  function changeFilter(value: Provider | 'all') {
    if (view.deleting || view.reviewing) return;
    filter = value;
    remote.clearSelection();
  }

  async function confirmDelete() {
    const targets = reviewTargets;
    const count = await remote.confirmDelete();
    if (count) app.toast({ key: 'Đã xóa {0} mục cloud trên {1}.', args: [String(count), targets] }, 'success');
    if (count) void app.refreshTorboxAccount(true);
  }
</script>

{#snippet headerCell(column: RemoteSortColumn, label: string)}
  {@const active = sort?.column === column}
  <button class="hcell" class:active aria-pressed={active} disabled={view.reviewing !== null || view.deleting} onclick={() => sortBy(column)} title={t('Sắp xếp theo cột {0}. Bấm lần nữa để đảo chiều, lần thứ ba để bỏ.', label)}>
    {label}
    {#if active && sort?.direction === 'asc'}<CaretUp size={12} weight="bold" />{:else if active}<CaretDown size={12} weight="bold" />{/if}
  </button>
{/snippet}

<section class="page">
  <header class="head">
    <div>
      <h1 class="title">{t('Quản lý cloud')}</h1>
      <p class="caption">{t('{0} mục cloud · {1} đã chọn · {2}', torrents.length, view.selected.size, formatBytes(totalBytes))}</p>
    </div>
    <div class="actions">
      <Select label={t('Nhà cung cấp')} width="180px" value={filter} options={[{ value: 'all' as const, label: t('Tất cả dịch vụ') }, ...app.accounts.filter((a) => a.hasToken || a.provider === filter).map((a) => ({ value: a.provider, label: providerLabel(a.provider) }))]} onchange={changeFilter} />
      <Select label={t('Loại dữ liệu cloud')} width="170px" value={kind} options={[{ value: 'all' as const, label: t('Tất cả loại') }, ...CLOUD_KINDS.map((value) => ({ value, label: cloudKindLabel(value) }))]} onchange={(value: CloudFilter) => { if (view.reviewing || view.deleting) return; remote.clearSelection(); kind = value; }} />
      <button class="btn" disabled={view.loading || view.reviewing !== null || view.deleting} onclick={load}><span class="ic blue"><ArrowClockwise size={16} weight="bold" /></span>{t('Tải lại')}</button>
    </div>
  </header>

  {#if showRealDebrid || showTorBox}
    <div class="account-summary" class:multiple={showRealDebrid && showTorBox}>
      {#if showRealDebrid}<RealDebridAccount compact={showRealDebrid && showTorBox} />{/if}
      {#if showTorBox}<TorBoxAccount compact={showRealDebrid && showTorBox} />{/if}
    </div>
  {/if}

  <div class="card table">
    <p class="note caption">{t('Quản lý dữ liệu trên dịch vụ. Xóa một mục cloud sẽ xóa các file trong mục đó trên dịch vụ; giữ file trên máy.')}</p>
    {#if view.loading}<div class="indeterminate" role="progressbar" aria-label={t('Đang tải danh sách…')}></div>{/if}
    {#each visibleErrors as [scope, error]}
      <p class="load-error" role="alert"><b>{providerLabel(parseRemoteScope(scope).provider)} · {cloudKindLabel(parseRemoteScope(scope).kind)}</b> · {tm(error)}</p>
    {/each}
    <div class="cols head-row"><span></span>{@render headerCell('name', t('Tên / ID'))}{@render headerCell('size', t('Dung lượng'))}{@render headerCell('status', t('Trạng thái'))}{@render headerCell('added', t('Ngày thêm'))}</div>
    <div class="body">
      {#if torrents.length === 0}
        <div class="empty">
          <span class="chip ic purple" style="width:48px;height:48px;border-radius:14px"><CloudArrowDown size={22} weight="duotone" /></span>
          <p class="caption">{view.loading ? t('Đang tải danh sách…') : !app.usableProviders.length ? t('Dán API token để xem torrent trên dịch vụ đã chọn.') : visibleErrors.length ? t('Không tải được danh sách. Bấm Tải lại để thử lại.') : t('Không có mục cloud nào trong bộ lọc này.')}</p>
        </div>
      {:else}
        {#key `${filter}|${kind}`}
        <VirtualList bind:this={listRef} items={torrents} rowHeight={60} key={remoteKey} onkeydown={keydown} label={t('Danh sách dữ liệu cloud')}>
          {#snippet row(torrent: RemoteRow)}
            <div class="cols item row-glass" class:on={view.selected.has(remoteKey(torrent))} role="option" aria-selected={view.selected.has(remoteKey(torrent))} tabindex="-1" onmousedown={selectionMouseDown} onclick={(e) => remote.pick(torrent, torrents, { ctrl: e.ctrlKey || e.metaKey, shift: e.shiftKey })} onkeydown={() => {}}>
              <input type="checkbox" checked={view.selected.has(remoteKey(torrent))} disabled={view.loading || view.reviewing !== null || view.deleting} onclick={(e) => { e.stopPropagation(); remote.toggle(torrent, e.shiftKey, torrents); }} aria-label={`${providerLabel(torrent.provider)} · ${torrent.account} · ${torrent.filename}`} />
              <div class="names">
                <b class="ell" title={torrent.filename}>{torrent.filename}</b>
                <span class="line"><span class="account ell" title={`${providerLabel(torrent.provider)} · ${torrent.account} · ${cloudKindLabel(torrent.kind ?? 'torrent')}`}>{providerLabel(torrent.provider)} · {torrent.account} · {cloudKindLabel(torrent.kind ?? 'torrent')}</span><span class="mono id">{torrent.id}</span>{#if queuedKeys.has(remoteKey(torrent))}<span class="pill info">{t('Trong hàng đợi')}</span>{/if}{#if view.failures[remoteKey(torrent)]}<span class="pill danger" title={tm(view.failures[remoteKey(torrent)])}>{tm(view.failures[remoteKey(torrent)])}</span>{/if}</span>
              </div>
              <span>{formatBytes(torrent.bytes)}</span>
              <span><span class="pill {tone(torrent.status)}" title={torrent.status}>{statusText(torrent)}</span></span>
              <span class="caption">{torrent.added ? new Date(torrent.added).toLocaleString(languageTag(app.settings.language), { dateStyle: 'short', timeStyle: 'short' }) : '—'}</span>
            </div>
          {/snippet}
        </VirtualList>
        {/key}
      {/if}
    </div>
    <footer class="foot">
      <button class="btn" disabled={view.loading || torrents.length === 0 || view.reviewing !== null || view.deleting} onclick={() => remote.toggleAll(torrents)}><span class="ic teal"><CheckSquare size={16} weight="duotone" /></span>{t('Chọn / bỏ chọn tất cả')}</button>
      <div class="actions">
        <button class="btn danger" disabled={view.loading || view.deleting || view.selected.size === 0} onclick={() => remote.review()}><span class="ic red"><Trash size={16} weight="duotone" /></span>{t('Xóa mục đã chọn…')}</button>
        <button class="btn danger" disabled={view.loading || view.deleting || torrents.length === 0 || visibleErrors.length > 0} onclick={() => remote.reviewAll(torrents)}><span class="ic red"><Trash size={16} weight="duotone" /></span>{t('Xóa tất cả cloud…')}</button>
      </div>
    </footer>
  </div>
</section>

{#if view.reviewing}
  <CloudDeleteDialog rows={view.reviewing} deleting={view.deleting} oncancel={() => remote.cancelReview()} onconfirm={confirmDelete} />
{/if}

<style>
  .page { display: flex; flex-direction: column; height: 100%; min-height: 0; }
  .head { display: flex; justify-content: space-between; align-items: flex-end; gap: 12px; margin-bottom: 16px; flex-wrap: wrap; }
  .actions { display: flex; align-items: center; gap: 4px; flex-wrap: wrap; }
  .account-summary { display: grid; gap: 12px; margin-bottom: 12px; flex: none; }
  .account-summary.multiple { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .table { position: relative; flex: 1; min-height: 0; display: flex; flex-direction: column; overflow: hidden; }
  .note { padding: 12px 16px; }
  .load-error { margin: 0 16px 8px; color: var(--danger); line-height: 1.5; }
  .cols { display: grid; grid-template-columns: 52px minmax(180px, 1fr) 100px 190px 136px; align-items: center; padding-right: 16px; }
  .head-row { height: 36px; margin: 0 8px; color: var(--muted); font-size: 11px; font-weight: 600; }
  .hcell { display: flex; align-items: center; gap: 5px; min-width: 0; height: 30px; padding: 0 6px; margin-left: -6px; border-radius: 8px; font: inherit; color: inherit; text-align: left; }
  .hcell:hover { background: var(--hover); color: var(--ink); }
  .hcell.active { color: var(--accent); }
  .body { flex: 1; min-height: 0; position: relative; }
  .item { height: 56px; }
  .item input { justify-self: center; }
  .names { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
  .ell { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .line { display: flex; gap: 8px; align-items: center; min-width: 0; }
  .account { color: var(--muted); font-size: 11px; }
  .line .pill { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .id { font-size: 11px; color: var(--muted); white-space: nowrap; flex: none; }
  .empty { position: absolute; inset: 0; display: flex; flex-direction: column; gap: 12px; align-items: center; justify-content: center; text-align: center; padding: 24px; }
  .foot { display: flex; align-items: center; justify-content: space-between; padding: 12px 16px; gap: 8px; flex-wrap: wrap; }
  .indeterminate { position: absolute; left: 0; right: 0; top: 40px; height: 3px; background: linear-gradient(90deg, transparent, var(--accent), transparent); background-size: 40% 100%; background-repeat: no-repeat; animation: slide 1.2s linear infinite; }
  @keyframes slide { from { background-position: -40% 0; } to { background-position: 140% 0; } }
</style>
