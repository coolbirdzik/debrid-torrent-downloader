import { untrack } from 'svelte';
import { api } from './api';
import { app } from './app.svelte';
import { activeDrafts, buildEntries, queueRequests, queuedLookup, remainingCount, remainingRequests, removalPatches, visibleFiles, type Entry, type HeaderSort, type Location } from './browser';
import type { LibraryPatch } from './types';

const HOME: Location = { draft: null, path: '', flat: false };

class LibraryState {
  location = $state<Location>({ ...HOME });
  back = $state<Location[]>([]);
  forward = $state<Location[]>([]);
  rawSearch = $state('');
  search = $state('');
  videosOnly = $state(false);
  header = $state<HeaderSort>(null);
  selected = $state.raw<Set<string>>(new Set());
  anchor = $state<string | null>(null);
  undoStack = $state.raw<LibraryPatch[][]>([]);
  input = $state('');

  // Only membership of the queue matters to the rows, not progress: a string signature keeps the (large) row list from
  // being rebuilt on every progress event.
  private queueSignature = $derived(app.jobs.map((job) => `${job.id}:${job.provider}:${job.sourceKey}:${job.selectedFileIds?.length ?? 'all'}`).join(','));

  entries = $derived.by<Entry[]>(() => {
    this.queueSignature;
    const jobs = untrack(() => app.jobs);
    return buildEntries(app.library, app.settings.provider, jobs, {
      location: this.location, search: this.search, videosOnly: this.videosOnly, sort: app.settings.browserSort, header: this.header
    });
  });

  selectedEntries = $derived(this.entries.filter((entry) => this.selected.has(entry.key)));
  drafts = $derived(activeDrafts(app.library, app.settings.provider));
  remaining = $derived.by(() => {
    this.queueSignature;
    return remainingCount(app.library, app.settings.provider, untrack(() => app.jobs));
  });
  canQueue = $derived(this.selectedEntries.some((entry) => !entry.queued && (entry.kind === 'file' || entry.fileCount > 0)));
  selectedBytes = $derived(this.selectedEntries.reduce((sum, entry) => sum + entry.size, 0));
  pendingCount = $derived(this.drafts.filter((d) => d.files.length === 0).length);
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

  setVideosOnly(value: boolean) {
    this.videosOnly = value;
    this.clearSelection();
  }

  clearSelection() {
    this.selected = new Set();
    this.anchor = null;
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
  }

  /** Plain click selects one row, Ctrl toggles, Shift selects the range from the anchor. */
  select(entry: Entry, modifiers: { ctrl: boolean; shift: boolean }) {
    const next = new Set(modifiers.ctrl || modifiers.shift ? this.selected : []);
    if (modifiers.shift && this.anchor) {
      const from = this.entries.findIndex((e) => e.key === this.anchor);
      const to = this.entries.findIndex((e) => e.key === entry.key);
      if (from >= 0 && to >= 0) for (const e of this.entries.slice(Math.min(from, to), Math.max(from, to) + 1)) next.add(e.key);
    } else if (modifiers.ctrl && next.has(entry.key)) next.delete(entry.key);
    else next.add(entry.key);
    if (!modifiers.shift) this.anchor = entry.key;
    this.selected = next;
  }

  toggle(entry: Entry, shift = false) {
    if (shift && this.anchor) return this.select(entry, { ctrl: true, shift: true });
    const next = new Set(this.selected);
    if (!next.delete(entry.key)) next.add(entry.key);
    this.anchor = entry.key;
    this.selected = next;
  }

  toggleAll() {
    this.flush();
    this.selected = this.entries.every((e) => this.selected.has(e.key)) ? new Set() : new Set(this.entries.map((e) => e.key));
    this.anchor = null;
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
    const lookup = queuedLookup(untrack(() => app.jobs));
    const requests = queueRequests(this.selectedEntries, lookup);
    if (requests.length === 0) return;
    this.clearSelection();
    await this.queueAndStart(requests);
  }

  async queueRemaining() {
    this.flush();
    const requests = remainingRequests(app.library, app.settings.provider, untrack(() => app.jobs));
    if (requests.length === 0) return;
    this.clearSelection();
    await this.queueAndStart(requests);
  }

  /** New queue rows start downloading right away; other queued, paused or stopped rows are left alone. */
  private async queueAndStart(requests: Parameters<typeof api.queueAdd>[0]) {
    const created = await app.run(() => api.queueAdd(requests));
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
    await app.run(() => api.libraryReset());
    app.library = [];
    this.input = '';
    this.resetNavigation();
  }

  async add() {
    const text = this.input;
    if (!text.trim()) return;
    const result = await app.run(() => api.libraryAdd(text));
    if (!result) return;
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
