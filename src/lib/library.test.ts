import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { CloudUpdate, Draft, Job, Snapshot } from './types';
import { draftId } from './browser';

const mocks = vi.hoisted(() => ({
  bootstrap: vi.fn(), libraryRefresh: vi.fn(), libraryRemove: vi.fn(), libraryUpdate: vi.fn(), libraryRetry: vi.fn(), queueAdd: vi.fn(), queueStart: vi.fn(),
  updateSettings: vi.fn(), on: vi.fn()
}));
vi.mock('./api', () => ({ api: mocks, on: mocks.on, toMsg: (error: unknown) => ({ key: String(error), args: [] }) }));
vi.mock('@tauri-apps/api/app', () => ({ getVersion: () => Promise.resolve('test') }));
import { app } from './app.svelte';
import { library } from './library.svelte';
import { answer, dialog } from './confirm.svelte';
import { answerDestination, destinationDialog } from './destination.svelte';

const draft = (overrides: Partial<Draft> = {}): Draft => ({
  provider: 'realDebrid', sourceKey: 'btih:AAA', source: '', name: 'Show', addedAt: '2026-01-01T00:00:00Z',
  torrentId: null, remoteAccount: null, files: [], excludedFileIds: [], downloadedFileIds: [], partialFileIds: [],
  isHidden: false, inspectionAttempts: 0, nextInspectionAt: null, inspectionError: null, cloud: null, ...overrides
});
const ready = (overrides: Partial<Draft> = {}) => draft({ files: [{ id: 1, path: '/Show/video.mkv', bytes: 100, selected: 1 }], ...overrides });
const snapshot = (drafts: Draft[]): Snapshot => ({
  settings: { ...app.settings, browserSort: 'added-desc', checkUpdates: false }, library: drafts, jobs: [],
  accounts: [], account: { provider: 'realDebrid', hasToken: false, user: null }, warning: null, dataFile: 'library-test'
});
let emitDraft: (value: Draft) => void;
let emitCloud: (value: CloudUpdate[]) => void;

beforeEach(async () => {
  vi.resetAllMocks();
  mocks.bootstrap.mockResolvedValue(snapshot([draft()]));
  mocks.on.mockImplementation((name, handler) => {
    if (name === 'draft') emitDraft = handler;
    if (name === 'cloud') emitCloud = handler;
    return Promise.resolve(() => {});
  });
  library.resetNavigation();
  library.source = 'all';
  library.refreshing = false;
  library.clearing = false;
  app.page = 'library';
  await app.init();
});

describe('Library removal', () => {
  it('requires confirmation for a cloud removal and sends only the selected provider', async () => {
    const rd = ready();
    app.apply({ ...snapshot([rd, ready({ provider: 'torBox' })]), settings: { ...app.settings, deleteCloudOnLibraryRemove: true } });
    library.select(library.entries.find(entry => entry.draft.provider === 'torBox')!, { ctrl: false, shift: false });
    const cancelled = library.removeSelected();
    expect(dialog.open).toBe(true);
    answer(false);
    await cancelled;
    expect(mocks.libraryRemove).not.toHaveBeenCalled();

    mocks.libraryRemove.mockResolvedValue({ undo: [], errors: [], deletedCloud: true, purged: [] });
    const approved = library.removeSelected();
    answer(true);
    await approved;
    expect(mocks.libraryRemove).toHaveBeenCalledWith([{
      provider: 'torBox', sourceKey: rd.sourceKey, isHidden: true, excludedFileIds: []
    }], true);
    expect(library.undoStack).toHaveLength(0);
  });

  it('keeps cloud files when the setting is off and offers undo', async () => {
    app.apply({ ...snapshot([ready()]), settings: { ...app.settings, deleteCloudOnLibraryRemove: false } });
    const undo = { provider: 'realDebrid', sourceKey: draft().sourceKey, isHidden: false, excludedFileIds: [] };
    mocks.libraryRemove.mockResolvedValue({ undo: [undo], errors: [], deletedCloud: false, purged: [] });
    library.select(library.entries[0], { ctrl: false, shift: false });
    await library.removeSelected();
    expect(dialog.open).toBe(false);
    expect(mocks.libraryRemove).toHaveBeenCalledWith([{ ...undo, isHidden: true }], false);
    await library.undo();
    expect(mocks.libraryUpdate).toHaveBeenCalledWith([undo]);
  });
});

describe('Library import activity', () => {
  it('counts completed imports in the navigation badge rather than pending torrents or individual files', () => {
    expect(app.libraryBadges.newImports).toBe(0);
    emitDraft(draft({ sourceKey: 'btih:WAITING', inspectionError: { key: 'Retry later', args: [] } }));
    expect(app.libraryBadges.newImports).toBe(0);
    emitDraft(ready({ files: [
      { id: 1, path: '/Show/one.mkv', bytes: 100, selected: 1 },
      { id: 2, path: '/Show/two.mkv', bytes: 100, selected: 1 }
    ] }));
    expect(app.libraryBadges.newImports).toBe(1);
    emitDraft(ready({ sourceKey: 'btih:SECOND' }));
    expect(app.libraryBadges.newImports).toBe(2);
  });

  it('keeps the badge in sync with New rows while navigating and filtering', () => {
    emitDraft(ready());
    expect(app.libraryBadges.newImports).toBe(1);
    app.page = 'queue';
    expect(app.libraryBadges.newImports).toBe(1);
    app.page = 'library';
    library.source = 'torBox';
    library.setVideosOnly(true);
    library.setSearch('no match');
    expect(app.libraryBadges.newImports).toBe(1);
    emitDraft(ready({ isHidden: true }));
    expect(app.libraryBadges.newImports).toBe(0);
  });

  it('excludes removed files from the badge and clears it with the New labels', () => {
    emitDraft(ready());
    expect(app.libraryBadges.newImports).toBe(1);
    emitDraft(ready({ excludedFileIds: [1] }));
    expect(app.libraryBadges.newImports).toBe(0);
    emitDraft(ready());
    expect(app.libraryBadges.newImports).toBe(1);
    app.clearLibraryView();
    expect(app.libraryBadges.newImports).toBe(0);
  });

  it('marks a pending-to-ready transition once, without treating later marks as another import', () => {
    emitDraft(ready());
    const sequence = app.imported.get(draftId(draft()));
    expect(sequence).toBeGreaterThan(0);
    emitDraft(ready({ downloadedFileIds: [1] }));
    expect(app.imported.get(draftId(draft()))).toBe(sequence);
  });

  it('keeps startup files as the baseline and clears activity when resetting', () => {
    app.apply(snapshot([ready()]));
    expect(app.imported.size).toBe(0);
    emitDraft(ready({ sourceKey: 'btih:BBB' }));
    expect(app.imported.size).toBe(1);
    app.clearLibraryView();
    expect(app.imported.size).toBe(0);
  });
});

describe('New badge and the queue', () => {
  const queued = (fileIds: number[]): Job => ({
    id: 'q1', provider: 'realDebrid', source: '', sourceKey: 'btih:AAA', name: 'Show', torrentId: null, remoteAccount: null, selectedFileIds: fileIds,
    outputDirectory: null, targetDirectory: null, flattenFiles: null, files: [], state: 'queued', detail: { key: '', args: [] }, remoteProgress: 0,
    localProgress: 0, totalBytes: 0, isActive: false, isStopping: false, isRemoving: false
  });

  it('stops counting a torrent once all its files are queued', () => {
    emitDraft(ready({ files: [{ id: 1, path: '/Show/a.mkv', bytes: 1, selected: 1 }, { id: 2, path: '/Show/b.mkv', bytes: 1, selected: 1 }] }));
    expect(app.libraryBadges.newImports).toBe(1);
    app.jobs = [queued([1])];
    expect(app.libraryBadges.newImports).toBe(1);
    app.jobs = [queued([1, 2])];
    expect(app.libraryBadges.newImports).toBe(0);
  });
});

describe('Queueing from the Library', () => {
  const twoFiles = (overrides: Partial<Draft> = {}) => ready({ files: [{ id: 1, path: '/Show/a.mkv', bytes: 1, selected: 1 }, { id: 2, path: '/Show/b.mkv', bytes: 1, selected: 1 }], ...overrides });
  const queueRoot = async () => {
    library.select(library.entries[0], { ctrl: false, shift: false });
    const pending = library.queueSelected();
    await vi.waitFor(() => expect(destinationDialog.open).toBe(true));
    // Wrapped: returning the promise itself would make the caller wait for the whole queue action.
    return { pending };
  };

  beforeEach(() => {
    mocks.queueAdd.mockResolvedValue(['new']);
    mocks.queueStart.mockResolvedValue(undefined);
    mocks.updateSettings.mockImplementation((patch) => Promise.resolve({ ...app.settings, ...patch }));
  });

  it('skips downloaded files by default and re-queues them with Force download', async () => {
    app.apply({ ...snapshot([twoFiles({ downloadedFileIds: [1] })]), settings: { ...app.settings, outputDirectory: 'D:\\Downloads' } });
    let { pending } = await queueRoot();
    expect(destinationDialog).toMatchObject({ files: 1, downloaded: 1 });
    answerDestination(true);
    await pending;
    expect(mocks.queueAdd).toHaveBeenLastCalledWith([{ provider: 'realDebrid', sourceKey: 'btih:AAA', fileIds: [2], outputDirectory: 'D:\\Downloads', force: false }]);

    ({ pending } = await queueRoot());
    answerDestination(true, true);
    await pending;
    expect(mocks.queueAdd).toHaveBeenLastCalledWith([{ provider: 'realDebrid', sourceKey: 'btih:AAA', fileIds: [1, 2], outputDirectory: 'D:\\Downloads', force: true }]);
  });

  it('only Force download can confirm when every file is already downloaded', async () => {
    app.apply({ ...snapshot([twoFiles({ downloadedFileIds: [1, 2] })]), settings: { ...app.settings, outputDirectory: 'D:\\Downloads' } });
    const { pending } = await queueRoot();
    expect(destinationDialog.files).toBe(0);
    answerDestination(true);
    expect(destinationDialog.open).toBe(true);
    answerDestination(false);
    await pending;
    expect(mocks.queueAdd).not.toHaveBeenCalled();
  });

  it('saves the chosen folder as the default only when asked', async () => {
    app.apply({ ...snapshot([twoFiles()]), settings: { ...app.settings, outputDirectory: 'D:\\Downloads' } });
    let { pending } = await queueRoot();
    destinationDialog.directory = 'E:\\Movies';
    answerDestination(true);
    await pending;
    expect(mocks.updateSettings).not.toHaveBeenCalled();

    app.jobs = [];
    ({ pending } = await queueRoot());
    expect(destinationDialog.makeDefault).toBe(false);
    destinationDialog.directory = 'E:\\Movies';
    destinationDialog.makeDefault = true;
    answerDestination(true);
    await pending;
    expect(mocks.updateSettings).toHaveBeenCalledWith({ outputDirectory: 'E:\\Movies' });
  });
});

describe('Import retry', () => {
  it('retries only the selected failed imports', async () => {
    const failed = draft({ inspectionError: { key: 'Boom', args: [] } });
    const waiting = draft({ sourceKey: 'btih:WAIT' });
    const dead = draft({ sourceKey: 'btih:DEAD', cloud: { status: 'magnet_error', progress: 0, speed: null, seeders: null } });
    app.apply({ ...snapshot([failed, waiting, dead]), accounts: [{ provider: 'realDebrid', hasToken: true, user: null }] });
    library.toggleAll();
    expect(library.selectedImportErrors.map((d) => d.sourceKey).sort()).toEqual(['btih:AAA', 'btih:DEAD']);
    await library.retryImport(library.selectedImportErrors);
    expect(mocks.libraryRetry).toHaveBeenCalledWith(expect.arrayContaining([
      { provider: 'realDebrid', sourceKey: 'btih:AAA' }, { provider: 'realDebrid', sourceKey: 'btih:DEAD' }
    ]));
    expect(mocks.libraryRetry.mock.calls[0][0]).toHaveLength(2);
  });

  it('does not offer a retry for a service without a token', () => {
    app.apply(snapshot([draft({ inspectionError: { key: 'Boom', args: [] } })]));
    library.toggleAll();
    expect(library.selectedImportErrors).toEqual([]);
  });
});

describe('Library cloud progress', () => {
  it('uses the job that owns the selected file after queueing', () => {
    const source = ready();
    const first: Job = {
      id: 'j1', provider: source.provider, source: source.source, sourceKey: source.sourceKey, name: source.name,
      torrentId: 'r1', remoteAccount: 'user', selectedFileIds: [1], outputDirectory: null, targetDirectory: null,
      flattenFiles: null, files: [], state: 'waiting', detail: { key: '', args: [] }, remoteProgress: 29,
      localProgress: 0, totalBytes: 0, isActive: true, isStopping: false, isRemoving: false
    };
    app.apply({ ...snapshot([source]), jobs: [first, { ...first, id: 'j2', torrentId: 'r2', selectedFileIds: [2], state: 'downloading', remoteProgress: 100 }] });
    expect(library.cloudView(source, 1)?.progress).toBe(29);
    expect(library.cloudView(source, 2)).toMatchObject({ tone: 'ready', progress: 100 });
    expect(library.cloudView(source, 3)).toBeUndefined();
  });

  it('keeps the last service progress for files of paused, stopped or failed jobs', () => {
    const source = ready();
    const job: Job = {
      id: 'p1', provider: source.provider, source: source.source, sourceKey: source.sourceKey, name: source.name,
      torrentId: 'r1', remoteAccount: 'user', selectedFileIds: [1], outputDirectory: null, targetDirectory: null,
      flattenFiles: null, files: [], state: 'paused', detail: { key: '', args: [] }, remoteProgress: 37,
      localProgress: 0, totalBytes: 0, isActive: false, isStopping: false, isRemoving: false
    };
    app.apply({ ...snapshot([source]), jobs: [job, { ...job, id: 'p2', torrentId: 'r2', selectedFileIds: [2], state: 'failed', remoteProgress: 100 }] });
    expect(library.cloudView(source, 1)).toMatchObject({ tone: 'idle', progress: 37, label: 'Tạm dừng' });
    expect(library.cloudView(source, 2)).toMatchObject({ tone: 'ready', progress: 100 });
  });

  it('shows live progress during import backoff and isolates copies of the same torrent by provider', () => {
    const rd = draft({ torrentId: '1', inspectionError: { key: 'Retry later', args: [] } });
    const tb = ready({ provider: 'torBox', torrentId: '1' });
    app.apply(snapshot([rd, tb]));
    const cloud = { status: 'downloading', progress: 29, speed: null, seeders: null };
    emitCloud([{ ...rd, cloud }, { ...tb, cloud: { ...cloud, status: 'downloaded', progress: 100 } }]);
    expect(library.entries).toHaveLength(2);
    expect(library.cloudView(rd)).toMatchObject({ tone: 'busy', progress: 29 });
    expect(library.cloudView(tb)).toMatchObject({ tone: 'ready', progress: 100 });
    emitCloud([{ ...rd, cloud: { ...cloud, progress: 52 } }]);
    expect(library.cloudView(rd)?.progress).toBe(52);
    library.setSource('torBox');
    expect(library.entries.map(entry => library.cloudView(entry.draft)?.progress)).toEqual([100]);
  });
});

describe('Library refresh', () => {
  it('picks up a missed completion while preserving navigation, filters and selection', async () => {
    library.setVideosOnly(true);
    library.setSearch('video');
    const location = library.location;
    const selection = library.selection;
    mocks.libraryRefresh.mockResolvedValue(snapshot([ready()]));
    await library.refresh();
    expect(app.library[0].files).toHaveLength(1);
    expect(app.imported.has(draftId(draft()))).toBe(true);
    expect(library.videosOnly).toBe(true);
    expect(library.search).toBe('video');
    expect(library.location).toBe(location);
    expect(library.selection).toBe(selection);
    expect(library.refreshing).toBe(false);
  });

  it('prevents duplicate refreshes and does not overwrite newer live events', async () => {
    let resolve!: (value: Snapshot) => void;
    mocks.libraryRefresh.mockReturnValue(new Promise<Snapshot>((done) => { resolve = done; }));
    const first = library.refresh();
    await library.refresh();
    expect(mocks.libraryRefresh).toHaveBeenCalledTimes(1);
    expect(library.refreshing).toBe(true);
    emitDraft(ready());
    resolve(snapshot([draft()]));
    await first;
    expect(app.library[0].files).toHaveLength(1);
    expect(library.refreshing).toBe(false);
  });

  it('does not resurrect a library cleared during refresh', async () => {
    let resolve!: (value: Snapshot) => void;
    mocks.libraryRefresh.mockReturnValue(new Promise<Snapshot>((done) => { resolve = done; }));
    const request = library.refresh();
    app.clearLibraryView();
    resolve(snapshot([ready()]));
    await request;
    expect(app.library).toHaveLength(0);
    expect(app.imported.size).toBe(0);
  });

  it('preserves cloud progress received while a refresh is in flight', async () => {
    const initial = ready({ cloud: { status: 'downloading', progress: 10, speed: null, seeders: null } });
    app.apply(snapshot([initial]));
    let resolve!: (value: Snapshot) => void;
    mocks.libraryRefresh.mockReturnValue(new Promise<Snapshot>((done) => { resolve = done; }));
    const request = library.refresh();
    emitCloud([{ provider: initial.provider, sourceKey: initial.sourceKey, cloud: { ...initial.cloud!, progress: 80 } }]);
    resolve(snapshot([initial]));
    await request;
    expect(app.cloudOf(initial)?.progress).toBe(80);
  });

  it('releases the refresh button and reports a failed request', async () => {
    const toast = vi.spyOn(app, 'toast').mockImplementation(() => {});
    mocks.libraryRefresh.mockRejectedValue('Refresh failed');
    await library.refresh();
    expect(library.refreshing).toBe(false);
    expect(app.library[0].files).toHaveLength(0);
    expect(toast).toHaveBeenCalledWith({ key: 'Refresh failed', args: [] }, 'error');
    toast.mockRestore();
  });
});
