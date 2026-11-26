import { describe, expect, it } from 'vitest';
import { activeDrafts, breadcrumbs, buildEntries, draftId, filterFiles, formatBytes, isDraftHandled, isHandled, isStatusDone, isStatusPartial, planQueue, queueRequests, queuedFileCount, queuedLookup, remainingCount, remainingRequests, removalPatches, downloadedRemoval, type BrowserQuery } from './browser';
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

  it('brings the latest completed imports ahead of newer pending torrents', () => {
    const first = draft({ sourceKey: 'btih:FIRST', name: 'First', addedAt: '2024-01-01T00:00:00Z' });
    const last = draft({ sourceKey: 'btih:LAST', name: 'Last', addedAt: '2024-02-01T00:00:00Z' });
    const pending = draft({ sourceKey: 'btih:PENDING', name: 'Pending', files: [], addedAt: '2024-06-01T00:00:00Z' });
    const imported = new Map([[draftId(first), 1], [draftId(last), 2]]);
    const rows = buildEntries([first, last, pending], 'all', [], query({ sort: 'added-desc', imported }));
    expect(rows.map((entry) => entry.name)).toEqual(['Last', 'First', 'Pending']);
  });

  it('uses import completion order for videos, flat files, search and provider filters', () => {
    const old = draft({ sourceKey: 'btih:OLD', name: 'Old', files: [file(1, '/Old/A.mkv'), file(2, '/Old/notes.txt')] });
    const fresh = draft({ provider: 'torBox', sourceKey: 'btih:FRESH', name: 'Fresh', files: [file(1, '/Fresh/Z.mkv'), file(2, '/Fresh/B.mkv')] });
    const imported = new Map([[draftId(fresh), 1]]);
    for (const mode of [{ videosOnly: true }, { location: { draft: null, path: '', flat: true } }, { search: '.mkv' }]) {
      const rows = buildEntries([old, fresh], 'all', [], query({ ...mode, sort: 'added-desc', imported }));
      expect(rows.slice(0, 2).map((entry) => entry.name)).toEqual(['B.mkv', 'Z.mkv']);
    }
    expect(buildEntries([old, fresh], 'realDebrid', [], query({ videosOnly: true, sort: 'added-desc', imported })).map((entry) => entry.name)).toEqual(['A.mkv']);
  });

  it('keeps explicit sorts ahead of import completion order', () => {
    const a = draft({ sourceKey: 'btih:A', name: 'A', addedAt: '2024-01-01T00:00:00Z' });
    const z = draft({ sourceKey: 'btih:Z', name: 'Z', addedAt: '2024-06-01T00:00:00Z' });
    const imported = new Map([[draftId(z), 1]]);
    for (const sort of ['name', 'added-asc'] as const) {
      expect(buildEntries([a, z], 'all', [], query({ sort, imported })).map((entry) => entry.name)).toEqual(['A', 'Z']);
    }
    for (const column of ['name', 'added'] as const) {
      expect(buildEntries([a, z], 'all', [], query({ sort: 'added-desc', header: { column, direction: 'asc' }, imported })).map((entry) => entry.name)).toEqual(['A', 'Z']);
    }
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

describe('already downloaded files', () => {
  const flat = (d: Draft) => buildEntries([d], 'all', [], query({ location: { draft: draftId(d), path: '', flat: true } }));

  it('skips downloaded files unless forced, even while their finished job is still in the queue', () => {
    const d = draft({ downloadedFileIds: [1] });
    const finished = job({ selectedFileIds: [1], state: 'completed' });
    const plan = planQueue(flat(d), [finished], [d]);
    expect(plan.fresh.map((r) => ({ ...r, fileIds: r.fileIds.sort() }))).toEqual([{ provider: 'realDebrid', sourceKey: 'btih:AAA', fileIds: [2, 3, 4] }]);
    expect(plan.forced[0].fileIds.sort()).toEqual([1, 2, 3, 4]);
    expect(plan.downloaded).toBe(1);
    expect(plan.duplicates).toBe(0);
  });

  it('offers only Force download when every selected file is downloaded', () => {
    const d = draft({ downloadedFileIds: [1, 2, 3, 4] });
    const plan = planQueue(flat(d), [], [d]);
    expect(plan.fresh).toEqual([]);
    expect(plan.downloaded).toBe(4);
  });

  it('does not force a second copy while one is still on its way, and still skips plain queued files', () => {
    const d = draft({ downloadedFileIds: [1] });
    const plan = planQueue(flat(d), [job({ selectedFileIds: [1, 2], state: 'downloading' })], [d]);
    expect(plan.forced).toEqual([{ provider: 'realDebrid', sourceKey: 'btih:AAA', fileIds: [3, 4] }]);
    expect(plan.downloaded).toBe(0);
    expect(plan.duplicates).toBe(2);
  });

  it('counts a file downloaded from one service as downloaded on the other', () => {
    const rd = draft({ downloadedFileIds: [1] });
    const tb = draft({ provider: 'torBox', files: [file(11, '/Show/S01/ep1.mkv'), file(12, '/Show/S01/ep2.mkv'), file(13, '/Show/S02/ep1.mkv'), file(14, '/Show/notes.txt', 5)] });
    const plan = planQueue(buildEntries([tb], 'torBox', [], query({ location: { draft: draftId(tb), path: '', flat: true } })), [], [rd, tb]);
    expect(plan.fresh.flatMap((r) => r.fileIds).sort()).toEqual([12, 13, 14]);
    expect(plan.downloaded).toBe(1);
  });

  it('leaves downloaded files out of "all remaining"', () => {
    expect(remainingCount([draft({ downloadedFileIds: [1, 2] })], 'all', [])).toBe(2);
  });
});

describe('New badge', () => {
  it('drops for queued, downloaded or partial files, and for folders once all their files are', () => {
    const d = draft({ downloadedFileIds: [3], partialFileIds: [4] });
    const jobs = [job({ selectedFileIds: [1] })];
    const files = buildEntries([d], 'all', jobs, query({ location: { draft: draftId(d), path: '', flat: true } }));
    expect(files.filter(isHandled).map((e) => e.file!.id).sort()).toEqual([1, 3, 4]);
    const folders = buildEntries([d], 'all', jobs, query({ location: { draft: draftId(d), path: 'Show', flat: false } }));
    expect(isHandled(folders.find((e) => e.name === 'S01')!)).toBe(false);
    expect(isHandled(folders.find((e) => e.name === 'S02')!)).toBe(true);
    const [root] = buildEntries([d], 'all', jobs, query());
    expect(isHandled(root)).toBe(false);
    const [all] = buildEntries([d], 'all', [job({ selectedFileIds: [1, 2] })], query());
    expect(isHandled(all)).toBe(true);
    expect(isDraftHandled(d, queuedLookup([job({ selectedFileIds: [1, 2] })], [d]))).toBe(true);
    expect(isDraftHandled(d, queuedLookup(jobs, [d]))).toBe(false);
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

describe('downloaded removal', () => {
  it('removes only the finished files of a partly downloaded torrent, and hides a finished torrent', () => {
    const partial = draft({ downloadedFileIds: [1, 4] });
    const complete = draft({ sourceKey: 'btih:BBB', name: 'Done', files: [file(1, '/Done/a.mkv'), file(2, '/Done/b.mkv')], downloadedFileIds: [1, 2] });
    const untouched = draft({ sourceKey: 'btih:CCC', name: 'New' });
    const { entries, files } = downloadedRemoval([partial, complete, untouched], 'all');
    expect(files).toBe(4);
    const { apply, undo } = removalPatches(entries);
    const of = (key: string) => apply.find((p) => p.sourceKey === key)!;
    expect(of('btih:AAA')).toMatchObject({ excludedFileIds: [1, 4], isHidden: false });
    expect(of('btih:BBB').isHidden).toBe(true);
    expect(apply.some((p) => p.sourceKey === 'btih:CCC')).toBe(false);
    expect(undo.find((p) => p.sourceKey === 'btih:AAA')).toMatchObject({ excludedFileIds: [], isHidden: false });
  });

  it('ignores files that are already removed and reports nothing when none are finished', () => {
    expect(downloadedRemoval([draft({ downloadedFileIds: [1], excludedFileIds: [1] })], 'all')).toEqual({ entries: [], files: 0 });
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

  it('lets a file that failed or was stopped on one service be queued on another', () => {
    const library = [draft(), onTorBox()];
    const done = { torrentFileId: 12, linkIndex: 0, link: '', relativePath: 'ep2.mkv', expectedBytes: 100, downloadedBytes: 100, completed: true };
    for (const state of ['failed', 'stopped'] as const) {
      const jobs = [job({ provider: 'torBox', selectedFileIds: [11, 12], state, files: [done] })];
      const lookup = queuedLookup(jobs, library);
      expect(lookup(library[0], 1, library[0].files[0])).toBe(false);
      // A file the job already finished still counts, and the same service still sees its own job.
      expect(lookup(library[0], 2, library[0].files[1])).toBe(true);
      expect(lookup(library[1], 11, library[1].files[0])).toBe(true);
      expect(queueRequests(buildEntries(library, 'realDebrid', [], query()), lookup)).toEqual([{ provider: 'realDebrid', sourceKey: 'btih:AAA', fileIds: [1, 3, 4] }]);
    }
    for (const state of ['queued', 'waiting', 'downloading', 'paused'] as const) {
      expect(queuedLookup([job({ provider: 'torBox', selectedFileIds: [11], state })], library)(library[0], 1, library[0].files[0])).toBe(true);
    }
  });
});
