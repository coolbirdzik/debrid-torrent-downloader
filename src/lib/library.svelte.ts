import { untrack } from 'svelte';
import { api } from './api';
import { app } from './app.svelte';
import { confirmAction } from './confirm.svelte';
import { askDestination } from './destination.svelte';
import { activeDrafts, buildEntries, downloadedRemoval, draftId, isStatusDone, planQueue, remainingCount, remainingRequests, removalPatches, requestFileCount, visibleFiles, type Entry, type HeaderSort, type Location, type QueueRequest, type Rank, type SourceFilter } from './browser';
import { chooseProviders } from './providers.svelte';
import { failedCloudStatus, PROVIDERS, type Draft, type Job, type LibraryPatch, type Provider } from './types';
import { emptySelection, selectAll, selectItem, selectionKey } from './selection';

/** Distinct magnets, links and .torrent paths in the add box. */
export function countSources(text: string): number {
  const found = text.match(/magnet:\?[^\s]+|https?:\/\/[^\s"]+|[A-Za-z]:\\[^\r\n"]*\.torrent|\/[^\r\n"]*\.torrent/gi) ?? [];
  return new Set(found.map((value) => value.toLowerCase())).size;
}

const HOME: Location = { draft: null, path: '', flat: false };

export interface CloudView {
  label: string;
  progress: number | null;
  tone: 'busy' | 'ready' | 'warn' | 'idle';
  speed?: number | null;
}

class LibraryState {
  location = $state<Location>({ ...HOME });
  back = $state<Location[]>([]);
  forward = $state<Location[]>([]);
  rawSearch = $state('');
  search = $state('');
  videosOnly = $state(false);
  header = $state<HeaderSort>(null);
  selection = $state.raw(emptySelection());
  selected = $derived(this.selection.selected);
  undoStack = $state.raw<LibraryPatch[][]>([]);
  input = $state('');
  clearing = $state(false);
  queueing = $state(false);
  refreshing = $state(false);
  /** Which service's torrents are shown. */
  source = $state<SourceFilter>('all');

  entries = $derived.by<Entry[]>(() => {
    // Only membership of the queue matters to the rows, not progress: the signature keeps the (large) row list from
    // being rebuilt on every progress event.
    app.queueSignature;
    const jobs = untrack(() => app.jobs);
    return buildEntries(app.library, this.source, jobs, {
      location: this.location, search: this.search, videosOnly: this.videosOnly, sort: app.settings.browserSort, header: this.header, imported: app.imported
    });
  });

  selectedEntries = $derived(this.entries.filter((entry) => this.selected.has(entry.key)));
  drafts = $derived(activeDrafts(app.library, this.source));
  /** Services that have torrents in the Library, in the fixed order: the source filter offers only these. */
  sources = $derived(PROVIDERS.map((p) => p.id).filter((id) => app.library.some((d) => d.provider === id && !d.isHidden)));
  remaining = $derived.by(() => {
    app.queueSignature;
    return remainingCount(app.library, this.source, untrack(() => app.jobs));
  });
  /** Queue jobs take ownership of a draft's remote torrent when files are queued. */
  private cloudJobs = $derived.by(() => {
    const map = new Map<string, Job[]>();
    for (const job of app.jobs) {
      if (!job.torrentId) continue;
      const id = draftId(job);
      const jobs = map.get(id);
      if (jobs) jobs.push(job);
      else map.set(id, [job]);
    }
    return map;
  });

  cloudView(draft: Draft, fileId?: number): CloudView | undefined {
    const cloud = app.cloudOf(draft);
    if (!cloud) {
      if (draft.torrentId) return;
      const jobs = this.cloudJobs.get(draftId(draft))?.filter(job =>
        fileId === undefined || job.selectedFileIds === null || job.selectedFileIds.includes(fileId));
      if (!jobs?.length) return;
      const waiting = jobs.filter(job => job.state === 'adding' || job.state === 'waiting');
      if (waiting.length) {
        const progress = waiting.reduce((highest, job) => Math.max(highest, job.remoteProgress), 0);
        return { label: 'Dịch vụ đang tải', progress: Math.min(100, progress), tone: 'busy' };
      }
      if (jobs.some(job => job.state === 'downloading' || job.state === 'completed'))
        return { label: 'Sẵn sàng', progress: 100, tone: 'ready' };
      // Queued, paused, stopped or failed jobs keep the last service progress they saw; it is not refreshed meanwhile.
      const last = jobs.reduce((best, job) => (job.remoteProgress > best.remoteProgress ? job : best));
      const progress = Math.max(0, Math.min(100, last.remoteProgress));
      if (progress >= 100) return { label: 'Sẵn sàng', progress: 100, tone: 'ready' };
      const label = ({ queued: 'Chờ xử lý', paused: 'Tạm dừng', stopped: 'Đã dừng', failed: 'Có lỗi' } as Partial<Record<Job['state'], string>>)[last.state] ?? 'Dịch vụ đang tải';
      return { label, progress, tone: 'idle' };
    }
    const progress = Number.isFinite(cloud.progress) ? Math.max(0, Math.min(100, cloud.progress)) : null;
    switch (cloud.status) {
      case 'downloaded': return { label: 'Sẵn sàng', progress: 100, tone: 'ready' };
      case 'waiting_files_selection': return { label: 'Chờ chọn file', progress: null, tone: 'idle' };
      case 'missing': return { label: 'Không còn trên dịch vụ', progress: null, tone: 'warn' };
      case 'magnet_error': case 'error': case 'virus': case 'dead':
        return { label: 'Lỗi trên dịch vụ', progress: null, tone: 'warn' };
      case 'queued': return { label: 'Đang xếp hàng trên dịch vụ', progress, tone: 'busy' };
      case 'magnet_conversion': return { label: 'Đang lấy metadata', progress, tone: 'busy' };
      case 'compressing': return { label: 'Đang đóng gói', progress, tone: 'busy' };
      case 'uploading': return { label: 'Đang chuẩn bị link', progress, tone: 'busy' };
      default: return { label: cloud.status === 'downloading' ? 'Dịch vụ đang tải' : cloud.status, progress, speed: cloud.speed, tone: 'busy' };
    }
  }

  /** Prefer the service whose cloud copy is furthest along when the same file is on several services. */
  private rank: Rank = (draft) => {
    const cloud = app.cloudOf(draft);
    if (!cloud) return draft.torrentId ? 0 : -1;
    return cloud.status === 'downloaded' ? 1000 : cloud.progress;
  };
  canQueue = $derived(!this.queueing && !this.clearing && this.selectedEntries.some((entry) => entry.kind === 'file' || entry.fileCount > 0));
  selectedBytes = $derived(this.selectedEntries.reduce((sum, entry) => sum + entry.size, 0));
  pendingCount = $derived(this.drafts.filter((d) => d.files.length === 0).length);
  /** Import is shared across services even when the browser is filtered to only one. */
  importSummary = $derived.by(() => {
    const drafts = activeDrafts(app.library);
    const pending = drafts.filter((draft) => draft.files.length === 0);
    return { total: drafts.length, ready: drafts.length - pending.length, pending: pending.length, errors: pending.filter((draft) => !!draft.inspectionError).length, needsAccount: pending.filter((draft) => !app.usableProviders.includes(draft.provider)).length };
  });
  readyCount = $derived(this.drafts.length - this.pendingCount);
  subfolders = $derived(this.drafts.reduce((sum, d) => sum + new Set(visibleFiles(d).map((f) => f.path.replace(/\\/g, '/').split('/').slice(0, -1).join('/').toLowerCase())).size, 0));
  /** Finished files across the whole Library (for the current service filter), whatever folder is open. */
  downloaded = $derived(downloadedRemoval(app.library, this.source));
  canRemoveFolder = $derived(this.location.draft !== null && !this.location.flat);
  /** Selected torrents whose import failed and can be read again now. */
  selectedImportErrors = $derived(this.importErrorsOf(this.selectedEntries));
  retrying = $state(false);

  /** A torrent that could not be read: it reported an error, or its service gave up on it. */
  isImportError(draft: Draft): boolean {
    if (draft.files.length > 0 || draft.isHidden) return false;
    return !!draft.inspectionError || failedCloudStatus(app.cloudOf(draft)?.status);
  }

  /** Failed imports among the entries, once per torrent, limited to services with a token. */
  importErrorsOf(entries: Entry[]): Draft[] {
    const found = new Map<string, Draft>();
    for (const entry of entries) {
      if (entry.pending && this.isImportError(entry.draft) && app.usableProviders.includes(entry.draft.provider)) found.set(draftId(entry.draft), entry.draft);
    }
    return [...found.values()];
  }

  private timer: ReturnType<typeof setTimeout> | undefined;

  setSearch(value: string) {
    this.rawSearch = value;
    this.clearSelection();
    clearTimeout(this.timer);
    // Typing stays instant on small libraries; big ones wait for a short pause before rebuilding thousands of rows.
    const total = app.library.reduce((sum, d) => sum + d.files.length, 0);
    if (total < 3000) this.search = value;
    else this.timer = setTimeout(() => (this.search = value), 180);
  }

  /** Apply a pending debounced search now, so an action never reads rows that no longer match the search box. */
  flush() {
    if (this.search !== this.rawSearch) {
      clearTimeout(this.timer);
      this.search = this.rawSearch;
    }
  }

  setSource(value: SourceFilter) {
    this.source = value;
    this.clearSelection();
    const open = this.location.draft;
    if (open && value !== 'all' && !open.startsWith(`${value}|`)) this.navigate({ ...HOME });
  }

  setVideosOnly(value: boolean) {
    this.videosOnly = value;
    this.clearSelection();
  }

  clearSelection() {
    this.selection = emptySelection();
  }

  navigate(location: Location) {
    this.back = [...this.back, this.location];
    this.forward = [];
    this.go(location);
  }

  private go(location: Location) {
    this.location = location;
    this.rawSearch = '';
    this.search = '';
    this.videosOnly = false;
    this.clearSelection();
  }

  goBack() {
    const previous = this.back.at(-1);
    if (!previous) return;
    this.forward = [this.location, ...this.forward];
    this.back = this.back.slice(0, -1);
    this.go(previous);
  }

  goForward() {
    const next = this.forward[0];
    if (!next) return;
    this.back = [...this.back, this.location];
    this.forward = this.forward.slice(1);
    this.go(next);
  }

  goUp() {
    const { draft, path } = this.location;
    if (!draft) return;
    if (this.location.flat || !path) this.navigate(HOME);
    else this.navigate({ draft, path: path.split('/').slice(0, -1).join('/'), flat: false });
  }

  showAll() {
    this.navigate({ draft: this.location.draft, path: this.location.path, flat: true });
  }

  open(entry: Entry) {
    if (entry.kind === 'folder') this.navigate({ draft: `${entry.draft.provider}|${entry.draft.sourceKey.toLowerCase()}`, path: entry.folderPath, flat: false });
  }

  resetNavigation() {
    this.back = [];
    this.forward = [];
    this.go({ ...HOME });
    this.undoStack = [];
    this.header = null;
  }

  /** Read the given torrents again now (every unread torrent when none are given); also resumes a paused import. */
  async retryImport(drafts?: Draft[]) {
    if (drafts && drafts.length === 0) return;
    this.retrying = true;
    try {
      const done = await app.run(async () => {
        await api.libraryRetry(drafts?.map(({ provider, sourceKey }) => ({ provider, sourceKey })));
        app.settings = { ...app.settings, libraryImportPaused: false };
        return true;
      });
      if (done && drafts) app.toast({ key: 'Đang thử lại import {0} torrent.', args: [String(drafts.length)] });
    } finally {
      this.retrying = false;
    }
  }

  async refresh() {
    if (this.refreshing || this.clearing) return;
    this.refreshing = true;
    try {
      await app.run(() => app.refreshLibrary());
    } finally {
      this.refreshing = false;
    }
  }

  async pauseImport() {
    await app.run(async () => {
      await api.libraryCancelInspect();
      app.settings = { ...app.settings, libraryImportPaused: true };
    });
  }

  /** Plain click selects one row, Ctrl toggles, Shift selects the range from the anchor. */
  select(entry: Entry, modifiers: { ctrl: boolean; shift: boolean }) {
    this.selection = selectItem(this.selection, this.entries.map((e) => e.key), entry.key, modifiers);
  }

  toggle(entry: Entry, shift = false) {
    this.selection = selectItem(this.selection, this.entries.map((e) => e.key), entry.key, { shift }, true);
  }

  /** Selects exactly the visible entries already saved on this computer (the ✓ rows). */
  selectDone() {
    this.flush();
    this.selection = selectAll(this.entries.filter(isStatusDone).map((e) => e.key));
  }

  toggleAll() {
    this.flush();
    this.selection = this.entries.every((e) => this.selected.has(e.key)) ? emptySelection() : selectAll(this.entries.map((e) => e.key));
  }

  keydown(event: KeyboardEvent, columns: number) {
    this.flush();
    const result = selectionKey(this.selection, this.entries.map((e) => e.key), event, columns);
    if (result) this.selection = result.selection;
    return result;
  }

  async removeSelected() {
    this.flush();
    await this.removeEntries(this.selectedEntries);
  }

  async removeDownloaded() {
    this.flush();
    const { entries } = this.downloaded;
    await this.removeEntries(entries, true);
  }

  async removeFolder() {
    const { draft } = this.location;
    const target = this.drafts.find((d) => `${d.provider}|${d.sourceKey.toLowerCase()}` === draft);
    if (!target) return;
    const entry = { kind: 'folder', draft: target, folderPath: this.location.path } as Entry;
    const location = this.location;
    if (await this.removeEntries([entry]) && this.location === location)
      this.navigate(location.path ? { draft, path: location.path.split('/').slice(0, -1).join('/'), flat: false } : HOME);
  }

  private async removeEntries(entries: Entry[], downloaded = false) {
    if (!entries.length || this.clearing || this.queueing) return false;
    this.clearing = true;
    try {
      const { apply } = removalPatches(entries);
      const deleteCloud = app.settings.deleteCloudOnLibraryRemove;
      const drafts = new Map(entries.map(entry => [draftId(entry.draft), entry.draft]));
      const cloudCount = deleteCloud ? apply.filter(patch => {
        const draft = drafts.get(draftId(patch))!;
        const excluded = new Set(patch.excludedFileIds);
        return patch.isHidden || (draft.files.length > 0 && draft.files.every(file => excluded.has(file.id)));
      }).length : 0;
      if (cloudCount || downloaded) {
        const confirmed = await confirmAction({
          title: { key: downloaded ? 'Bỏ các file đã tải xong?' : 'Bỏ khỏi thư viện?', args: [] },
          body: cloudCount
            ? { key: 'Xóa cả bản cloud của {0} torrent được bỏ hết khỏi thư viện. Tác vụ tải liên quan sẽ bị dừng và xóa khỏi hàng đợi. File trên máy được giữ lại. Không thể hoàn tác xóa cloud.', args: [String(cloudCount)] }
            : { key: 'Bỏ {0} file đã tải xong khỏi thư viện. Giữ file trên máy, hàng đợi và dữ liệu cloud. Có thể hoàn tác bằng Ctrl+Z.', args: [String(this.downloaded.files)] },
          confirm: { key: 'Bỏ khỏi thư viện', args: [] },
          danger: true
        });
        if (!confirmed) return false;
      }
      const result = await app.run(() => api.libraryRemove(apply, deleteCloud));
      if (!result) return false;
      if (result.deletedCloud) this.undoStack = [];
      if (result.undo.length) this.undoStack = [...this.undoStack, result.undo];
      result.errors.forEach(error => app.toast(error, 'error'));
      this.clearSelection();
      if (this.location.draft && !this.drafts.some(draft => draftId(draft) === this.location.draft)) this.navigate({ ...HOME });
      return result.errors.length === 0;
    } finally {
      this.clearing = false;
    }
  }

  async undo() {
    if (this.clearing) return;
    const patches = this.undoStack.at(-1);
    if (!patches) return;
    this.undoStack = this.undoStack.slice(0, -1);
    await app.run(() => api.libraryUpdate(patches));
  }

  async queueSelected() {
    if (this.queueing || this.clearing) return;
    this.queueing = true;
    try {
      this.flush();
      const entries = this.selectedEntries;
      const plan = () => planQueue(entries, untrack(() => app.jobs), app.library, this.rank);
      let { fresh, forced, downloaded, duplicates } = plan();
      // Files that are only downloaded still open the dialog: Force download is how they are queued again.
      if (forced.length === 0) {
        this.warnQueued(duplicates);
        return;
      }
      const answer = await askDestination({ files: requestFileCount(fresh), downloaded, directory: app.settings.outputDirectory });
      if (answer === null) return;
      // A file can enter the queue or finish while the destination dialog is open. Recheck the original selection.
      ({ fresh, forced, downloaded, duplicates } = plan());
      const requests = answer.force ? forced : fresh;
      if (requests.length === 0) {
        this.warnQueued(duplicates);
        if (!answer.force) this.warnDownloaded(downloaded);
        return;
      }
      this.clearSelection();
      await this.queueAndStart(requests, answer.directory, duplicates, answer.force);
      if (!answer.force) this.warnDownloaded(downloaded);
    } finally {
      this.queueing = false;
    }
  }

  async queueRemaining() {
    if (this.queueing || this.clearing) return;
    this.queueing = true;
    try {
      this.flush();
      const requests = remainingRequests(app.library, this.source, untrack(() => app.jobs), this.rank);
      if (requests.length === 0) return;
      const answer = await askDestination({ files: requestFileCount(requests), directory: app.settings.outputDirectory });
      if (answer === null) return;
      this.clearSelection();
      await this.queueAndStart(requests, answer.directory);
    } finally {
      this.queueing = false;
    }
  }

  private warnQueued(count: number) {
    if (count > 0) app.toast({ key: '{0} file đã có trong hàng đợi, bỏ qua file trùng.', args: [String(count)] }, 'warning');
  }

  private warnDownloaded(count: number) {
    if (count > 0) app.toast({ key: 'Bỏ qua {0} file đã tải về máy.', args: [String(count)] }, 'warning');
  }

  /** New queue rows start downloading right away; other queued, paused or stopped rows are left alone. */
  private async queueAndStart(requests: QueueRequest[], directory: string, duplicates = 0, force = false) {
    const created = await app.run(() => api.queueAdd(requests.map((request) => ({ ...request, outputDirectory: directory, force }))));
    if (!created) return;
    this.warnQueued(duplicates);
    if (!created.length) {
      if (!duplicates) app.toast({ key: 'Không có file mới được thêm vào hàng đợi.', args: [] }, 'warning');
      return;
    }
    let started = true;
    await app.run(async () => {
      try {
        await api.queueStart(created);
      } catch (error) {
        started = false;
        throw error;
      }
    });
    const key = started ? 'Đã thêm {0} torrent vào hàng đợi và bắt đầu tải.' : 'Đã thêm {0} torrent vào hàng đợi.';
    app.toast({ key, args: [String(created.length)] }, 'success');
  }

  async newBatch() {
    if (this.clearing || this.queueing) return;
    this.clearing = true;
    try {
      const deleteCloud = app.settings.deleteCloudOnLibraryRemove;
      const patches = app.library.map(draft => ({
        provider: draft.provider, sourceKey: draft.sourceKey, excludedFileIds: draft.excludedFileIds, isHidden: true
      }));
      const confirmed = await confirmAction({
        title: { key: 'Xóa toàn bộ thư viện?', args: [] },
        body: {
          key: deleteCloud
            ? 'Xóa {0} mục khỏi thư viện và xóa bản cloud của các torrent đang hiển thị. Tác vụ tải liên quan sẽ bị dừng và xóa khỏi hàng đợi. File trên máy được giữ lại. Không thể hoàn tác xóa cloud.'
            : 'Xóa {0} mục khỏi thư viện của tất cả nhà cung cấp, kể cả mục đang chờ đọc. Giữ hàng đợi, file trên máy và dữ liệu cloud.',
          args: [String(patches.length)]
        },
        confirm: { key: 'Xóa toàn bộ thư viện', args: [] },
        danger: true
      });
      if (!confirmed) return;
      if (deleteCloud) {
        const result = await app.run(() => api.libraryRemove(patches, true, true));
        if (!result) return;
        app.forgetLibraryDrafts(result.purged);
        if (result.deletedCloud || result.purged.length) this.undoStack = [];
        result.errors.forEach(error => app.toast(error, 'error'));
        if (result.errors.length) return;
      } else {
        const cleared = await app.run(async () => { await api.libraryReset(); return true; });
        if (!cleared) return;
        app.clearLibraryView();
      }
      this.input = '';
      this.source = 'all';
      this.resetNavigation();
    } finally {
      this.clearing = false;
    }
  }

  async add() {
    if (this.clearing) return;
    const text = this.input;
    if (!text.trim()) return;
    const { settings } = app;
    const choice = await chooseProviders({
      count: countSources(text), usable: app.usableProviders, fallback: settings.provider, saved: settings.libraryProviders, remember: settings.rememberLibraryProviders
    });
    if (!choice) return;
    const same = (a: Provider[], b: Provider[]) => a.length === b.length && a.every((p) => b.includes(p));
    if (!same(choice.providers, settings.libraryProviders) || choice.remember !== settings.rememberLibraryProviders) {
      void app.setSettings({ libraryProviders: choice.providers, rememberLibraryProviders: choice.remember });
    }
    const result = await app.run(() => api.libraryAdd(text, choice.providers));
    if (!result) return;
    app.settings = { ...app.settings, libraryImportPaused: false };
    this.input = '';
    if (result.errors.length) app.toast(result.errors[0], 'error');
    if (result.duplicates) app.toast({ key: '{0} torrent đã có trong thư viện.', args: [String(result.duplicates)] });
  }

  appendSources(paths: string[]) {
    const lines = paths.filter((p) => p.toLowerCase().endsWith('.torrent'));
    if (lines.length) this.input = (this.input.trim() ? this.input.trimEnd() + '\n' : '') + lines.join('\n');
  }
}

export const library = new LibraryState();
