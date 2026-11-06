import { untrack } from 'svelte';
import { api } from './api';
import { app } from './app.svelte';
import { askDestination } from './destination.svelte';
import { activeDrafts, buildEntries, draftId, queueRequests, queuedLookup, remainingCount, remainingRequests, removalPatches, visibleFiles, type Entry, type HeaderSort, type Location, type Rank, type SourceFilter } from './browser';
import { chooseProviders } from './providers.svelte';
import { PROVIDERS, type LibraryPatch, type Provider } from './types';
import { emptySelection, selectAll, selectItem, selectionKey } from './selection';

/** Distinct magnets, links and .torrent paths in the add box. */
export function countSources(text: string): number {
  const found = text.match(/magnet:\?[^\s]+|https?:\/\/[^\s"]+|[A-Za-z]:\\[^\r\n"]*\.torrent|\/[^\r\n"]*\.torrent/gi) ?? [];
  return new Set(found.map((value) => value.toLowerCase())).size;
}

const HOME: Location = { draft: null, path: '', flat: false };

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
  /** Which service's torrents are shown. */
  source = $state<SourceFilter>('all');

  // Only membership of the queue matters to the rows, not progress: a string signature keeps the (large) row list from
  // being rebuilt on every progress event.
  private queueSignature = $derived(app.jobs.map((job) => `${job.id}:${job.provider}:${job.sourceKey}:${job.selectedFileIds?.length ?? 'all'}`).join(','));

  entries = $derived.by<Entry[]>(() => {
    this.queueSignature;
    const jobs = untrack(() => app.jobs);
    return buildEntries(app.library, this.source, jobs, {
      location: this.location, search: this.search, videosOnly: this.videosOnly, sort: app.settings.browserSort, header: this.header
    });
  });

  selectedEntries = $derived(this.entries.filter((entry) => this.selected.has(entry.key)));
  drafts = $derived(activeDrafts(app.library, this.source));
  /** Services that have torrents in the Library, in the fixed order: the source filter offers only these. */
  sources = $derived(PROVIDERS.map((p) => p.id).filter((id) => app.library.some((d) => d.provider === id && !d.isHidden)));
  remaining = $derived.by(() => {
    this.queueSignature;
    return remainingCount(app.library, this.source, untrack(() => app.jobs));
  });
  /** Remote progress of queued torrents still being fetched by their service, per Library torrent. */
  jobCloud = $derived.by(() => {
    const map = new Map<string, number>();
    for (const job of app.jobs) {
      if (job.state !== 'adding' && job.state !== 'waiting') continue;
      const id = draftId(job);
      map.set(id, Math.max(map.get(id) ?? 0, job.remoteProgress));
    }
    return map;
  });

  /** Prefer the service whose cloud copy is furthest along when the same file is on several services. */
  private rank: Rank = (draft) => {
    const cloud = app.cloudOf(draft);
    if (!cloud) return draft.torrentId ? 0 : -1;
    return cloud.status === 'downloaded' ? 1000 : cloud.progress;
  };
  canQueue = $derived(this.selectedEntries.some((entry) => !entry.queued && (entry.kind === 'file' || entry.fileCount > 0)));
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
  canRemoveFolder = $derived(this.location.draft !== null && !this.location.flat);

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

  async retryImport() {
    await app.run(async () => {
      await api.libraryRetry();
      app.settings = { ...app.settings, libraryImportPaused: false };
    });
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
    if (this.selectedEntries.length === 0) return;
    const { apply, undo } = removalPatches(this.selectedEntries);
    this.undoStack = [...this.undoStack, undo];
    this.clearSelection();
    await app.run(() => api.libraryUpdate(apply));
  }

  async removeFolder() {
    const { draft } = this.location;
    const target = this.drafts.find((d) => `${d.provider}|${d.sourceKey.toLowerCase()}` === draft);
    if (!target) return;
    const entry = { kind: 'folder', draft: target, folderPath: this.location.path } as Entry;
    const { apply, undo } = removalPatches([entry]);
    this.undoStack = [...this.undoStack, undo];
    this.navigate(this.location.path ? { draft, path: this.location.path.split('/').slice(0, -1).join('/'), flat: false } : HOME);
    await app.run(() => api.libraryUpdate(apply));
  }

  async undo() {
    const patches = this.undoStack.at(-1);
    if (!patches) return;
    this.undoStack = this.undoStack.slice(0, -1);
    await app.run(() => api.libraryUpdate(patches));
  }

  async queueSelected() {
    this.flush();
    const lookup = queuedLookup(untrack(() => app.jobs), app.library);
    const requests = queueRequests(this.selectedEntries, lookup, this.rank);
    if (requests.length === 0) return;
    const directory = await this.askDirectory(requests);
    if (directory === null) return;
    this.clearSelection();
    await this.queueAndStart(requests, directory);
  }

  async queueRemaining() {
    this.flush();
    const requests = remainingRequests(app.library, this.source, untrack(() => app.jobs), this.rank);
    if (requests.length === 0) return;
    const directory = await this.askDirectory(requests);
    if (directory === null) return;
    this.clearSelection();
    await this.queueAndStart(requests, directory);
  }

  /** Where to save: the Settings folder is offered first. `null` means the user cancelled. */
  private askDirectory(requests: Parameters<typeof api.queueAdd>[0]): Promise<string | null> {
    const files = requests.reduce((sum, request) => sum + request.fileIds.length, 0);
    return askDestination({ files, directory: app.settings.outputDirectory });
  }

  /** New queue rows start downloading right away; other queued, paused or stopped rows are left alone. */
  private async queueAndStart(requests: Parameters<typeof api.queueAdd>[0], directory: string) {
    const created = await app.run(() => api.queueAdd(requests.map((request) => ({ ...request, outputDirectory: directory }))));
    if (!created?.length) return;
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
    if (this.clearing) return;
    this.clearing = true;
    try {
      const cleared = await app.run(async () => { await api.libraryReset(); return true; });
      if (!cleared) return;
      app.clearLibraryView();
      this.input = '';
      this.source = 'all';
      this.resetNavigation();
    } finally {
      this.clearing = false;
    }
  }

  async add() {
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
