import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { CloudUpdate, Draft, Snapshot } from './types';
import { draftId } from './browser';

const mocks = vi.hoisted(() => ({ bootstrap: vi.fn(), libraryRefresh: vi.fn(), on: vi.fn() }));
vi.mock('./api', () => ({ api: mocks, on: mocks.on, toMsg: (error: unknown) => ({ key: String(error), args: [] }) }));
vi.mock('@tauri-apps/api/app', () => ({ getVersion: () => Promise.resolve('test') }));
import { app } from './app.svelte';
import { library } from './library.svelte';

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
