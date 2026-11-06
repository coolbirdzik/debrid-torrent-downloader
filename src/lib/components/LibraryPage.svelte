<script lang="ts">
  import { open as pickFiles } from '@tauri-apps/plugin-dialog';
  import ArrowLeft from 'phosphor-svelte/lib/ArrowLeft';
  import ArrowRight from 'phosphor-svelte/lib/ArrowRight';
  import ArrowUp from 'phosphor-svelte/lib/ArrowUp';
  import ArrowCounterClockwise from 'phosphor-svelte/lib/ArrowCounterClockwise';
  import ArrowsClockwise from 'phosphor-svelte/lib/ArrowsClockwise';
  import CheckSquare from 'phosphor-svelte/lib/CheckSquare';
  import ClipboardText from 'phosphor-svelte/lib/ClipboardText';
  import FilePlus from 'phosphor-svelte/lib/FilePlus';
  import FolderSimple from 'phosphor-svelte/lib/FolderSimple';
  import FilmSlate from 'phosphor-svelte/lib/FilmSlate';
  import MusicNotes from 'phosphor-svelte/lib/MusicNotes';
  import ImageIcon from 'phosphor-svelte/lib/Image';
  import FileZip from 'phosphor-svelte/lib/FileZip';
  import FileText from 'phosphor-svelte/lib/FileText';
  import File from 'phosphor-svelte/lib/File';
  import House from 'phosphor-svelte/lib/House';
  import MagnifyingGlass from 'phosphor-svelte/lib/MagnifyingGlass';
  import Plus from 'phosphor-svelte/lib/Plus';
  import Trash from 'phosphor-svelte/lib/Trash';
  import User from 'phosphor-svelte/lib/User';
  import X from 'phosphor-svelte/lib/X';
  import DownloadSimple from 'phosphor-svelte/lib/DownloadSimple';
  import ListBullets from 'phosphor-svelte/lib/ListBullets';
  import SquaresFour from 'phosphor-svelte/lib/SquaresFour';
  import ArrowsDownUp from 'phosphor-svelte/lib/ArrowsDownUp';
  import CaretDown from 'phosphor-svelte/lib/CaretDown';
  import CaretUp from 'phosphor-svelte/lib/CaretUp';
  import Check from 'phosphor-svelte/lib/Check';
  import TextAa from 'phosphor-svelte/lib/TextAa';
  import SortAscending from 'phosphor-svelte/lib/SortAscending';
  import SortDescending from 'phosphor-svelte/lib/SortDescending';
  import { app } from '../app.svelte';
  import { breadcrumbs, formatBytes, isStatusDone, isStatusPartial, type Entry, type HeaderSort } from '../browser';
  import type { Settings } from '../types';
  import { confirmAction } from '../confirm.svelte';
  import { t } from '../i18n.svelte';
  import { library } from '../library.svelte';
  import { api } from '../api';
  import VirtualList from './VirtualList.svelte';

  const colors: Record<string, string> = { folder: 'amber', video: 'purple', audio: 'pink', image: 'green', archive: 'orange', document: 'blue', other: '' };
  const crumbs = $derived(breadcrumbs(library.location, library.drafts));
  const grid = $derived(app.settings.browserView === 'grid');
  const sources = $derived(countSources(library.input));
  let listRef: { reveal: (index: number) => void } | undefined = $state();

  function countSources(text: string): number {
    const found = text.match(/magnet:\?[^\s]+|https?:\/\/[^\s"]+|[A-Za-z]:\\[^\r\n"]*\.torrent|\/[^\r\n"]*\.torrent/gi) ?? [];
    return new Set(found.map((value) => value.toLowerCase())).size;
  }

  function mark(entry: Entry): string {
    return entry.pending ? '…' : isStatusDone(entry) ? '✓' : isStatusPartial(entry) ? '◐' : '○';
  }

  function statusLabel(entry: Entry): string {
    const state = entry.pending ? t('Chờ đọc') : isStatusDone(entry) ? t('Đã tải xong') : isStatusPartial(entry) ? t('Đã tải một phần') : t('Chưa tải');
    if (entry.kind === 'folder' && !entry.pending) return t('{0}/{1} file · {2}', entry.downloaded, entry.total, state);
    return !isStatusDone(entry) && !isStatusPartial(entry) && !entry.pending && entry.queued ? t('Đã có trong hàng đợi') : state;
  }

  const sizeLabel = (entry: Entry) => (entry.kind === 'folder' ? t('{0} file', entry.fileCount) : formatBytes(entry.size));
  const dateLabel = (entry: Entry) => (entry.addedAt ? new Date(entry.addedAt).toLocaleString(app.settings.language === 'vi' ? 'vi-VN' : 'en-US', { dateStyle: 'short', timeStyle: 'short' }) : '—');

  async function paste() {
    try {
      const text = await navigator.clipboard.readText();
      if (text) library.input = (library.input.trim() ? library.input.trimEnd() + '\n' : '') + text;
    } catch {
      app.toast({ key: 'Không đọc được clipboard.', args: [] }, 'error');
    }
  }

  async function chooseFiles() {
    const picked = await pickFiles({ multiple: true, filters: [{ name: 'Torrent', extensions: ['torrent'] }] });
    if (picked) library.appendSources(Array.isArray(picked) ? picked : [picked]);
  }

  async function newBatch() {
    const sure = await confirmAction({
      title: { key: 'Batch mới', args: [] },
      body: { key: 'Xóa toàn bộ thư viện ảo, ô nhập và bộ lọc để thêm batch mới. Giữ hàng đợi, file trên máy và torrent trên dịch vụ.', args: [] },
      confirm: { key: 'Batch mới', args: [] }
    });
    if (sure) await library.newBatch();
  }

  function keydown(event: KeyboardEvent) {
    const ctrl = event.ctrlKey || event.metaKey;
    if (ctrl && event.key.toLowerCase() === 'a') { event.preventDefault(); library.toggleAll(); }
    else if (ctrl && event.key.toLowerCase() === 'z') { event.preventDefault(); void library.undo(); }
    else if (event.key === 'Delete') { event.preventDefault(); void library.removeSelected(); }
    else if (event.key === 'Enter') { const target = library.selectedEntries[0]; if (target) library.open(target); }
    else if (event.key === 'Backspace' || (event.altKey && event.key === 'ArrowLeft')) { event.preventDefault(); event.altKey ? library.goBack() : library.goUp(); }
    else if (event.altKey && event.key === 'ArrowRight') { event.preventDefault(); library.goForward(); }
    else if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      const entries = library.entries;
      const current = library.anchor ? entries.findIndex((e) => e.key === library.anchor) : -1;
      const next = Math.max(0, Math.min(entries.length - 1, current + (event.key === 'ArrowDown' ? 1 : -1)));
      if (entries[next]) { library.select(entries[next], { ctrl: false, shift: event.shiftKey }); listRef?.reveal(next); }
    }
  }

  // Mouse side buttons move through the library history, as in a file explorer. A plain mouse sends them as buttons
  // 3 (back) and 4 (forward); mouse software (Logitech, Razer…) often sends the keyboard keys BrowserBack/BrowserForward
  // instead. Cancelling both mousedown and mouseup also stops the webview from running its own history navigation.
  function mouseNav(event: MouseEvent) {
    if (event.button !== 3 && event.button !== 4) return;
    event.preventDefault();
    if (event.type !== 'mouseup') return;
    if (event.button === 3) library.goBack();
    else library.goForward();
  }

  // Sort menu: the toolbar order (name / newest / oldest). A column header sort, when set, takes precedence.
  const sorts: { value: Settings['browserSort']; label: string; short: string }[] = $derived([
    { value: 'name', label: t('Theo tên'), short: t('Tên') },
    { value: 'added-desc', label: t('Ngày thêm: mới nhất'), short: t('Mới nhất') },
    { value: 'added-asc', label: t('Ngày thêm: cũ nhất'), short: t('Cũ nhất') }
  ]);
  const currentSort = $derived(sorts.find((s) => s.value === app.settings.browserSort) ?? sorts[0]);
  const columnLabels = $derived({ name: t('Tên file'), size: t('Dung lượng'), status: t('Loại / Trạng thái') });
  const sortShort = $derived(library.header ? columnLabels[library.header.column] : currentSort.short);
  let sortOpen = $state(false);
  let sortBox: HTMLDivElement | undefined = $state();

  function chooseSort(value: Settings['browserSort']) {
    sortOpen = false;
    library.header = null;
    void app.setSettings({ browserSort: value });
  }

  function closeSortOutside(event: PointerEvent) {
    if (sortOpen && sortBox && !sortBox.contains(event.target as Node)) sortOpen = false;
  }

  /** Header click cycles ascending → descending → off. */
  function sortBy(column: NonNullable<HeaderSort>['column']) {
    const now = library.header;
    library.header = now?.column !== column ? { column, direction: 'asc' } : now.direction === 'asc' ? { column, direction: 'desc' } : null;
  }

  function browserKeys(event: KeyboardEvent) {
    if (event.key === 'Escape' && sortOpen) { sortOpen = false; return; }
    if (event.key !== 'BrowserBack' && event.key !== 'BrowserForward') return;
    event.preventDefault();
    if (event.key === 'BrowserBack') library.goBack();
    else library.goForward();
  }

  function rowClick(entry: Entry, event: MouseEvent) {
    library.select(entry, { ctrl: event.ctrlKey || event.metaKey, shift: event.shiftKey });
  }

  const gridTitle = (entry: Entry) => `${entry.name}\n${entry.location}`;
</script>

<svelte:window onmousedown={mouseNav} onmouseup={mouseNav} onkeydown={browserKeys} onpointerdown={closeSortOutside} />

{#snippet headerCell(column: NonNullable<HeaderSort>['column'], label: string)}
  {@const active = library.header?.column === column}
  <button class="hcell" class:active onclick={() => sortBy(column)} title={t('Sắp xếp theo cột {0}. Bấm lần nữa để đảo chiều, lần thứ ba để bỏ.', label)}>
    {label}
    {#if active && library.header?.direction === 'asc'}<CaretUp size={12} weight="bold" />{:else if active}<CaretDown size={12} weight="bold" />{/if}
  </button>
{/snippet}

{#snippet kindIcon(entry: Entry, size: number)}
  {#if entry.videoKind === 'folder'}<FolderSimple {size} weight="duotone" />
  {:else if entry.videoKind === 'video'}<FilmSlate {size} weight="duotone" />
  {:else if entry.videoKind === 'audio'}<MusicNotes {size} weight="duotone" />
  {:else if entry.videoKind === 'image'}<ImageIcon {size} weight="duotone" />
  {:else if entry.videoKind === 'archive'}<FileZip {size} weight="duotone" />
  {:else if entry.videoKind === 'document'}<FileText {size} weight="duotone" />
  {:else}<File {size} weight="duotone" />{/if}
{/snippet}

<section class="page">
  <header class="head">
    <div>
      <h1 class="title">{t('Thư viện')}</h1>
      <p class="caption">{t('Thư viện: {0} torrent · {1} đã đọc · {2} chờ đọc · {3} thư mục con', library.drafts.length, library.readyCount, library.pendingCount, library.subfolders)}</p>
    </div>
    <div class="actions">
      {#if !app.hasToken}
        <button class="btn" onclick={() => (app.page = 'settings')}><span class="ic blue"><User size={16} weight="duotone" /></span>{t('Kết nối tài khoản')}</button>
      {/if}
      {#if library.pendingCount > 0}
        <button class="btn" onclick={() => api.libraryRetry()}><span class="ic blue"><ArrowsClockwise size={16} weight="duotone" /></span>{t('Thử lại tất cả ({0})', library.pendingCount)}</button>
      {/if}
      {#if library.canRemoveFolder}
        <button class="btn danger" onclick={() => library.removeFolder()}><span class="ic red"><Trash size={16} weight="duotone" /></span>{t('Bỏ thư mục')}</button>
      {/if}
      <button class="btn" onclick={() => library.showAll()}><span class="ic purple"><FileText size={16} weight="duotone" /></span>{t('Tất cả file')}</button>
      <button class="btn" disabled={library.drafts.length === 0 && !library.input} onclick={newBatch}><span class="ic green"><Plus size={16} weight="duotone" /></span>{t('Batch mới')}</button>
    </div>
  </header>

  <div class="add card">
    <b class="label">{t('Thêm torrent')}</b>
    <div class="add-grid">
      <div class="input">
        <textarea rows="3" bind:value={library.input} placeholder={t('magnet:?xt=urn:btih:…\nHoặc kéo thả các file .torrent vào cửa sổ')} aria-label={t('Danh sách torrent')} spellcheck="false"></textarea>
        <div class="row">
          <span class="caption">{t('Ô nhập: {0} torrent', sources)}</span>
          <span class="spacer"></span>
          <button class="btn" onclick={paste}><span class="ic blue"><ClipboardText size={16} weight="duotone" /></span>{t('Dán clipboard')}</button>
          <button class="btn" onclick={chooseFiles}><span class="ic orange"><FilePlus size={16} weight="duotone" /></span>{t('Chọn file .torrent…')}</button>
        </div>
      </div>
      <div class="go">
        {#if app.inspect.running}
          <button class="btn outline" onclick={() => api.libraryCancelInspect()}><span class="ic red"><X size={16} weight="bold" /></span>{t('Dừng đọc')}</button>
        {/if}
        <button class="btn primary" disabled={sources === 0} onclick={() => library.add()}><Plus size={16} weight="bold" />{t('Thêm vào thư viện')}</button>
      </div>
    </div>
    {#if app.inspect.running}<div class="indeterminate" role="progressbar" aria-label={t('Đang đọc torrent…')}></div>{/if}
  </div>

  <div class="browser card">
    <div class="toolbar">
      <div class="nav">
        <button class="btn icon" onclick={() => library.goBack()} disabled={library.back.length === 0} title="{t('Quay lại (Alt+Left)')}" aria-label={t('Quay lại')}><ArrowLeft size={16} /></button>
        <button class="btn icon" onclick={() => library.goForward()} disabled={library.forward.length === 0} title="{t('Tiến tới (Alt+Right)')}" aria-label={t('Tiến tới')}><ArrowRight size={16} /></button>
        <button class="btn icon" onclick={() => library.goUp()} disabled={!library.location.draft} title="{t('Lên một cấp (Backspace)')}" aria-label={t('Lên một cấp')}><ArrowUp size={16} /></button>
        <button class="btn icon" onclick={() => library.navigate({ draft: null, path: '', flat: false })} title={t('Tất cả torrent')} aria-label={t('Tất cả torrent')}><House size={16} /></button>
      </div>
      <nav class="crumbs" aria-label="Path">
        {#each crumbs as crumb, index (index)}
          {#if index > 0}<span class="sep">›</span>{/if}
          <button class="crumb" class:last={index === crumbs.length - 1 && !library.location.flat} onclick={() => library.navigate(crumb.location)}>{crumb.label ?? t('Tất cả torrent')}</button>
        {/each}
        {#if library.location.flat}<span class="sep">›</span><b class="crumb last">{t('Tất cả file')}</b>{/if}
      </nav>
      <div class="sort" bind:this={sortBox}>
        <button class="btn outline sort-btn" onclick={() => (sortOpen = !sortOpen)} aria-haspopup="menu" aria-expanded={sortOpen} aria-label={t('Sắp xếp thư viện')} title={t('Sắp xếp: {0}', library.header ? sortShort : currentSort.label)}>
          <span class="ic blue"><ArrowsDownUp size={16} weight="bold" /></span>{sortShort}<CaretDown size={12} weight="bold" />
        </button>
        {#if sortOpen}
          <div class="menu card menu-glass" role="menu" aria-label={t('Sắp xếp thư viện')}>
            {#each sorts as sort (sort.value)}
              <button class="menu-item" role="menuitemradio" aria-checked={sort.value === app.settings.browserSort} onclick={() => chooseSort(sort.value)}>
                <span class="ic {sort.value === 'name' ? 'purple' : 'teal'}">
                  {#if sort.value === 'name'}<TextAa size={16} weight="bold" />{:else if sort.value === 'added-desc'}<SortDescending size={16} weight="bold" />{:else}<SortAscending size={16} weight="bold" />{/if}
                </span>
                <span class="grow">{sort.label}</span>
                {#if sort.value === app.settings.browserSort && !library.header}<Check size={14} weight="bold" />{/if}
              </button>
            {/each}
          </div>
        {/if}
      </div>
      <div class="seg" role="group" aria-label={t('Kiểu xem file')}>
        <button class="btn icon" class:on={!grid} aria-pressed={!grid} onclick={() => app.setSettings({ browserView: 'list' })} title={t('Xem dạng danh sách')} aria-label={t('Danh sách')}><span class="ic blue"><ListBullets size={17} weight={grid ? 'regular' : 'bold'} /></span></button>
        <button class="btn icon" class:on={grid} aria-pressed={grid} onclick={() => app.setSettings({ browserView: 'grid' })} title={t('Xem dạng lưới')} aria-label={t('Lưới')}><span class="ic orange"><SquaresFour size={17} weight={grid ? 'fill' : 'regular'} /></span></button>
      </div>
    </div>
    <div class="toolbar second">
      <div class="search">
        <span class="ic"><MagnifyingGlass size={16} /></span>
        <input type="text" value={library.rawSearch} oninput={(e) => library.setSearch(e.currentTarget.value)} placeholder={t('Tìm tên video, file hoặc thư mục…')} aria-label={t('Tìm file trong thư mục và thư mục con')} />
      </div>
      <button class="btn outline toggle" class:on={library.videosOnly} aria-pressed={library.videosOnly} onclick={() => library.setVideosOnly(!library.videosOnly)} title={t('Hiện trực tiếp video trong thư mục hiện tại và mọi thư mục con.')}>
        <span class="ic purple"><FilmSlate size={16} weight={library.videosOnly ? 'fill' : 'duotone'} /></span>{t('Chỉ video')}
      </button>
      <button class="btn icon" onclick={() => library.toggleAll()} disabled={library.entries.length === 0} title="{t('Chọn / bỏ chọn kết quả (Ctrl+A)')}" aria-label={t('Chọn / bỏ chọn kết quả')}><span class="ic teal"><CheckSquare size={17} weight="duotone" /></span></button>
    </div>

    {#if !grid}
      <div class="cols head-row"><span></span>{@render headerCell('name', columnLabels.name)}{@render headerCell('size', columnLabels.size)}<span>{t('Ngày thêm')}</span>{@render headerCell('status', columnLabels.status)}</div>
    {/if}

    <div class="body">
      {#if library.entries.length === 0}
        <p class="empty caption">
          {#if library.drafts.length === 0 && app.library.filter((d) => d.provider === app.settings.provider).length === 0}{t('Dán link torrent ở trên để duyệt thư mục và chọn video.')}
          {:else}{t('Không tìm thấy file. Thử từ khóa khác hoặc bỏ lọc.')}{/if}
        </p>
      {:else if grid}
        <VirtualList bind:this={listRef} items={library.entries} rowHeight={134} tileWidth={160} key={(e) => e.key} onkeydown={keydown} label={t('Lưới file trong torrent')}>
          {#snippet row(entry: Entry)}
            <div class="tile" class:on={library.selected.has(entry.key)} role="option" aria-selected={library.selected.has(entry.key)} tabindex="-1" title={gridTitle(entry)}
              onclick={(e) => rowClick(entry, e)} ondblclick={() => library.open(entry)} onkeydown={() => {}}>
              <div class="tile-top">
                <span class="chip {colors[entry.videoKind]} ic {colors[entry.videoKind]}" style="width:38px;height:38px">{@render kindIcon(entry, 20)}</span>
              </div>
              <div class="tile-name"><b>{entry.name}</b><span class="m {isStatusDone(entry) ? 'done' : isStatusPartial(entry) ? 'partial' : ''}">{mark(entry)}</span></div>
              <span class="caption">{sizeLabel(entry)}</span>
              <span class="caption tiny-text">{statusLabel(entry)}</span>
            </div>
          {/snippet}
        </VirtualList>
      {:else}
        <VirtualList bind:this={listRef} items={library.entries} rowHeight={52} key={(e) => e.key} onkeydown={keydown} label={t('Danh sách file trong torrent')}>
          {#snippet row(entry: Entry)}
            <div class="cols item row-glass" class:on={library.selected.has(entry.key)} role="option" aria-selected={library.selected.has(entry.key)} tabindex="-1"
              onclick={(e) => rowClick(entry, e)} ondblclick={() => library.open(entry)} onkeydown={() => {}}>
              <input type="checkbox" checked={library.selected.has(entry.key)} onclick={(e) => { e.stopPropagation(); library.toggle(entry, e.shiftKey); }} aria-label={entry.location} />
              <div class="name">
                <span class="chip {colors[entry.videoKind]} ic {colors[entry.videoKind]}" style="width:34px;height:34px">{@render kindIcon(entry, 18)}</span>
                <div class="names">
                  <b class="ell">{entry.name}<span class="m {isStatusDone(entry) ? 'done' : isStatusPartial(entry) ? 'partial' : ''}" title={statusLabel(entry)}>{mark(entry)}</span></b>
                  <span class="caption ell loc">{entry.location}</span>
                </div>
              </div>
              <span>{sizeLabel(entry)}</span>
              <span class="caption">{dateLabel(entry)}</span>
              <span class="caption ell" title={statusLabel(entry)}>{statusLabel(entry)}</span>
            </div>
          {/snippet}
        </VirtualList>
      {/if}
    </div>

    <footer class="foot">
      <div class="sum">
        <b>{t('Đã chọn {0} mục · {1}', library.selectedEntries.length, formatBytes(library.selectedBytes))}</b>
        <span class="caption">{t('{0} thư mục · {1} file · {2}', library.entries.filter((e) => e.kind === 'folder').length, library.entries.filter((e) => e.kind === 'file').length, formatBytes(library.entries.reduce((s, e) => s + e.size, 0)))}</span>
      </div>
      <div class="actions">
        {#if library.selectedEntries.length > 0}
          <button class="btn danger" onclick={() => library.removeSelected()} title={t('Bỏ file và thư mục đã chọn khỏi thư viện (Delete). Giữ file trên máy và các tác vụ tải.')}><span class="ic red"><Trash size={16} weight="duotone" /></span>{t('Bỏ khỏi thư viện')}</button>
        {/if}
        {#if library.undoStack.length > 0}
          <button class="btn icon" onclick={() => library.undo()} title={t('Hoàn tác (Ctrl+Z)')} aria-label={t('Hoàn tác')}><span class="ic purple"><ArrowCounterClockwise size={16} weight="bold" /></span></button>
        {/if}
        <button class="btn outline" disabled={library.remaining === 0} onclick={() => library.queueRemaining()} title={t('Thêm mọi file còn lại trong toàn bộ thư viện vào queue, kể cả ngoài bộ lọc hiện tại; bỏ qua file đã có trong queue.')}>
          <span class="ic blue"><DownloadSimple size={16} weight="duotone" /></span>{t('Thêm tất cả còn lại ({0})', library.remaining)}
        </button>
        <button class="btn primary" disabled={!library.canQueue} onclick={() => library.queueSelected()}><Plus size={16} weight="bold" />{t('Thêm vào hàng đợi')}</button>
      </div>
    </footer>
  </div>
</section>

<style>
  .page { display: flex; flex-direction: column; height: 100%; min-height: 0; }
  .head { display: flex; justify-content: space-between; align-items: flex-end; gap: 16px; margin-bottom: 16px; }
  .actions { display: flex; align-items: center; gap: 4px; flex-wrap: wrap; justify-content: flex-end; }
  .add { padding: 16px 18px; margin-bottom: 16px; position: relative; overflow: hidden; }
  .label { display: block; margin-bottom: 8px; }
  .add-grid { display: grid; grid-template-columns: 1fr 190px; gap: 20px; }
  .input .row { display: flex; align-items: center; gap: 4px; margin-top: 8px; }
  .spacer { flex: 1; }
  .go { display: flex; flex-direction: column; gap: 8px; }
  .indeterminate { position: absolute; left: 0; right: 0; bottom: 0; height: 3px; background: linear-gradient(90deg, transparent, var(--accent), transparent); background-size: 40% 100%; background-repeat: no-repeat; animation: slide 1.2s linear infinite; }
  @keyframes slide { from { background-position: -40% 0; } to { background-position: 140% 0; } }
  .browser { flex: 1; min-height: 0; display: flex; flex-direction: column; overflow: hidden; }
  .toolbar { display: flex; align-items: center; gap: 8px; padding: 12px 16px 0; }
  .toolbar.second { padding-top: 10px; padding-bottom: 10px; }
  .nav { display: flex; }
  .crumbs { flex: 1; min-width: 0; display: flex; align-items: center; overflow: hidden; white-space: nowrap; }
  .crumb { padding: 4px 9px; border-radius: 999px; color: var(--muted); font-size: 12px; font-weight: 600; max-width: 260px; overflow: hidden; text-overflow: ellipsis; }
  .crumb:hover { background: var(--hover); color: var(--ink); }
  .crumb.last { color: var(--ink); font-size: 14px; }
  .sep { color: var(--muted); }
  .sort { position: relative; }
  .sort-btn { gap: 6px; }
  .menu { position: absolute; right: 0; top: calc(100% + 8px); z-index: 20; min-width: 228px; padding: 6px; display: flex; flex-direction: column; gap: 2px; border-radius: 18px; }
  .menu-item { display: flex; align-items: center; gap: 10px; padding: 9px 12px; border-radius: 12px; font-size: 12px; text-align: left; transition: background 0.15s; }
  .menu-item:hover { background: var(--hover); }
  .menu-item[aria-checked='true'] { background: var(--sheen), var(--raised); box-shadow: var(--rim); }
  .grow { flex: 1; }
  /* Segmented control: a recessed channel with a raised droplet on the active option. */
  .seg { display: flex; gap: 2px; padding: 3px; border-radius: 999px; background: var(--track); box-shadow: var(--well-shadow); }
  .seg .btn.icon { min-height: 28px; width: 32px; }
  .seg .btn.on, .toggle.on { background: var(--sheen), var(--raised); box-shadow: var(--rim), 0 4px 10px -6px rgb(35 45 70 / 0.4); color: var(--ink); }
  .hcell { display: inline-flex; align-items: center; gap: 4px; font: inherit; color: inherit; padding: 3px 8px; margin-left: -8px; border-radius: 999px; justify-self: start; transition: background 0.15s; }
  .hcell:hover, .hcell.active { color: var(--ink); background: var(--hover); }
  .search { position: relative; flex: 1; }
  .search .ic { position: absolute; left: 13px; top: 10px; }
  .search input { padding-left: 36px; border-radius: 999px; }
  .cols { display: grid; grid-template-columns: 46px minmax(200px, 1fr) 116px 136px 190px; align-items: center; padding: 0 16px 0 0; }
  .head-row { height: 34px; margin: 0 8px; color: var(--muted); font-size: 11px; font-weight: 600; }
  .head-row span:first-child { grid-column: 1; }
  .head-row span:nth-child(2) { padding-left: 0; }
  .body { flex: 1; min-height: 0; position: relative; }
  .item { height: 48px; cursor: default; }
  .item input { justify-self: center; }
  .name { display: flex; align-items: center; gap: 12px; min-width: 0; }
  .names { display: flex; flex-direction: column; min-width: 0; }
  .ell { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .loc { font-size: 10px; }
  .m { margin-left: 8px; font-weight: 600; color: var(--muted); }
  .m.done { color: var(--success); } .m.partial { color: var(--partial); }
  .empty { position: absolute; inset: 0; display: grid; place-content: center; text-align: center; padding: 24px; }
  .tile { position: relative; width: 160px; height: 134px; padding: 12px; border-radius: 20px; display: flex; flex-direction: column; gap: 4px; transition: background 0.15s, box-shadow 0.15s; }
  .tile:hover { background: var(--hover); }
  .tile.on { background: var(--sheen), var(--raised); box-shadow: var(--rim), 0 10px 22px -14px rgb(35 45 70 / 0.5); }
  .tile-top { display: flex; justify-content: space-between; }
  .tile-name { display: flex; justify-content: space-between; gap: 4px; font-size: 12px; max-height: 32px; overflow: hidden; }
  .tiny-text { font-size: 10px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .foot { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 12px 16px; background: linear-gradient(180deg, transparent, var(--hover)); }
  .sum { display: flex; flex-direction: column; min-width: 0; }
</style>
