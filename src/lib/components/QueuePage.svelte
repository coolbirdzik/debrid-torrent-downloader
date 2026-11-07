<script lang="ts">
  import ArrowClockwise from 'phosphor-svelte/lib/ArrowClockwise';
  import CheckCircle from 'phosphor-svelte/lib/CheckCircle';
  import FolderOpen from 'phosphor-svelte/lib/FolderOpen';
  import Pause from 'phosphor-svelte/lib/Pause';
  import Play from 'phosphor-svelte/lib/Play';
  import Stop from 'phosphor-svelte/lib/Stop';
  import Trash from 'phosphor-svelte/lib/Trash';
  import DownloadSimple from 'phosphor-svelte/lib/DownloadSimple';
  import { api } from '../api';
  import { app } from '../app.svelte';
  import { formatBytes } from '../browser';
  import { confirmAction } from '../confirm.svelte';
  import { t, tm } from '../i18n.svelte';
  import { canRemove, canResume, canRetry, canStop, providerLabel, providerShort, type Job } from '../types';
  import VirtualList from './VirtualList.svelte';
  import ContextMenu, { type MenuItem } from './ContextMenu.svelte';
  import Copy from 'phosphor-svelte/lib/Copy';
  import { copyText } from '../clipboard';
  import { untrack } from 'svelte';
  import { emptySelection, reconcileSelection, selectItem, selectionKey, selectionMouseDown } from '../selection';

  let selection = $state.raw(emptySelection());
  const selected = $derived(selection.selected);
  let listRef: { reveal: (index: number) => void } | undefined = $state();

  const jobs = $derived(app.jobs);
  const chosen = $derived(jobs.filter((job) => selected.has(job.id)));
  const activeNow = $derived(jobs.filter((job) => job.isActive).length);
  const failed = $derived(jobs.filter(canRetry).length);
  const jobSignature = $derived(JSON.stringify(jobs.map((job) => job.id)));
  const jobKeys = $derived<string[]>(JSON.parse(jobSignature));
  $effect(() => {
    const keys = jobKeys;
    untrack(() => { selection = reconcileSelection(selection, keys); });
  });

  const stateLabel = (job: Job) =>
    t(job.isStopping ? 'Đang dừng…' : ({ queued: 'Chờ xử lý', adding: 'Đang thêm', waiting: providerLabel(job.provider), downloading: 'Đang tải về', paused: 'Tạm dừng', stopped: 'Đã dừng', completed: 'Hoàn tất', failed: 'Có lỗi' } as const)[job.state]);
  const tone = (job: Job) => (job.state === 'completed' ? 'success' : job.state === 'failed' ? 'danger' : job.state === 'downloading' ? 'info' : job.state === 'paused' ? 'warn' : '');

  function pick(job: Job, event: MouseEvent) {
    selection = selectItem(selection, jobKeys, job.id, { ctrl: event.ctrlKey || event.metaKey, shift: event.shiftKey });
  }

  function keydown(event: KeyboardEvent) {
    const result = selectionKey(selection, jobKeys, event);
    if (!result) return;
    event.preventDefault();
    selection = result.selection;
    if (result.index !== undefined) {
      listRef?.reveal(result.index);
      (event.currentTarget as HTMLElement).focus();
    }
  }

  const start = () => app.run(() => api.queueStart());
  const ids = (list: Job[], test: (job: Job) => boolean) => list.filter(test).map((job) => job.id);

  async function remove(list: string[]) {
    await app.run(() => api.queueRemove(list));
    selection = reconcileSelection(selection, jobKeys);
  }

  async function clearAll() {
    const sure = await confirmAction({
      title: { key: 'Xóa toàn bộ hàng đợi?', args: [] },
      body: { key: 'Dừng mọi tác vụ rồi xóa toàn bộ hàng đợi. Giữ file đã tải, file tạm và torrent trên dịch vụ.', args: [] },
      confirm: { key: 'Xóa toàn bộ', args: [] },
      danger: true
    });
    if (sure) await app.run(() => api.queueClear(false));
    selection = emptySelection();
  }

  // Right-click acts on the selection; a row outside it becomes the only selected row first.
  let menu: { x: number; y: number } | null = $state(null);
  function openMenu(job: Job, event: MouseEvent) {
    event.preventDefault();
    if (!selected.has(job.id)) selection = selectItem(selection, jobKeys, job.id, {});
    menu = { x: event.clientX, y: event.clientY };
  }
  function menuItems(): MenuItem[] {
    const list = chosen;
    return [
      { label: t('Retry'), icon: ArrowClockwise, weight: 'bold', disabled: !list.some(canRetry), action: () => app.run(() => api.queueStart(ids(list, canRetry))) },
      { label: t('Bắt đầu / Resume'), icon: Play, tone: 'green', weight: 'fill', disabled: !list.some(canResume), action: () => app.run(() => api.queueStart(ids(list, canResume))) },
      { label: t('Stop'), icon: Stop, tone: 'red', weight: 'fill', disabled: !list.some(canStop), action: () => app.run(() => api.queueStop(ids(list, canStop))) },
      'separator',
      { label: t('Mở thư mục'), icon: FolderOpen, tone: 'amber', action: openFolder },
      { label: t('Sao chép tên'), icon: Copy, tone: 'teal', action: () => copyText(list.map((job) => job.name).join('\n'), { key: 'Đã sao chép {0} tên.', args: [String(list.length)] }) },
      'separator',
      { label: t('Xóa khỏi danh sách'), icon: Trash, tone: 'red', danger: true, disabled: !list.some(canRemove), action: () => remove(ids(list, canRemove)) }
    ];
  }

  function openFolder() {
    const dir = chosen[0]?.outputDirectory ?? chosen[0]?.targetDirectory ?? app.settings.outputDirectory;
    if (dir) void app.run(() => api.openFolder(dir));
  }
</script>

<section class="page">
  <header class="head">
    <div>
      <h1 class="title">{t('Hàng đợi tải')}</h1>
      <p class="caption">{t('{0} torrent, {1} hoàn tất, {2} đang xử lý', jobs.length, app.completedCount, activeNow)}</p>
    </div>
    <div class="actions">
      {#if app.anyActive}<button class="btn" onclick={() => app.run(() => api.queuePause())}><span class="ic amber"><Pause size={16} weight="duotone" /></span>{t('Pause tất cả')}</button>{/if}
      {#if jobs.some(canStop)}<button class="btn" onclick={() => app.run(() => api.queueStop())}><span class="ic red"><Stop size={16} weight="duotone" /></span>{t('Stop tất cả')}</button>{/if}
      <!-- Always shown so it is easy to find; disabled while nothing has failed. -->
      <button class="btn outline" disabled={failed === 0} onclick={() => app.run(() => api.queueStart(ids(jobs, canRetry)))} title={t('Thử lại ngay mọi torrent bị lỗi, tiếp tục phần đã tải nếu có')}><span class="ic blue"><ArrowClockwise size={16} weight="bold" /></span>{t('Retry tất cả ({0})', failed)}</button>
      <button class="btn primary" onclick={start}><Play size={16} weight="fill" />{t('Bắt đầu / Resume')}</button>
    </div>
  </header>

  <div class="card table">
    <div class="cols head-row"><span>{t('Torrent / File')}</span><span>{t('Dung lượng')}</span><span>{t('Dịch vụ tải')}</span><span>{t('Tải về máy')}</span><span>{t('Trạng thái')}</span><span>{t('Thao tác')}</span></div>
    <div class="body">
      {#if jobs.length === 0}
        <div class="empty">
          <span class="chip ic blue" style="width:56px;height:56px;border-radius:16px"><DownloadSimple size={26} weight="duotone" /></span>
          <h2 class="section">{t('Sẵn sàng cho loạt tải đầu tiên')}</h2>
          <p class="caption">{t('Chọn file ở tab File trong torrent rồi thêm vào hàng đợi.')}</p>
        </div>
      {:else}
        <VirtualList bind:this={listRef} items={jobs} rowHeight={64} key={(job) => job.id} onkeydown={keydown} label={t('Hàng đợi tải')}>
          {#snippet row(job: Job)}
            <div class="cols item row-glass" class:on={selected.has(job.id)} role="option" aria-selected={selected.has(job.id)} tabindex="-1" onmousedown={selectionMouseDown} onclick={(e) => pick(job, e)} oncontextmenu={(e) => openMenu(job, e)} onkeydown={() => {}}>
              <div class="names">
                <b class="ell" title={job.name}>{job.name}</b>
                <span class="caption ell" title={tm(job.detail)}>{tm(job.detail)}</span>
              </div>
              <span>{formatBytes(job.totalBytes)}</span>
              <div class="prog" title={providerLabel(job.provider)}><span>{providerShort(job.provider)} · {job.remoteProgress.toFixed(0)}%</span><div class="bar"><span style:width="{job.remoteProgress}%"></span></div></div>
              <div class="prog"><span class="local"><span>{job.localProgress.toFixed(0)}%</span>{#if job.state === 'downloading' && job.speed}<b class="speed">{formatBytes(job.speed)}/s</b>{/if}</span><div class="bar"><span style:width="{job.localProgress}%"></span></div></div>
              <span><span class="pill {tone(job)}">{stateLabel(job)}</span></span>
              <div class="row-actions">
                {#if canRetry(job)}<button class="btn icon" title={t('Thử lại torrent bị lỗi ngay, tiếp tục phần đã tải nếu có')} aria-label={t('Retry')} onclick={(e) => { e.stopPropagation(); app.run(() => api.queueStart([job.id])); }}><span class="ic blue"><ArrowClockwise size={16} weight="bold" /></span></button>{/if}
                {#if canResume(job)}<button class="btn icon" title={t('Bắt đầu hoặc tiếp tục torrent này')} aria-label={t('Resume')} onclick={(e) => { e.stopPropagation(); app.run(() => api.queueStart([job.id])); }}><span class="ic green"><Play size={16} weight="fill" /></span></button>{/if}
                {#if canStop(job)}<button class="btn icon" title={t('Dừng tác vụ này trong app; giữ file tạm để Resume. Torrent trên dịch vụ tiếp tục xử lý.')} aria-label={t('Stop')} onclick={(e) => { e.stopPropagation(); app.run(() => api.queueStop([job.id])); }}><span class="ic red"><Stop size={16} weight="fill" /></span></button>{/if}
                <button class="btn icon" disabled={!canRemove(job)} title={t('Dừng tác vụ và xóa dòng khỏi danh sách. Giữ file trên máy và torrent trên dịch vụ.')} aria-label={t('Xóa')} onclick={(e) => { e.stopPropagation(); remove([job.id]); }}><span class="ic red"><Trash size={16} weight="duotone" /></span></button>
              </div>
            </div>
          {/snippet}
        </VirtualList>
      {/if}
    </div>
    <footer class="foot">
      <div class="actions">
        {#if chosen.length > 0}
          <span class="caption">{t('Dòng đã chọn:')}</span>
          {#if chosen.some(canRetry)}<button class="btn icon" aria-label={t('Retry')} title={t('Retry')} onclick={() => app.run(() => api.queueStart(ids(chosen, canRetry)))}><span class="ic blue"><ArrowClockwise size={16} weight="bold" /></span></button>{/if}
          {#if chosen.some(canResume)}<button class="btn icon" aria-label={t('Resume')} title={t('Resume')} onclick={() => app.run(() => api.queueStart(ids(chosen, canResume)))}><span class="ic green"><Play size={16} weight="fill" /></span></button>{/if}
          {#if chosen.some(canStop)}<button class="btn icon" aria-label={t('Stop')} title={t('Stop')} onclick={() => app.run(() => api.queueStop(ids(chosen, canStop)))}><span class="ic red"><Stop size={16} weight="fill" /></span></button>{/if}
          {#if chosen.some(canRemove)}<button class="btn icon" aria-label={t('Xóa khỏi danh sách')} title={t('Dừng rồi bỏ dòng khỏi app, giữ file đã tải và torrent trên dịch vụ')} onclick={() => remove(ids(chosen, canRemove))}><span class="ic red"><Trash size={16} weight="duotone" /></span></button>{/if}
        {/if}
      </div>
      <div class="actions">
        <button class="btn" onclick={openFolder}><span class="ic amber"><FolderOpen size={16} weight="duotone" /></span>{t('Mở thư mục')}</button>
        {#if app.completedCount > 0}<button class="btn" onclick={() => app.run(() => api.queueClear(true))}><span class="ic green"><CheckCircle size={16} weight="duotone" /></span>{t('Xóa đã tải xong ({0})', app.completedCount)}</button>{/if}
        {#if jobs.length > 0}<button class="btn danger" onclick={clearAll}><span class="ic red"><Trash size={16} weight="duotone" /></span>{t('Xóa toàn bộ')}</button>{/if}
      </div>
    </footer>
  </div>
</section>

{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={menuItems()} onclose={() => (menu = null)} />
{/if}

<style>
  .page { display: flex; flex-direction: column; height: 100%; min-height: 0; }
  .head { display: flex; justify-content: space-between; align-items: flex-end; gap: 16px; margin-bottom: 16px; }
  .actions { display: flex; align-items: center; gap: 4px; flex-wrap: wrap; }
  .table { flex: 1; min-height: 0; display: flex; flex-direction: column; overflow: hidden; }
  .cols { display: grid; grid-template-columns: minmax(220px, 1fr) 96px 110px 160px 120px 150px; align-items: center; padding: 0 16px; column-gap: 8px; }
  .head-row { height: 40px; margin: 4px 8px 0; color: var(--muted); font-size: 11px; font-weight: 600; }
  .body { flex: 1; min-height: 0; position: relative; }
  .item { height: 60px; }
  .names { display: flex; flex-direction: column; min-width: 0; gap: 2px; }
  .ell { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .prog { display: flex; flex-direction: column; gap: 5px; font-size: 12px; }
  .local { display: flex; justify-content: space-between; align-items: baseline; gap: 6px; }
  .speed { color: var(--accent); font-size: 12px; font-variant-numeric: tabular-nums; white-space: nowrap; }
  .row-actions { display: flex; }
  .empty { position: absolute; inset: 0; display: flex; flex-direction: column; gap: 8px; align-items: center; justify-content: center; text-align: center; }
  .foot { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 12px 16px; }
</style>
