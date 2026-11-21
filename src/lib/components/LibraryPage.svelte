<script lang="ts">
  import { open as pickFiles } from '@tauri-apps/plugin-dialog';
  import ArrowLeft from 'phosphor-svelte/lib/ArrowLeft';
  import ArrowRight from 'phosphor-svelte/lib/ArrowRight';
  import ArrowUp from 'phosphor-svelte/lib/ArrowUp';
  import ArrowCounterClockwise from 'phosphor-svelte/lib/ArrowCounterClockwise';
  import FolderOpen from 'phosphor-svelte/lib/FolderOpen';
  import Link from 'phosphor-svelte/lib/Link';
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
  import Play from 'phosphor-svelte/lib/Play';
  import ListChecks from 'phosphor-svelte/lib/ListChecks';
  import VideoPlayer from './VideoPlayer.svelte';
  import ContextMenu, { type MenuItem } from './ContextMenu.svelte';
  import { copyText } from '../clipboard';
  import ListBullets from 'phosphor-svelte/lib/ListBullets';
  import SquaresFour from 'phosphor-svelte/lib/SquaresFour';
  import ArrowsDownUp from 'phosphor-svelte/lib/ArrowsDownUp';
  import CaretDown from 'phosphor-svelte/lib/CaretDown';
  import CaretUp from 'phosphor-svelte/lib/CaretUp';
  import Check from 'phosphor-svelte/lib/Check';
  import CheckCircle from 'phosphor-svelte/lib/CheckCircle';
  import CircleHalf from 'phosphor-svelte/lib/CircleHalf';
  import CircleDashed from 'phosphor-svelte/lib/CircleDashed';
  import Clock from 'phosphor-svelte/lib/Clock';
  import WarningCircle from 'phosphor-svelte/lib/WarningCircle';
  import TextAa from 'phosphor-svelte/lib/TextAa';
  import SortAscending from 'phosphor-svelte/lib/SortAscending';
  import SortDescending from 'phosphor-svelte/lib/SortDescending';
  import { app } from '../app.svelte';
  import CloudArrowDown from 'phosphor-svelte/lib/CloudArrowDown';
  import { breadcrumbs, draftId, formatBytes, isStatusDone, isStatusPartial, magnetOf, remoteOf, thumbRequest, type Entry, type HeaderSort, type SourceFilter } from '../browser';
  import Thumb from './Thumb.svelte';
  import { PROVIDERS, providerLabel, type Settings } from '../types';
  import Select from './Select.svelte';
  import { dialog } from '../confirm.svelte';
  import { destinationDialog } from '../destination.svelte';
  import { languageTag } from '../languages';
  import { t, tm } from '../i18n.svelte';
  import { countSources, library, type CloudView } from '../library.svelte';
  import { selectionControl, selectionMouseDown } from '../selection';
  import VirtualList from './VirtualList.svelte';

  const crumbs = $derived(breadcrumbs(library.location, library.drafts));
  const grid = $derived(app.settings.browserView === 'grid');
  const sources = $derived(countSources(library.input));
  const importTotal = $derived(app.inspect.running && app.inspect.total > 0 ? app.inspect.total : library.importSummary.total);
  const importDone = $derived(Math.min(importTotal, app.inspect.running && app.inspect.total > 0 ? app.inspect.done : library.importSummary.ready));
  const importPercent = $derived(importTotal > 0 ? Math.max(0, Math.min(100, importDone / importTotal * 100)) : 0);
  const importPhase = $derived(app.inspect.running ? app.settings.libraryImportPaused ? t('Đang dừng…') : t('Đang import thư viện…')
    : library.importSummary.pending === 0 ? t('Import hoàn tất')
    : app.settings.libraryImportPaused ? t('Import đã tạm dừng')
    : library.importSummary.needsAccount === library.importSummary.pending ? t('Chờ kết nối tài khoản để import')
    : library.importSummary.errors > 0 ? t('Chờ thử lại import') : t('Đang chờ import'));
  let listRef: { reveal: (index: number) => void; columns: () => number } | undefined = $state();
  const sourceOptions = $derived<{ value: SourceFilter; label: string }[]>([
    { value: 'all', label: t('Tất cả nguồn') },
    ...PROVIDERS.map((provider) => ({ value: provider.id as SourceFilter, label: provider.label }))
  ]);

  function statusLabel(entry: Entry): string {
    if (entry.pending) {
      if (app.settings.libraryImportPaused) return t('Import đã tạm dừng');
      if (!app.usableProviders.includes(entry.draft.provider)) return t('Chờ kết nối tài khoản để import');
      if (entry.draft.inspectionError && !library.cloudView(entry.draft)) return t('Chờ thử lại import');
      return t('Chờ đọc');
    }
    const state = isStatusDone(entry) ? t('Đã tải xong') : isStatusPartial(entry) ? t('Đã tải một phần') : t('Chưa tải');
    if (entry.kind === 'folder') return t('{0}/{1} file · {2}', entry.downloaded, entry.total, state);
    return !isStatusDone(entry) && !isStatusPartial(entry) && entry.queued ? t('Đã có trong hàng đợi') : state;
  }

  const sizeLabel = (entry: Entry) => (entry.kind === 'folder' ? t('{0} file', entry.fileCount) : formatBytes(entry.size));
  const kindLabel = (entry: Entry) => t(({ folder: 'Thư mục', video: 'Video', audio: 'Âm thanh', image: 'Hình ảnh', archive: 'Tệp nén', document: 'Tài liệu', other: 'File' })[entry.videoKind]);
  const newlyImported = (entry: Entry) => app.imported.has(draftId(entry.draft));
  const dateLabel = (entry: Entry) => (entry.addedAt ? new Date(entry.addedAt).toLocaleString(languageTag(app.settings.language), { dateStyle: 'short', timeStyle: 'short' }) : '—');

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

  function keydown(event: KeyboardEvent) {
    if (selectionControl(event)) return;
    const selection = library.keydown(event, listRef?.columns() ?? 1);
    if (selection) {
      event.preventDefault();
      if (selection.index !== undefined) {
        listRef?.reveal(selection.index);
        (event.currentTarget as HTMLElement).focus();
      }
      return;
    }
    const ctrl = event.ctrlKey || event.metaKey;
    if (ctrl && event.key.toLowerCase() === 'z') { event.preventDefault(); void library.undo(); }
    else if (event.key === 'Delete') { event.preventDefault(); void library.removeSelected(); }
    else if (event.key === 'Enter') { const target = library.selectedEntries[0]; if (target) library.open(target); }
    else if (event.key === 'Backspace' || (event.altKey && event.key === 'ArrowLeft')) { event.preventDefault(); event.altKey ? library.goBack() : library.goUp(); }
    else if (event.altKey && event.key === 'ArrowRight') { event.preventDefault(); library.goForward(); }
  }

  // Mouse side buttons move through the library history, as in a file explorer. A plain mouse sends them as buttons
  // 3 (back) and 4 (forward); mouse software (Logitech, Razer…) often sends the keyboard keys BrowserBack/BrowserForward
  // instead. Cancelling both mousedown and mouseup also stops the webview from running its own history navigation.
  function mouseNav(event: MouseEvent) {
    if (playing || (event.button !== 3 && event.button !== 4)) return;
    event.preventDefault();
    if (event.type !== 'mouseup') return;
    if (event.button === 3) library.goBack();
    else library.goForward();
  }

  // Newest first is the default; an explicitly chosen column sort still takes precedence.
  const sorts: { value: Settings['browserSort']; label: string; short: string }[] = $derived([
    { value: 'added-desc', label: t('Ngày thêm: mới nhất'), short: t('Mới nhất') },
    { value: 'added-asc', label: t('Ngày thêm: cũ nhất'), short: t('Cũ nhất') },
    { value: 'name', label: t('Theo tên'), short: t('Tên') }
  ]);
  const currentSort = $derived(sorts.find((s) => s.value === app.settings.browserSort) ?? sorts[0]);
  const columnLabels = $derived({ name: t('Tên file'), source: t('Nguồn'), size: t('Dung lượng'), added: t('Ngày thêm'), status: t('Loại / Trạng thái') });
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

  // Right-click menu. Like a file explorer: right-clicking a row outside the selection selects just that row first.
  let menu: { x: number; y: number; entry: Entry } | null = $state(null);

  function openMenu(entry: Entry, event: MouseEvent) {
    event.preventDefault();
    sortOpen = false;
    if (!library.selected.has(entry.key)) library.select(entry, { ctrl: false, shift: false });
    menu = { x: event.clientX, y: event.clientY, entry };
  }

  const doneCount = $derived(library.entries.filter(isStatusDone).length);

  function menuItems(entry: Entry): MenuItem[] {
    const chosen = library.selectedEntries;
    const magnets = [...new Set(chosen.map((e) => magnetOf(e.draft)).filter((m): m is string => m !== null))];
    const names = chosen.map((e) => e.name);
    return [
      ...(entry.kind === 'folder' ? [{ label: t('Mở'), icon: FolderOpen, tone: 'amber', action: () => library.open(entry) } as const] : []),
      ...(canPlay(entry) ? [{ label: t('Phát video'), icon: Play, weight: 'fill', action: () => play(entry) } as const] : []),
      'separator',
      { label: t('Thêm vào hàng đợi'), icon: DownloadSimple, disabled: !library.canQueue, action: () => library.queueSelected() },
      'separator',
      { label: t('Sao chép tên'), icon: TextAa, tone: 'teal', action: () => copyText(names.join('\n'), { key: 'Đã sao chép {0} tên.', args: [String(names.length)] }) },
      { label: t('Sao chép magnet link'), icon: Link, tone: 'purple', disabled: magnets.length === 0, action: () => copyText(magnets.join('\n'), { key: 'Đã sao chép {0} magnet link.', args: [String(magnets.length)] }) },
      'separator',
      { label: t('Chọn các file đã tải xong ({0})', doneCount), icon: ListChecks, tone: 'green', disabled: doneCount === 0, action: () => library.selectDone() },
      'separator',
      { label: t('Bỏ khỏi thư viện'), icon: Trash, tone: 'red', danger: true, action: () => library.removeSelected() }
    ];
  }

  /** Header click cycles ascending → descending → off. */
  function sortBy(column: NonNullable<HeaderSort>['column']) {
    const now = library.header;
    library.header = now?.column !== column ? { column, direction: 'asc' } : now.direction === 'asc' ? { column, direction: 'desc' } : null;
  }

  const typing = (target: EventTarget | null) => target instanceof HTMLElement && (target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName));

  function browserKeys(event: KeyboardEvent) {
    if (playing) return; // the player owns the keyboard while it is open
    if (event.key === 'F5' && !event.ctrlKey && !event.metaKey && !event.altKey && !dialog.open && !destinationDialog.open) {
      event.preventDefault();
      void library.refresh();
      return;
    }
    if (event.key === 'Escape' && (sortOpen || menu)) { sortOpen = false; menu = null; return; }
    // Shift+D: choose where to save, then queue the selection (same as the "Thêm vào hàng đợi" button).
    if (event.code === 'KeyD' && event.shiftKey && !event.ctrlKey && !event.altKey && !event.metaKey && !event.repeat && !typing(event.target)) {
      event.preventDefault();
      if (library.canQueue && !destinationDialog.open && !dialog.open) { menu = null; void library.queueSelected(); }
      return;
    }
    if (event.key !== 'BrowserBack' && event.key !== 'BrowserForward') return;
    event.preventDefault();
    if (event.key === 'BrowserBack') library.goBack();
    else library.goForward();
  }

  // Streaming: a video whose torrent the service has fully fetched. Whether this exact file has a link is checked by the
  // backend, because the draft's file list can predate the file selection made by the queue.
  // Queueing hands the remote torrent from the draft to its job, so a queued file is found through that job.
  // A torrent the Library still owns may be selected for playback (`select`); one a queue job owns is only waited on,
  // because the job makes its own file selection.
  type PlaySource = { torrentId: string; account: string | null; fileId: number; title: string; provider: Entry['draft']['provider']; select: boolean };
  let playing = $state.raw<PlaySource | null>(null);
  function playSource(entry: Entry): PlaySource | null {
    const remote = remoteOf(entry, app.jobs);
    return remote && entry.file ? { ...remote, fileId: entry.file.id, title: entry.name, provider: entry.draft.provider } : null;
  }
  const thumbOf = (entry: Entry) => thumbRequest(entry, app.jobs, app.cloudOf(entry.draft)?.status);
  const canPlay = (entry: Entry) => playSource(entry) !== null;
  function play(entry: Entry) {
    menu = null;
    playing = playSource(entry);
  }
  const activate = (entry: Entry) => (canPlay(entry) ? play(entry) : library.open(entry));

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

{#snippet progressCell(view: CloudView, entry: Entry)}
  {@const label = t(view.label)}
  {@const value = view.progress === null ? '' : `${view.progress.toFixed(0)}%`}
  {@const description = [providerLabel(entry.draft.provider), label, value, view.speed ? `${formatBytes(view.speed)}/s` : ''].filter(Boolean).join(' · ')}
  <span class="status-cell" title={description}>
    <span class="status-line">
      <span class="caption cloud {view.tone}">{#if view.tone === 'warn'}<WarningCircle size={14} weight="duotone" />{:else}<CloudArrowDown size={14} weight="duotone" />{/if}<span class="ell">{label}</span></span>
      {#if value}<b class="progress-value">{value}</b>{/if}
    </span>
    {#if view.progress !== null && view.tone === 'busy'}
      <span class="bar mini" role="progressbar" aria-label={`${entry.name} · ${description}`} aria-valuemin="0" aria-valuemax="100" aria-valuenow={view.progress}><span style:width="{view.progress}%"></span></span>
    {/if}
  </span>
{/snippet}

{#snippet statusIcon(entry: Entry)}
  {#if entry.pending}<Clock size={15} weight="duotone" />
  {:else if isStatusDone(entry)}<CheckCircle size={15} weight="fill" />
  {:else if isStatusPartial(entry)}<CircleHalf size={15} weight="fill" />
  {:else if entry.queued}<ListChecks size={15} weight="bold" />
  {:else}<CircleDashed size={15} weight="bold" />{/if}
{/snippet}

{#snippet localStatus(entry: Entry)}
  <span class="local-status" class:done={isStatusDone(entry)} class:partial={isStatusPartial(entry)} class:queued={entry.queued && !isStatusDone(entry) && !isStatusPartial(entry)} title={entry.pending && entry.draft.inspectionError ? tm(entry.draft.inspectionError) : statusLabel(entry)}>
    <span class="status-icon" aria-hidden="true">{@render statusIcon(entry)}</span><span class="ell">{statusLabel(entry)}</span>
  </span>
{/snippet}

<section class="page">
  <header class="head">
    <div>
      <h1 class="title">{t('Thư viện')}</h1>
      <p class="caption">{t('Thư viện: {0} torrent · {1} đã đọc · {2} chờ đọc · {3} thư mục con', library.drafts.length, library.readyCount, library.pendingCount, library.subfolders)}</p>
    </div>
    <div class="actions">
      {#if app.usableProviders.length === 0}
        <button class="btn" onclick={() => (app.page = 'settings')}><span class="ic blue"><User size={16} weight="duotone" /></span>{t('Kết nối tài khoản')}</button>
      {/if}
      {#if library.importSummary.pending > 0}
        <button class="btn" disabled={app.inspect.running} onclick={() => library.retryImport()}><span class="ic blue"><ArrowsClockwise size={16} weight="duotone" /></span>{t('Thử lại tất cả ({0})', library.importSummary.pending)}</button>
      {/if}
      {#if library.canRemoveFolder}
        <button class="btn danger" onclick={() => library.removeFolder()}><span class="ic red"><Trash size={16} weight="duotone" /></span>{t('Bỏ thư mục')}</button>
      {/if}
      <button class="btn" onclick={() => library.showAll()}><span class="ic purple"><FileText size={16} weight="duotone" /></span>{t('Tất cả file')}</button>
      <button class="btn danger" disabled={library.clearing || (app.library.length === 0 && !library.input)} onclick={() => library.newBatch()}><span class="ic red"><Trash size={16} weight="duotone" /></span>{t('Xóa toàn bộ thư viện…')}</button>
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
          <button class="btn outline stop-inspection" disabled={app.settings.libraryImportPaused} onclick={() => library.pauseImport()}><span class="ic red" aria-hidden="true"><X size={16} weight="bold" /></span>{t(app.settings.libraryImportPaused ? 'Đang dừng…' : 'Dừng đọc')}</button>
        {/if}
        <button class="btn primary" disabled={sources === 0} onclick={() => library.add()}><Plus size={16} weight="bold" />{t('Thêm vào thư viện')}</button>
      </div>
    </div>
    {#if library.importSummary.total > 0}
      <section class="import-progress" class:complete={!app.inspect.running && library.importSummary.pending === 0} class:waiting={!app.inspect.running && library.importSummary.pending > 0} aria-label={t('Tiến độ import')}>
        <div class="import-header">
          <strong role="status">{#if app.inspect.running}<ArrowsClockwise size={14} weight="bold" class="spin" />{/if}{importPhase}</strong>
          <span class="caption">{app.inspect.running ? t('Đã xử lý {0}/{1} mục', importDone, importTotal) : t('Đã đọc {0}/{1} mục', importDone, importTotal)} · {importPercent.toFixed(0)}%</span>
        </div>
        <div class="import-track" role="progressbar" aria-label={t('Tiến độ import')} aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(importPercent)} aria-valuetext={`${importPhase} · ${importDone}/${importTotal}`}><span style:width="{importPercent}%"></span></div>
        <p class="caption">{t('{0} đã đọc · {1} chờ đọc · {2} lỗi · Tất cả dịch vụ', library.importSummary.ready, library.importSummary.pending, library.importSummary.errors)}</p>
      </section>
    {/if}
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
      <button class="btn outline" disabled={library.refreshing || library.clearing} onclick={() => library.refresh()} title={t('Làm mới thư viện (F5)')} aria-busy={library.refreshing}>
        <span class="ic blue"><ArrowsClockwise size={16} weight="bold" class={library.refreshing ? 'spin' : ''} /></span>{t(library.refreshing ? 'Đang làm mới…' : 'Làm mới')}
      </button>
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
      <Select width="168px" label={t('Nhà cung cấp')} value={library.source} options={sourceOptions} onchange={(v: SourceFilter) => { library.setSource(v); listRef?.reveal(0); }} />
      <button class="btn icon" onclick={() => library.selectDone()} disabled={doneCount === 0} title={t('Chọn các file đã tải xong ({0})', doneCount)} aria-label={t('Chọn các file đã tải xong ({0})', doneCount)}><span class="ic green"><ListChecks size={17} weight="duotone" /></span></button>
      <button class="btn icon" onclick={() => library.toggleAll()} disabled={library.entries.length === 0} title="{t('Chọn / bỏ chọn kết quả (Ctrl+A)')}" aria-label={t('Chọn / bỏ chọn kết quả')}><span class="ic teal"><CheckSquare size={17} weight="duotone" /></span></button>
    </div>

    {#if !grid}
      <div class="cols head-row"><span></span>{@render headerCell('name', columnLabels.name)}{@render headerCell('source', columnLabels.source)}{@render headerCell('size', columnLabels.size)}{@render headerCell('added', columnLabels.added)}{@render headerCell('status', columnLabels.status)}</div>
    {/if}

    <div class="body">
      {#if library.entries.length === 0}
        <p class="empty caption">
          {#if library.drafts.length === 0 && app.library.length === 0}{t('Dán link torrent ở trên để duyệt thư mục và chọn video.')}
          {:else}{t('Không tìm thấy file. Thử từ khóa khác hoặc bỏ lọc.')}{/if}
        </p>
      {:else if grid}
        <VirtualList bind:this={listRef} items={library.entries} rowHeight={206} tileWidth={160} key={(e) => e.key} onkeydown={keydown} label={t('Lưới file trong torrent')}>
          {#snippet row(entry: Entry)}
            {@const cloud = library.cloudView(entry.draft, entry.file?.id)}
            <div class="tile" class:on={library.selected.has(entry.key)} role="option" aria-selected={library.selected.has(entry.key)} tabindex="-1" title={gridTitle(entry)}
              onmousedown={selectionMouseDown} onclick={(e) => rowClick(entry, e)} ondblclick={() => activate(entry)} oncontextmenu={(e) => openMenu(entry, e)} onkeydown={() => {}}>
              <div class="preview">
                <Thumb request={thumbOf(entry)}>
                  <span class="chip kind-{entry.videoKind}" style="width:44px;height:44px">{@render kindIcon(entry, 24)}</span>
                </Thumb>
                <span class="tag" title={providerLabel(entry.draft.provider)}>{providerLabel(entry.draft.provider)}</span>
                {#if newlyImported(entry)}<span class="new-badge" title={t('Vừa import xong')}>{t('Mới')}</span>{/if}
              </div>
              <div class="tile-name"><b>{entry.name}</b></div>
              <span class="caption">{sizeLabel(entry)}</span>
              {#if cloud}
                {@render progressCell(cloud, entry)}
              {/if}
              {@render localStatus(entry)}
            </div>
          {/snippet}
        </VirtualList>
      {:else}
        <VirtualList bind:this={listRef} items={library.entries} rowHeight={52} key={(e) => e.key} onkeydown={keydown} label={t('Danh sách file trong torrent')}>
          {#snippet row(entry: Entry)}
            {@const cloud = library.cloudView(entry.draft, entry.file?.id)}
            <div class="cols item row-glass" class:on={library.selected.has(entry.key)} role="option" aria-selected={library.selected.has(entry.key)} tabindex="-1"
              onmousedown={selectionMouseDown} onclick={(e) => rowClick(entry, e)} ondblclick={() => activate(entry)} oncontextmenu={(e) => openMenu(entry, e)} onkeydown={() => {}}>
              <input type="checkbox" checked={library.selected.has(entry.key)} onclick={(e) => { e.stopPropagation(); library.toggle(entry, e.shiftKey); }} aria-label={entry.location} />
              <div class="name">
                <span class="lead">
                  <Thumb request={thumbOf(entry)}>
                    <span class="chip kind-{entry.videoKind}" style="width:34px;height:34px">{@render kindIcon(entry, 18)}</span>
                  </Thumb>
                </span>
                <div class="names">
                  <div class="name-line"><b class="ell">{entry.name}</b>{#if newlyImported(entry)}<span class="new-badge" title={t('Vừa import xong')}>{t('Mới')}</span>{/if}</div>
                  <span class="caption ell loc">{entry.location}</span>
                </div>
              </div>
              <div class="source">
                <span class="tag">{providerLabel(entry.draft.provider)}</span>
                {#if cloud}{@render progressCell(cloud, entry)}{/if}
              </div>
              <span>{sizeLabel(entry)}</span>
              <span class="caption">{dateLabel(entry)}</span>
              <div class="type-status">
                {@render localStatus(entry)}
                <span class="file-kind"><span class="kind-{entry.videoKind}" aria-hidden="true">{@render kindIcon(entry, 12)}</span>{kindLabel(entry)}</span>
              </div>
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
          <button class="btn danger" disabled={library.clearing} onclick={() => library.removeSelected()} title={t('Bỏ khỏi thư viện')}><span class="ic red"><Trash size={16} weight="duotone" /></span>{t('Bỏ khỏi thư viện')}</button>
        {/if}
        {#if library.undoStack.length > 0}
          <button class="btn icon" onclick={() => library.undo()} title={t('Hoàn tác (Ctrl+Z)')} aria-label={t('Hoàn tác')}><span class="ic purple"><ArrowCounterClockwise size={16} weight="bold" /></span></button>
        {/if}
        <button class="btn danger" disabled={library.clearing || library.downloaded.files === 0} onclick={() => library.removeDownloaded()} title={t('Bỏ khỏi thư viện mọi file đã tải xong, kể cả ngoài thư mục đang xem. Giữ file trên máy.')}>
          <span class="ic red"><Trash size={16} weight="duotone" /></span>{t('Bỏ các file đã tải xong ({0})', library.downloaded.files)}
        </button>
        <button class="btn outline" disabled={library.queueing || library.remaining === 0} onclick={() => library.queueRemaining()} title={t('Thêm mọi file còn lại trong toàn bộ thư viện vào queue, kể cả ngoài bộ lọc hiện tại; bỏ qua file đã có trong queue.')}>
          <span class="ic blue"><DownloadSimple size={16} weight="duotone" /></span>{t('Thêm tất cả còn lại ({0})', library.remaining)}
        </button>
        <button class="btn primary" disabled={!library.canQueue} onclick={() => library.queueSelected()} title={t('Chọn nơi lưu rồi thêm vào hàng đợi (Shift+D)')}><Plus size={16} weight="bold" />{t('Thêm vào hàng đợi')}</button>
      </div>
    </footer>
  </div>

  {#if menu}
    <ContextMenu x={menu.x} y={menu.y} items={menuItems(menu.entry)} onclose={() => (menu = null)} />
  {/if}
</section>

{#if playing}
  <VideoPlayer provider={playing.provider} id={playing.torrentId} account={playing.account} title={playing.title} fileId={playing.fileId} select={playing.select} onclose={() => (playing = null)} />
{/if}

<style>
  .page { display: flex; flex-direction: column; height: 100%; min-height: 0; }
  .head { display: flex; justify-content: space-between; align-items: flex-end; gap: 12px; margin-bottom: 16px; flex-wrap: wrap; }
  .actions { display: flex; align-items: center; gap: 4px; flex-wrap: wrap; justify-content: flex-end; }
  .add { padding: 16px 18px; margin-bottom: 16px; position: relative; overflow: hidden; }
  .label { display: block; margin-bottom: 8px; }
  .add-grid { display: grid; grid-template-columns: 1fr 190px; gap: 20px; }
  .input .row { display: flex; align-items: center; gap: 4px; margin-top: 8px; }
  .spacer { flex: 1; }
  .go { display: flex; flex-direction: column; gap: 8px; }
  .stop-inspection { min-height: 40px; justify-content: center; }
  .stop-inspection .ic { display: inline-flex; line-height: 0; }
  .import-progress { margin-top: 12px; padding: 12px; border-radius: 12px; background: var(--subtle); display: flex; flex-direction: column; gap: 8px; }
  .import-header { display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 6px 12px; }
  .import-header strong { display: inline-flex; align-items: center; gap: 6px; font-size: 12px; }
  .import-header > span { font-variant-numeric: tabular-nums; }
  .import-track { height: 6px; border-radius: 999px; background: var(--track); overflow: hidden; }
  .import-track > span { display: block; height: 100%; background: var(--accent); }
  .complete .import-track > span { background: var(--success); }
  .waiting .import-track > span { background: var(--partial); }
  :global(.spin) { animation: spin 1.2s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
  @media (prefers-reduced-motion: reduce) { :global(.spin) { animation: none; } }
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
  .menu-item[aria-checked='true'] { background: var(--selected); }
  .grow { flex: 1; }
  .menu-item:disabled { opacity: 0.45; pointer-events: none; }
  /* Segmented control: options on a soft track. */
  .seg { display: flex; gap: 2px; padding: 3px; border-radius: 999px; background: var(--subtle); }
  .seg .btn.icon { min-height: 28px; width: 32px; }
  /* The active option wears the accent: the chosen tokens (cobalt pill with a cyan icon, like the logo). */
  .seg .btn.on, .toggle.on, .seg .btn.on:hover:not(:disabled), .toggle.on:hover:not(:disabled) { background: var(--chosen-bg); color: var(--chosen-ink); }
  .seg .btn.on .ic, .toggle.on .ic { color: var(--chosen-icon); }
  .hcell { display: inline-flex; align-items: center; gap: 4px; font: inherit; color: inherit; padding: 3px 8px; margin-left: -8px; border-radius: 999px; justify-self: start; transition: background 0.15s; }
  .hcell:hover, .hcell.active { color: var(--ink); background: var(--hover); }
  .search { position: relative; flex: 1; }
  .search .ic { position: absolute; left: 13px; top: 10px; }
  .search input { padding-left: 36px; border-radius: 999px; }
  .cols { display: grid; grid-template-columns: 46px minmax(200px, 1fr) 180px 90px 132px 166px; align-items: center; padding: 0 16px 0 0; }
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
  .name-line { display: flex; align-items: center; gap: 7px; min-width: 0; }
  .new-badge { flex: none; padding: 1px 6px; border-radius: 999px; background: var(--selected); color: var(--accent); font-size: 9px; font-weight: 700; line-height: 14px; }
  .preview .new-badge { position: absolute; top: 5px; left: 5px; }
  .type-status { display: flex; flex-direction: column; gap: 3px; min-width: 0; }
  .local-status { display: flex; align-items: center; gap: 6px; min-width: 0; color: var(--muted); font-size: 12px; font-weight: 500; }
  .local-status.done { color: var(--success); }
  .local-status.partial { color: var(--partial); }
  .local-status.queued { color: var(--accent); }
  .status-icon { display: inline-flex; flex: none; }
  .file-kind { display: flex; align-items: center; gap: 5px; font-size: 10px; color: var(--muted); }
  .file-kind > span { display: inline-flex; }
  .tile .local-status { font-size: 10px; }
  .empty { position: absolute; inset: 0; display: grid; place-content: center; text-align: center; padding: 24px; }
  .tile { position: relative; width: 160px; height: 206px; padding: 12px; border-radius: 20px; display: flex; flex-direction: column; gap: 4px; transition: background 0.15s, box-shadow 0.15s; }
  .tile:hover { background: var(--hover); }
  .tile.on { background: var(--selected); }
  .tile > * { flex: none; }
  /* 16:9 preview: the video's frame when there is one, else the file-type icon on a soft panel. */
  .preview { position: relative; height: 76px; border-radius: 12px; overflow: hidden; background: var(--subtle); display: grid; place-items: center; }
  .preview .tag { position: absolute; top: 5px; right: 5px; background: rgb(0 0 0 / 0.55); color: #fff; backdrop-filter: blur(4px); }
  /* List rows: a 16:9 slot so a frame is wide enough to recognise; icons sit at its left edge. */
  .lead { flex: none; width: 60px; height: 34px; border-radius: 8px; overflow: hidden; display: flex; align-items: center; }
  /* Two lines of name, never squeezed out by the rows below it. */
  .tile-name { display: flex; justify-content: space-between; align-items: flex-start; gap: 4px; font-size: 12px; line-height: 16px; height: 32px; overflow: hidden; }
  .tile-name b { min-width: 0; overflow-wrap: anywhere; display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  /* Source: a quiet tag, so the file-type colors stay the loudest thing in a row. */
  .tag { display: inline-block; max-width: 100%; padding: 2px 8px; border-radius: 999px; background: var(--subtle); color: var(--muted); font-size: 11px; font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .source { display: flex; flex-direction: column; align-items: flex-start; gap: 2px; min-width: 0; }
  .source .status-cell { align-self: stretch; }
  .status-cell { display: flex; flex-direction: column; gap: 3px; min-width: 0; padding-right: 8px; }
  .status-line { display: flex; align-items: center; justify-content: space-between; gap: 6px; min-width: 0; }
  .progress-value { flex: none; font-size: 11px; font-variant-numeric: tabular-nums; color: var(--accent); }
  .tile .status-cell { padding-right: 0; }
  .tile .status-cell .caption { font-size: 10px; }
  .cloud { display: inline-flex; align-items: center; gap: 5px; min-width: 0; }
  .cloud :global(svg) { flex: none; }
  .cloud.busy { color: var(--accent); }
  .cloud.ready { color: var(--success); }
  .cloud.warn { color: var(--danger); }
  .bar.mini { height: 4px; }
  .foot { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 12px 16px; flex-wrap: wrap; }
  @media (max-width: 1100px) {
    .head { margin-bottom: 12px; }
    .head > div:first-child { flex: 1 1 220px; min-width: 0; }
    .head > .actions { flex: 0 1 auto; }
    .add { padding: 12px; margin-bottom: 12px; }
    .input textarea { height: 56px; min-height: 56px; }
    .import-progress { margin-top: 8px; padding: 8px 10px; gap: 6px; }
    .toolbar { padding-top: 8px; }
    .toolbar.second { padding-top: 6px; padding-bottom: 6px; }
    .foot { padding-top: 8px; padding-bottom: 8px; }
    .cols { grid-template-columns: 42px minmax(140px, 1fr) 144px 72px 106px 132px; }
  }
  .sum { display: flex; flex-direction: column; min-width: 0; }
</style>
