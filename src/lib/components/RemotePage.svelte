<script lang="ts">
  import ArrowClockwise from 'phosphor-svelte/lib/ArrowClockwise';
  import CloudArrowDown from 'phosphor-svelte/lib/CloudArrowDown';
  import CheckSquare from 'phosphor-svelte/lib/CheckSquare';
  import CaretUp from 'phosphor-svelte/lib/CaretUp';
  import CaretDown from 'phosphor-svelte/lib/CaretDown';
  import DownloadSimple from 'phosphor-svelte/lib/DownloadSimple';
  import Trash from 'phosphor-svelte/lib/Trash';
  import { onDestroy, untrack } from 'svelte';
  import { api } from '../api';
  import { app } from '../app.svelte';
  import { formatBytes } from '../browser';
  import { askDestination } from '../destination.svelte';
  import { languageTag } from '../languages';
  import { t, tm } from '../i18n.svelte';
  import { emptyRemoteState, nextRemoteSort, parseRemoteScope, queueableRemoteRows, RemoteBrowser, remoteFolders, remoteKey, remoteQueuedKeys, sortRemoteFolders, sortRemoteRows, type RemoteFolder, type RemoteRow, type RemoteSort, type RemoteSortColumn } from '../remote';
  import { CLOUD_KINDS, cloudKindLabel, providerLabel, type CloudFilter, type Provider, type RemoteTorrent } from '../types';
  import TorBoxAccount from './TorBoxAccount.svelte';
  import RealDebridAccount from './RealDebridAccount.svelte';
  import Select from './Select.svelte';
  import VirtualList from './VirtualList.svelte';
  import { selectionMouseDown } from '../selection';
  import CloudDeleteDialog from './CloudDeleteDialog.svelte';
  import ContextMenu, { type MenuItem } from './ContextMenu.svelte';
  import Copy from 'phosphor-svelte/lib/Copy';
  import Hash from 'phosphor-svelte/lib/Hash';
  import { copyText } from '../clipboard';
  import ListChecks from 'phosphor-svelte/lib/ListChecks';
  import FolderSimple from 'phosphor-svelte/lib/FolderSimple';
  import FolderOpen from 'phosphor-svelte/lib/FolderOpen';
  import ArrowUp from 'phosphor-svelte/lib/ArrowUp';
  import House from 'phosphor-svelte/lib/House';
  import ListBullets from 'phosphor-svelte/lib/ListBullets';
  import { selectionControl } from '../selection';

  let view = $state.raw(emptyRemoteState());
  let filter = $state<Provider | 'all'>('all');
  let kind = $state<CloudFilter>('all');
  let sort = $state<RemoteSort>(null);
  let listRef: { reveal: (index: number) => void } | undefined = $state();
  const remote = new RemoteBrowser(api, (next) => (view = next));
  const torrents = $derived(sortRemoteRows(view.rows.filter((row) => filter === 'all' || row.provider === filter), sort, languageTag(app.settings.language)));

  // "All services" opens on one virtual folder per service; picking a service opens its folder. The flat view keeps
  // the single newest-first list across services. The choice is remembered on this device only.
  const FLAT_KEY = 'cloud.flatView';
  let flat = $state(recallFlat());
  function recallFlat() { try { return localStorage.getItem(FLAT_KEY) === '1'; } catch { return false; } }
  function setFlat(value: boolean) {
    flat = value;
    try { localStorage.setItem(FLAT_KEY, value ? '1' : '0'); } catch { /* storage may be blocked */ }
    listRef?.reveal(0);
  }
  const atRoot = $derived(filter === 'all' && !flat);
  const folders = $derived(atRoot ? sortRemoteFolders(remoteFolders(view.rows, app.usableProviders, kind, view.errors), sort, languageTag(app.settings.language)) : []);
  let cursor = $state(0);
  const folderCheck = (folder: RemoteFolder) => {
    const picked = folder.rows.filter((row) => view.selected.has(remoteKey(row))).length;
    return { checked: picked > 0 && picked === folder.rows.length, indeterminate: picked > 0 && picked < folder.rows.length };
  };
  const busy = $derived(view.loading || view.reviewing !== null || view.deleting);

  function open(provider: Provider) {
    if (changeFilter(provider)) listRef?.reveal(0);
  }
  /** The parent of a service is "All services" in the remembered view: its folder list, or the flat list. */
  function goUp() {
    const from = filter;
    if (from === 'all' || !changeFilter('all')) return;
    cursor = Math.max(0, folders.findIndex((folder) => folder.provider === from));
    listRef?.reveal(cursor);
  }
  /** House and the first crumb go to the service folders without changing the remembered view. */
  function goHome() {
    if (changeFilter('all')) flat = false;
  }
  function folderClick(folder: RemoteFolder, index: number, event: MouseEvent) {
    cursor = index;
    remote.pickRows(folder.rows, event.ctrlKey || event.metaKey || event.shiftKey ? 'toggle' : 'replace');
  }
  const queuedKeys = $derived(remoteQueuedKeys(app.jobs));
  const selectedRows = $derived(torrents.filter((row) => view.selected.has(remoteKey(row))));
  const queueable = $derived(queueableRemoteRows(selectedRows, queuedKeys));
  let queueing = $state(false);
  let disposed = false;
  const visibleErrors = $derived(Object.entries(view.errors).filter(([scope]) => filter === 'all' || parseRemoteScope(scope).provider === filter));
  const reviewTargets = $derived([...new Set(view.reviewing?.map((row) => `${providerLabel(row.provider)} · ${row.account} · ${cloudKindLabel(row.kind ?? 'torrent')}`) ?? [])].join(', '));
  const totalBytes = $derived(torrents.reduce((sum, row) => sum + row.bytes, 0));
  const showRealDebrid = $derived((filter === 'all' || filter === 'realDebrid') && app.accounts.some((account) => account.provider === 'realDebrid' && account.hasToken));
  const showTorBox = $derived((filter === 'all' || filter === 'torBox') && app.accounts.some((account) => account.provider === 'torBox' && account.hasToken));

  // Right-click acts on the selection; a row outside it becomes the only selected row first.
  let menu: { x: number; y: number; folder?: Provider } | null = $state(null);
  function openMenu(row: RemoteRow, event: MouseEvent) {
    event.preventDefault();
    if (view.loading || view.reviewing || view.deleting) return;
    if (!view.selected.has(remoteKey(row))) remote.pick(row, torrents, {});
    menu = { x: event.clientX, y: event.clientY };
  }
  function openFolderMenu(folder: RemoteFolder, index: number, event: MouseEvent) {
    event.preventDefault();
    if (view.loading || view.reviewing || view.deleting) return;
    cursor = index;
    if (!folderCheck(folder).checked) remote.pickRows(folder.rows, 'replace');
    menu = { x: event.clientX, y: event.clientY, folder: folder.provider };
  }
  const downloadedCount = $derived(torrents.filter((row) => row.status === 'downloaded').length);

  function menuItems(): MenuItem[] {
    const list = selectedRows;
    const folder = menu?.folder;
    // From a service folder, "select finished" stays inside that service.
    const scope = folder ? torrents.filter((row) => row.provider === folder) : torrents;
    const scopeDone = scope.filter((row) => row.status === 'downloaded');
    return [
      ...(folder ? [{ label: t('Mở'), icon: FolderOpen, tone: 'amber', action: () => open(folder) } as const, 'separator' as const] : []),
      { label: t('Thêm vào hàng đợi'), icon: DownloadSimple, tone: 'blue', disabled: queueing || queueable.length === 0, action: queueSelected },
      'separator',
      { label: t('Sao chép tên'), icon: Copy, tone: 'teal', action: () => copyText(list.map((row) => row.filename).join('\n'), { key: 'Đã sao chép {0} tên.', args: [String(list.length)] }) },
      { label: t('Sao chép ID'), icon: Hash, tone: 'purple', action: () => copyText(list.map((row) => row.id).join('\n'), { key: 'Đã sao chép {0} ID.', args: [String(list.length)] }) },
      'separator',
      { label: t('Chọn các mục đã tải xong ({0})', scopeDone.length), icon: ListChecks, tone: 'green', disabled: scopeDone.length === 0, action: () => remote.selectDownloaded(scope) },
      'separator',
      { label: t('Xóa mục đã chọn…'), icon: Trash, tone: 'red', danger: true, disabled: list.length === 0, action: () => remote.review() }
    ];
  }

  function load() { return remote.load(app.usableProviders, kind); }

  function sortBy(column: RemoteSortColumn) {
    if (view.reviewing || view.deleting) return;
    sort = nextRemoteSort(sort, column);
    listRef?.reveal(0);
  }

  /** Backspace or Alt+Up/Left leaves a service folder; the flat list has no parent. */
  function leavesFolder(event: KeyboardEvent) {
    if (filter === 'all' || selectionControl(event)) return false;
    return (event.key === 'Backspace' && !event.altKey && !event.ctrlKey && !event.metaKey) || (event.altKey && (event.key === 'ArrowUp' || event.key === 'ArrowLeft'));
  }

  /** Arrows move between service folders, Enter opens one and Space selects its items; Ctrl+A and Esc act on items. */
  function folderKeydown(event: KeyboardEvent) {
    if (event.altKey || selectionControl(event)) return;
    const ctrl = event.ctrlKey || event.metaKey;
    if ((ctrl && event.key.toLowerCase() === 'a') || event.key === 'Escape') return keydown(event);
    const folder = folders[cursor];
    if (event.key === 'Enter' || event.key === ' ') {
      if (!folder) return;
      event.preventDefault();
      if (event.key === 'Enter') open(folder.provider);
      else remote.pickRows(folder.rows, 'toggle');
      return;
    }
    const step = ({ ArrowDown: 1, ArrowUp: -1 } as Record<string, number>)[event.key];
    if (step === undefined && event.key !== 'Home' && event.key !== 'End') return;
    event.preventDefault();
    cursor = event.key === 'Home' ? 0 : event.key === 'End' ? folders.length - 1 : Math.max(0, Math.min(folders.length - 1, cursor + step));
    listRef?.reveal(cursor);
    (event.currentTarget as HTMLElement).focus();
  }

  function keydown(event: KeyboardEvent) {
    if (leavesFolder(event)) {
      event.preventDefault();
      goUp();
      return;
    }
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
  onDestroy(() => { disposed = true; remote.invalidate(); });

  const tone = (status: string) => (status === 'downloaded' ? 'success' : ['error', 'magnet_error', 'virus', 'dead'].includes(status) ? 'danger' : ['downloading', 'queued', 'magnet_conversion', 'compressing', 'uploading', 'waiting_files_selection'].includes(status) ? 'info' : '');
  const statusText = (torrent: RemoteTorrent) => `${torrent.status.replaceAll('_', ' ')}${torrent.status === 'downloaded' ? '' : ` ${torrent.progress.toFixed(0)}%`}`;

  function changeFilter(value: Provider | 'all') {
    if (view.deleting || view.reviewing) return false;
    filter = value;
    remote.clearSelection();
    return true;
  }

  async function confirmDelete() {
    const targets = reviewTargets;
    const count = await remote.confirmDelete();
    if (count) app.toast({ key: 'Đã xóa {0} mục cloud trên {1}.', args: [String(count), targets] }, 'success');
    if (count) void app.refreshTorboxAccount(true);
  }

  async function queueSelected() {
    if (busy || queueing || !queueable.length) return;
    const selected = new Set(queueable.map(remoteKey));
    queueing = true;
    try {
      const directory = await askDestination({ directory: app.settings.outputDirectory });
      if (directory === null || disposed) return;
      const rows = queueableRemoteRows(view.rows.filter((row) => selected.has(remoteKey(row))), remoteQueuedKeys(app.jobs));
      if (rows.length === 0) {
        app.toast({ key: 'Không có file mới được thêm vào hàng đợi.', args: [] }, 'warning');
        return;
      }
      const result = await app.run(() => api.remoteQueue(rows.map(({ provider, account, kind = 'torrent', id, filename, bytes, progress }) => ({ provider, account, kind, id, filename, bytes, progress })), directory));
      if (!result) return;
      for (const error of result.errors) app.toast(error, 'error');
      if (result.created.length) {
        app.toast({ key: 'Đã thêm {0} mục cloud vào hàng đợi.', args: [String(result.created.length)] }, 'success');
        await app.run(() => api.queueStart(result.created));
      } else if (!result.errors.length) {
        app.toast({ key: 'Không có file mới được thêm vào hàng đợi.', args: [] }, 'warning');
      }
    } finally {
      queueing = false;
    }
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
    <div class="toolbar">
      <div class="nav">
        <button class="btn icon" onclick={goUp} disabled={filter === 'all' || view.reviewing !== null || view.deleting} title={t('Lên một cấp (Backspace)')} aria-label={t('Lên một cấp')}><ArrowUp size={16} /></button>
        <button class="btn icon" onclick={goHome} disabled={atRoot || view.reviewing !== null || view.deleting} title={t('Tất cả dịch vụ')} aria-label={t('Tất cả dịch vụ')}><House size={16} /></button>
      </div>
      <nav class="crumbs" aria-label="Path">
        <button class="crumb" class:last={atRoot} disabled={view.reviewing !== null || view.deleting} onclick={goHome}>{t('Tất cả dịch vụ')}</button>
        {#if filter !== 'all'}<span class="sep">›</span><b class="crumb last">{providerLabel(filter)}</b>
        {:else if flat}<span class="sep">›</span><b class="crumb last">{t('Tất cả mục')}</b>{/if}
      </nav>
      <div class="seg" role="group" aria-label={t('Kiểu xem cloud')}>
        <button class="btn icon" class:on={!flat} aria-pressed={!flat} disabled={view.reviewing !== null || view.deleting} onclick={() => setFlat(false)} title={t('Nhóm theo dịch vụ')} aria-label={t('Nhóm theo dịch vụ')}><span class="ic amber"><FolderSimple size={17} weight={flat ? 'regular' : 'fill'} /></span></button>
        <button class="btn icon" class:on={flat} aria-pressed={flat} disabled={view.reviewing !== null || view.deleting} onclick={() => { if (changeFilter('all')) setFlat(true); }} title={t('Xem phẳng mọi mục')} aria-label={t('Tất cả mục')}><span class="ic blue"><ListBullets size={17} weight={flat ? 'bold' : 'regular'} /></span></button>
      </div>
    </div>
    <div class="cols head-row"><span></span>{@render headerCell('name', t('Tên / ID'))}{@render headerCell('size', t('Dung lượng'))}{@render headerCell('status', t('Trạng thái'))}{@render headerCell('added', t('Ngày thêm'))}</div>
    <div class="body">
      {#if atRoot && folders.length > 0}
        {#key `root|${kind}`}
        <VirtualList bind:this={listRef} items={folders} rowHeight={60} key={(folder) => folder.key} onkeydown={folderKeydown} label={t('Thư mục dịch vụ')}>
          {#snippet row(folder: RemoteFolder, index: number)}
            {@const check = folderCheck(folder)}
            <div class="cols item row-glass" class:on={check.checked} class:cursor={index === cursor} role="option" aria-selected={check.checked} tabindex="-1" onmousedown={selectionMouseDown} onclick={(e) => folderClick(folder, index, e)} ondblclick={() => open(folder.provider)} oncontextmenu={(e) => openFolderMenu(folder, index, e)} onkeydown={() => {}}>
              <input type="checkbox" checked={check.checked} indeterminate={check.indeterminate} disabled={busy || folder.rows.length === 0} onclick={(e) => { e.stopPropagation(); cursor = index; remote.pickRows(folder.rows, 'toggle'); }} aria-label={providerLabel(folder.provider)} />
              <div class="folder">
                <span class="ic amber"><FolderSimple size={22} weight="duotone" /></span>
                <div class="names">
                  <b class="ell">{providerLabel(folder.provider)}</b>
                  <span class="line"><span class="account ell">{[...folder.accounts, t('{0} mục', folder.rows.length)].join(' · ')}</span>{#if folder.failed}<span class="pill danger">{t('Không tải được')}</span>{/if}</span>
                </div>
              </div>
              <span>{formatBytes(folder.bytes)}</span>
              <span>{#if view.loading && folder.rows.length === 0 && !folder.failed}<span class="caption">{t('Đang tải danh sách…')}</span>{:else}<span class="pill {folder.rows.length > 0 && folder.done === folder.rows.length ? 'success' : ''}">{t('Xong {0}/{1}', folder.done, folder.rows.length)}</span>{/if}</span>
              <span class="caption">{folder.newest ? new Date(folder.newest).toLocaleString(languageTag(app.settings.language), { dateStyle: 'short', timeStyle: 'short' }) : '—'}</span>
            </div>
          {/snippet}
        </VirtualList>
        {/key}
      {:else if torrents.length === 0}
        <div class="empty">
          <span class="chip ic purple" style="width:48px;height:48px;border-radius:14px"><CloudArrowDown size={22} weight="duotone" /></span>
          <p class="caption">{view.loading ? t('Đang tải danh sách…') : !app.usableProviders.length ? t('Dán API token để xem torrent trên dịch vụ đã chọn.') : visibleErrors.length ? t('Không tải được danh sách. Bấm Tải lại để thử lại.') : t('Không có mục cloud nào trong bộ lọc này.')}</p>
        </div>
      {:else}
        {#key `${filter}|${kind}|${flat}`}
        <VirtualList bind:this={listRef} items={torrents} rowHeight={60} key={remoteKey} onkeydown={keydown} label={t('Danh sách dữ liệu cloud')}>
          {#snippet row(torrent: RemoteRow)}
            <div class="cols item row-glass" class:on={view.selected.has(remoteKey(torrent))} role="option" aria-selected={view.selected.has(remoteKey(torrent))} tabindex="-1" onmousedown={selectionMouseDown} onclick={(e) => remote.pick(torrent, torrents, { ctrl: e.ctrlKey || e.metaKey, shift: e.shiftKey })} oncontextmenu={(e) => openMenu(torrent, e)} onkeydown={() => {}}>
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
      <div class="actions">
      <button class="btn" disabled={view.loading || torrents.length === 0 || view.reviewing !== null || view.deleting} onclick={() => remote.toggleAll(torrents)}><span class="ic teal"><CheckSquare size={16} weight="duotone" /></span>{t('Chọn / bỏ chọn tất cả')}</button>
      <button class="btn" disabled={view.loading || downloadedCount === 0 || view.reviewing !== null || view.deleting} onclick={() => remote.selectDownloaded(torrents)} title={t('Chọn các mục dịch vụ đã tải xong')}><span class="ic green"><ListChecks size={16} weight="duotone" /></span>{t('Chọn các mục đã tải xong ({0})', downloadedCount)}</button>
      </div>
      <div class="actions">
        <button class="btn primary" disabled={busy || queueing || queueable.length === 0} onclick={queueSelected}><DownloadSimple size={16} weight="bold" />{t('Thêm vào hàng đợi')}</button>
        <button class="btn danger" disabled={view.loading || view.deleting || view.selected.size === 0} onclick={() => remote.review()}><span class="ic red"><Trash size={16} weight="duotone" /></span>{t('Xóa mục đã chọn…')}</button>
        <button class="btn danger" disabled={view.loading || view.deleting || torrents.length === 0 || visibleErrors.length > 0} onclick={() => remote.reviewAll(torrents)}><span class="ic red"><Trash size={16} weight="duotone" /></span>{t('Xóa tất cả cloud…')}</button>
      </div>
    </footer>
  </div>
</section>

{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={menuItems()} onclose={() => (menu = null)} />
{/if}

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
  .toolbar { display: flex; align-items: center; gap: 8px; padding: 0 16px 8px; }
  .nav { display: flex; }
  .crumbs { flex: 1; min-width: 0; display: flex; align-items: center; overflow: hidden; white-space: nowrap; }
  .crumb { padding: 4px 9px; border-radius: 999px; color: var(--muted); font-size: 12px; font-weight: 600; max-width: 260px; overflow: hidden; text-overflow: ellipsis; }
  button.crumb:hover:not(:disabled) { background: var(--hover); color: var(--ink); }
  .crumb.last { color: var(--ink); font-size: 14px; }
  .sep { color: var(--muted); }
  .seg { display: flex; gap: 2px; padding: 3px; border-radius: 999px; background: var(--subtle); }
  .seg .btn.icon { min-height: 28px; width: 32px; }
  .seg .btn.on, .seg .btn.on:hover:not(:disabled) { background: var(--chosen-bg); color: var(--chosen-ink); }
  .seg .btn.on .ic { color: var(--chosen-icon); }
  .folder { display: flex; align-items: center; gap: 10px; min-width: 0; }
  :global(.viewport:focus-within) .item.cursor { box-shadow: inset 0 0 0 1.5px color-mix(in srgb, var(--accent) 55%, transparent); }
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
