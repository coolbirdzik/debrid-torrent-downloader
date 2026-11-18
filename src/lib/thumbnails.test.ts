import { describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({ convertFileSrc: (path: string, protocol: string) => `${protocol}://localhost/${path}`, invoke: vi.fn() }));

import { buildEntries, draftId, remoteOf, thumbRequest, type BrowserQuery } from './browser';
import { frameTime } from './thumbnails.svelte';
import type { Draft, Job } from './types';

const file = (id: number, path: string) => ({ id, path, bytes: 100, selected: 1 });
const draft = (overrides: Partial<Draft> = {}): Draft => ({
  addedAt: null, provider: 'realDebrid', source: '', sourceKey: 'btih:AAA', name: 'Show', torrentId: null, remoteAccount: null,
  files: [file(1, '/Show/ep1.mkv'), file(2, '/Show/notes.txt')], excludedFileIds: [], downloadedFileIds: [], partialFileIds: [],
  isHidden: false, inspectionAttempts: 0, nextInspectionAt: null, inspectionError: null, cloud: null, ...overrides
});
const job = (overrides: Partial<Job> = {}): Job => ({
  id: 'j1', provider: 'realDebrid', source: '', sourceKey: 'BTIH:aaa', torrentId: null, remoteAccount: null, selectedFileIds: [1], outputDirectory: null, targetDirectory: null, flattenFiles: null, files: [], name: 'Show',
  state: 'completed', detail: { key: '', args: [] }, remoteProgress: 0, localProgress: 0, totalBytes: 0, isActive: false, isStopping: false, isRemoving: false, ...overrides
});
const query = (d: Draft): BrowserQuery => ({ location: { draft: draftId(d), path: 'Show', flat: false }, search: '', videosOnly: false, sort: 'name', header: null });
const entriesOf = (d: Draft, jobs: Job[] = []) => buildEntries([d], 'realDebrid', jobs, query(d));

describe('thumbnail sources', () => {
  it('uses the draft torrent while the Library owns it, and only for videos', () => {
    const d = draft({ torrentId: 'T1', remoteAccount: 'me' });
    const [video, notes] = entriesOf(d);
    expect(remoteOf(video, [])).toEqual({ torrentId: 'T1', account: 'me', select: true });
    expect(remoteOf(notes, [])).toBeNull();
    expect(thumbRequest(video, [], 'downloaded')).toEqual({ id: 'realDebrid|btih:aaa|1', provider: 'realDebrid', sourceKey: 'btih:AAA', fileId: 1, torrentId: 'T1', account: 'me' });
    expect(thumbRequest(notes, [], 'downloaded')).toBeNull();
  });

  it('finds a queued torrent through the job that selected the file, never selecting on it', () => {
    const d = draft();
    const [video] = entriesOf(d);
    expect(remoteOf(video, [job({ torrentId: 'T2', selectedFileIds: [9] })])).toBeNull();
    expect(remoteOf(video, [job({ torrentId: 'T2', remoteAccount: 'me' })])).toEqual({ torrentId: 'T2', account: 'me', select: false });
  });

  it('waits for the service to finish, but a downloaded file is read locally regardless', () => {
    const [cloudOnly] = entriesOf(draft({ torrentId: 'T1' }));
    expect(thumbRequest(cloudOnly, [], 'downloading')).toBeNull();
    expect(thumbRequest(cloudOnly, [], undefined)?.torrentId).toBe('T1');
    const [local] = entriesOf(draft({ downloadedFileIds: [1] }));
    expect(thumbRequest(local, [], undefined)).toMatchObject({ id: 'realDebrid|btih:aaa|1', torrentId: null });
    const [localAndCloud] = entriesOf(draft({ torrentId: 'T1', downloadedFileIds: [1] }));
    expect(thumbRequest(localAndCloud, [], 'downloading')).toMatchObject({ torrentId: null });
  });

  it('picks a frame past the intro, capped for long videos and inside short ones', () => {
    expect(frameTime(100)).toBe(15);
    expect(frameTime(7200)).toBe(300);
    expect(frameTime(7200, 1)).toBe(900);
    expect(frameTime(0.4)).toBe(0);
    expect(frameTime(NaN)).toBe(5);
    expect(frameTime(Infinity, 1)).toBe(30);
  });
});
