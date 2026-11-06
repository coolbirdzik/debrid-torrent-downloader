import { describe, expect, it } from 'vitest';
import { activeDrafts, breadcrumbs, buildEntries, draftId, filterFiles, formatBytes, isStatusDone, isStatusPartial, queueRequests, queuedFileCount, queuedLookup, remainingCount, remainingRequests, removalPatches, type BrowserQuery } from './browser';
import type { Draft, Job } from './types';

const file = (id: number, path: string, bytes = 100) => ({ id, path, bytes, selected: 1 });

function draft(overrides: Partial<Draft> = {}): Draft {
  return {
    addedAt: '2024-05-01T10:00:00Z', provider: 'realDebrid', source: 'magnet:?xt=urn:btih:AAA', sourceKey: 'btih:AAA', name: 'Show', torrentId: null, remoteAccount: null,
    files: [file(1, '/Show/S01/ep1.mkv'), file(2, '/Show/S01/ep2.mkv'), file(3, '/Show/S02/ep1.mkv'), file(4, '/Show/notes.txt', 5)],
    excludedFileIds: [], downloadedFileIds: [], partialFileIds: [], isHidden: false, inspectionAttempts: 0, nextInspectionAt: null, inspectionError: null, cloud: null, ...overrides
  };
}

const query = (overrides: Partial<BrowserQuery> = {}): BrowserQuery => ({ location: { draft: null, path: '', flat: false }, search: '', videosOnly: false, sort: 'name', header: null, ...overrides });
const job = (overrides: Partial<Job> = {}): Job => ({
  id: 'j1', provider: 'realDebrid', source: '', sourceKey: 'btih:AAA', torrentId: null, remoteAccount: null, selectedFileIds: [1], outputDirectory: null, targetDirectory: null, flattenFiles: null, files: [], name: 'Show',
  state: 'queued', detail: { key: '', args: [] }, remoteProgress: 0, localProgress: 0, totalBytes: 0, isActive: false, isStopping: false, isRemoving: false, ...overrides
});

describe('browser entries', () => {
  it('shows one row per torrent at home', () => {
    const entries = buildEntries([draft()], 'realDebrid', [], query());
    expect(entries.map((e) => [e.kind, e.name, e.fileCount])).toEqual([['folder', 'Show', 4]]);
  });

  it('lists child folders and direct files inside a torrent, folders first', () => {
    const d = draft();
    const loc = { draft: draftId(d), path: '', flat: false };
    const root = buildEntries([d], 'realDebrid', [], query({ location: loc }));
    expect(root.map((e) => e.name)).toEqual(['Show']);
    const show = buildEntries([d], 'realDebrid', [], query({ location: { ...loc, path: 'Show' } }));
    expect(show.map((e) => e.name)).toEqual(['S01', 'S02', 'notes.txt']);
    const season = buildEntries([d], 'realDebrid', [], query({ location: { ...loc, path: 'Show/S01' } }));
    expect(season.map((e) => e.name)).toEqual(['ep1.mkv', 'ep2.mkv']);
  });

  it('search and videos-only are recursive and respect exact folder boundaries', () => {
    const d = draft({ files: [file(1, '/Show/S01/ep1.mkv'), file(2, '/Show/S010/ep9.mp4'), file(3, '/Show/S01/notes.txt')] });
    const loc = { draft: draftId(d), path: 'Show/S01', flat: false };
    expect(buildEntries([d], 'realDebrid', [], query({ location: loc, videosOnly: true })).map((e) => e.name)).toEqual(['ep1.mkv']);
    expect(buildEntries([d], 'realDebrid', [], query({ search: 'ep9' })).map((e) => e.name)).toEqual(['ep9.mp4']);
  });

  it('excluded files and hidden torrents disappear; empty torrents disappear too', () => {
    const d = draft({ excludedFileIds: [1, 2, 3] });
    expect(buildEntries([d], 'realDebrid', [], query({ location: { draft: draftId(d), path: '', flat: true } })).map((e) => e.name)).toEqual(['notes.txt']);
    expect(activeDrafts([draft({ isHidden: true })], 'realDebrid')).toHaveLength(0);
    expect(activeDrafts([draft({ excludedFileIds: [1, 2, 3, 4] })], 'realDebrid')).toHaveLength(0);
    expect(activeDrafts([draft({ files: [] })], 'realDebrid')).toHaveLength(1); // still being read
    expect(activeDrafts([draft()], 'torBox')).toHaveLength(0);
  });

  it('computes folder completion from descendants', () => {
    const d = draft({ downloadedFileIds: [1, 2], partialFileIds: [3] });
    const loc = { draft: draftId(d), path: '', flat: false };
    const entries = buildEntries([d], 'realDebrid', [], query({ location: { ...loc, path: 'Show' } }));
    const s01 = entries.find((e) => e.name === 'S01')!;
    const s02 = entries.find((e) => e.name === 'S02')!;
    expect(isStatusDone(s01)).toBe(true);
    expect(isStatusPartial(s02)).toBe(true);
  });

  it('sorts by added date and by header column', () => {
    const older = draft({ sourceKey: 'btih:OLD', name: 'Older', addedAt: '2024-01-01T00:00:00Z' });
    const newer = draft({ sourceKey: 'btih:NEW', name: 'Newer', addedAt: '2024-06-01T00:00:00Z' });
    expect(buildEntries([older, newer], 'realDebrid', [], query({ sort: 'added-desc' })).map((e) => e.name)).toEqual(['Newer', 'Older']);
    expect(buildEntries([older, newer], 'realDebrid', [], query({ sort: 'added-asc' })).map((e) => e.name)).toEqual(['Older', 'Newer']);
    expect(buildEntries([older, newer], 'realDebrid', [], query({ header: { column: 'name', direction: 'desc' } })).map((e) => e.name)).toEqual(['Older', 'Newer']);
    expect(buildEntries([older, newer], 'realDebrid', [], query({ header: { column: 'added', direction: 'desc' } })).map((e) => e.name)).toEqual(['Newer', 'Older']);
  });

  it('keeps the chosen order even when download status differs', () => {
    const older = draft({ sourceKey: 'btih:OLD', name: 'Older', addedAt: '2024-01-01T00:00:00Z' });
    const newer = draft({ sourceKey: 'btih:NEW', name: 'Newer', addedAt: '2024-06-01T00:00:00Z', downloadedFileIds: [1, 2, 3, 4] });
    expect(buildEntries([older, newer], 'realDebrid', [], query({ sort: 'added-desc' })).map((e) => e.name)).toEqual(['Newer', 'Older']);
    expect(buildEntries([older, newer], 'realDebrid', [], query({ sort: 'added-asc' })).map((e) => e.name)).toEqual(['Older', 'Newer']);
    expect(buildEntries([older, newer], 'realDebrid', [], query({ header: { column: 'name', direction: 'asc' } })).map((e) => e.name)).toEqual(['Newer', 'Older']);
  });

  it('sorts names naturally (Ep 2 before Ep 10)', () => {
    const d = draft({ files: [file(1, '/Ep 10.mkv'), file(2, '/Ep 2.mkv')] });
    const loc = { draft: draftId(d), path: '', flat: false };
    expect(buildEntries([d], 'realDebrid', [], query({ location: loc, sort: 'name' })).map((e) => e.name)).toEqual(['Ep 2.mkv', 'Ep 10.mkv']);
  });
});

describe('queueing', () => {
  it('counts only existing queued files, once for overlapping folder/file selections', () => {
    const d = draft();
    const [root] = buildEntries([d], 'all', [], query());
    const files = buildEntries([d], 'all', [], query({ location: { draft: draftId(d), path: '', flat: true } }));
    const lookup = queuedLookup([job({ selectedFileIds: [1] })], [d]);
    expect(queuedFileCount([root, files[0], root], lookup)).toBe(1);
    expect(queueRequests([root, files[0]], lookup)[0].fileIds).toEqual([2, 3, 4]);
    expect(queuedFileCount([root], queuedLookup([]))).toBe(0);
    expect(queuedFileCount([root], queuedLookup([job({ selectedFileIds: null })]))).toBe(4);
  });

  it('knows which files are already queued, including all-files jobs', () => {
    const d = draft();
    expect(queuedLookup([job({ selectedFileIds: [1] })])(d, 1)).toBe(true);
    expect(queuedLookup([job({ selectedFileIds: [1] })])(d, 2)).toBe(false);
    expect(queuedLookup([job({ selectedFileIds: null })])(d, 99)).toBe(true);
    expect(queuedLookup([job({ provider: 'torBox' })])(d, 1)).toBe(false);
  });

  it('builds per-torrent requests that skip queued files and expand folders', () => {
    const d = draft();
    const entries = buildEntries([d], 'realDebrid', [], query({ location: { draft: draftId(d), path: 'Show', flat: false } }));
    const season = entries.find((e) => e.name === 'S01')!;
    const lookup = queuedLookup([job({ selectedFileIds: [1] })]);
    expect(queueRequests([season], lookup)).toEqual([{ provider: 'realDebrid', sourceKey: 'btih:AAA', fileIds: [2] }]);
    expect(remainingCount([d], 'realDebrid', [job({ selectedFileIds: [1, 2] })])).toBe(2);
  });
});

describe('removal and undo', () => {
  it('excludes files and folders, hides torrent roots, and restores the previous state', () => {
    const d = draft({ excludedFileIds: [4] });
    const entries = buildEntries([d], 'realDebrid', [], query({ location: { draft: draftId(d), path: 'Show', flat: false } }));
    const season = entries.find((e) => e.name === 'S01')!;
    const { apply, undo } = removalPatches([season]);
    expect(apply[0].excludedFileIds.sort()).toEqual([1, 2, 4]);
    expect(undo[0].excludedFileIds).toEqual([4]);
    const [root] = buildEntries([d], 'realDebrid', [], query());
    const hidden = removalPatches([root]);
    expect(hidden.apply[0].isHidden).toBe(true);
    expect(hidden.undo[0].isHidden).toBe(false);
  });
});

describe('helpers', () => {
  it('builds breadcrumbs, filters files and formats sizes', () => {
    const d = draft();
    expect(breadcrumbs({ draft: draftId(d), path: 'Show/S01', flat: false }, [d]).map((c) => c.label)).toEqual([null, 'Show', 'Show', 'S01']);
    expect(filterFiles(d.files, 'S01 ep2', false).map((f) => f.id)).toEqual([2]);
    expect(formatBytes(1536)).toBe('1.5 KB');
    expect(formatBytes(0)).toBe('0 B');
  });

  it('copes with a 30,000-file library quickly', () => {
    const files = Array.from({ length: 30000 }, (_, i) => file(i + 1, `/Big/Season ${i % 50}/Episode ${String(i).padStart(5, '0')}.mkv`));
    const d = draft({ files, name: 'Big' });
    const started = performance.now();
    const entries = buildEntries([d], 'realDebrid', [], query({ location: { draft: draftId(d), path: '', flat: true } }));
    const searched = buildEntries([d], 'realDebrid', [], query({ search: 'episode 0123' }));
    expect(entries).toHaveLength(30000);
    expect(searched.length).toBeGreaterThan(0);
    expect(performance.now() - started).toBeLessThan(2000);
  });
});

describe('several services', () => {
  // The same torrent on TorBox: other file IDs and paths, same names and sizes.
  const onTorBox = () => draft({ provider: 'torBox', files: [file(11, '/Show/S01/ep1.mkv'), file(12, '/Show/S01/ep2.mkv'), file(13, '/Show/S02/ep1.mkv'), file(14, '/Show/notes.txt', 5)] });

  it('shows every service unless a source is picked', () => {
    const library = [draft(), onTorBox()];
    expect(activeDrafts(library)).toHaveLength(2);
    expect(activeDrafts(library, 'torBox').map((d) => d.provider)).toEqual(['torBox']);
    expect(buildEntries(library, 'all', [], query())).toHaveLength(2);
  });

  it('downloads a file held on two services only once, from the better-ranked one', () => {
    const library = [draft(), onTorBox()];
    expect(remainingCount(library, 'all', [])).toBe(4);
    const ranked = remainingRequests(library, 'all', [], (d) => (d.provider === 'torBox' ? 1 : 0));
    expect(ranked).toEqual([{ provider: 'torBox', sourceKey: 'btih:AAA', fileIds: [11, 12, 13, 14] }]);
    const entries = buildEntries(library, 'all', [], query());
    expect(queueRequests(entries, queuedLookup([], library)).map((r) => r.provider)).toEqual(['realDebrid']);
  });

  it('treats a file queued on one service as queued on the other', () => {
    const library = [draft(), onTorBox()];
    const lookup = queuedLookup([job({ selectedFileIds: [1] })], library);
    expect(lookup(library[1], 11, library[1].files[0])).toBe(true);
    expect(lookup(library[1], 12, library[1].files[1])).toBe(false);
    expect(remainingCount(library, 'all', [job({ selectedFileIds: [1] })])).toBe(3);
    const entries = buildEntries(library, 'all', [], query());
    expect(queuedFileCount(entries, lookup)).toBe(1);
    expect(queuedFileCount(entries.filter((entry) => entry.draft.provider === 'torBox'), lookup)).toBe(1);
  });
});
