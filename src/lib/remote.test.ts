import { describe, expect, it, vi } from 'vitest';
import { RemoteBrowser, remoteKey, remoteQueuedKeys } from './remote';
import type { AccountInfo, CloudKind, Job, RemoteTorrent } from './types';

const torrent: RemoteTorrent = { id: 'same-id', filename: 'Movie', bytes: 100, progress: 100, status: 'downloaded', added: null, speed: null, seeders: null };
const response = (account: string) => ({ account, torrents: [torrent] });
const deferred = <T>() => {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => { resolve = done; });
  return { promise, resolve };
};

describe('remote torrents across accounts', () => {
  it('selects ranges only inside the visible provider and freezes selection during review', async () => {
    const api = {
      remoteList: vi.fn(async () => ({ account: 'user', torrents: Array.from({ length: 5 }, (_, index) => ({ ...torrent, id: String(index) })) })),
      remoteDelete: vi.fn()
    };
    const remote = new RemoteBrowser(api, () => {});
    await remote.load(['realDebrid', 'torBox']);
    const visible = remote.state.rows.filter((row) => row.provider === 'torBox');
    remote.toggle(visible[1], false, visible);
    remote.toggle(visible[4], true, visible);
    expect([...remote.state.selected]).toEqual(visible.slice(1).map(remoteKey));
    remote.toggle(visible[2], true, visible);
    expect([...remote.state.selected]).toEqual(visible.slice(3).map(remoteKey));
    remote.pick(visible[0], visible, {});
    remote.pick(visible[4], visible, { shift: true });
    remote.pick(visible[2], visible, { shift: true });
    expect([...remote.state.selected]).toEqual(visible.slice(0, 3).map(remoteKey));
    remote.review();
    remote.toggle(visible[4], true, visible);
    remote.toggleAll(visible);
    expect(remote.state.reviewing).toEqual(visible.slice(0, 3));
    expect([...remote.state.selected]).toEqual(visible.slice(0, 3).map(remoteKey));
  });

  it('orders cloud items by newest added time across providers and types, with unknown dates last', async () => {
    const api = {
      remoteList: vi.fn(async (provider: string, kind?: CloudKind) => ({ account: 'user', torrents: [{
        ...torrent, id: `${provider}-${kind}`,
        added: provider === 'realDebrid' ? '2026-11-01T00:00:00Z' : kind === 'torrent' ? '2026-11-07T00:00:00Z' : kind === 'webDownload' ? null : 'invalid'
      }] })), remoteDelete: vi.fn()
    };
    const remote = new RemoteBrowser(api, () => {});
    await remote.load(['realDebrid', 'torBox'], 'all');
    expect(remote.state.rows.map((row) => row.id)).toEqual(['torBox-torrent', 'realDebrid-torrent', 'torBox-webDownload', 'torBox-usenet']);
  });

  it('shows a fast service while another list is still pending', async () => {
    const slow = deferred<ReturnType<typeof response>>();
    const fast = deferred<ReturnType<typeof response>>();
    const api = { remoteList: vi.fn().mockReturnValueOnce(slow.promise).mockReturnValueOnce(fast.promise), remoteDelete: vi.fn() };
    const remote = new RemoteBrowser(api, () => {});
    const loading = remote.load(['realDebrid', 'torBox']);
    fast.resolve(response('tb-user'));
    await fast.promise;
    expect(remote.state.rows.map((row) => row.provider)).toEqual(['torBox']);
    expect(remote.state.loading).toBe(true);
    slow.resolve(response('rd-user'));
    await loading;
    expect(remote.state.rows.map((row) => row.provider)).toEqual(['realDebrid', 'torBox']);
    expect(remote.state.loading).toBe(false);
  });

  it('does not restart lists on first verification, but clears them when a token or known account changes', async () => {
    const api = { remoteList: vi.fn(async () => response('user')), remoteDelete: vi.fn() };
    const remote = new RemoteBrowser(api, () => {});
    const account: AccountInfo = { provider: 'realDebrid', hasToken: true, user: null };
    await remote.syncAccounts([account]);
    const verified = { ...account, user: { username: 'user', kind: 'premium', premium: 1000, allows_downloads: true } };
    await remote.syncAccounts([verified]);
    expect(api.remoteList).toHaveBeenCalledTimes(1);
    await remote.syncAccounts([{ ...verified, user: { ...verified.user, username: 'USER' } }]);
    expect(api.remoteList).toHaveBeenCalledTimes(1);
    await remote.syncAccounts([account]);
    expect(api.remoteList).toHaveBeenCalledTimes(2);
    await remote.syncAccounts([{ ...account, hasToken: false }]);
    expect(remote.state.rows).toEqual([]);
    expect(remote.state.selected.size).toBe(0);
  });

  it('lists multiple services independently and keeps successful lists when another service fails', async () => {
    const api = {
      remoteList: vi.fn(async (provider: string) => {
        if (provider === 'allDebrid') throw { key: 'invalid token', args: [] };
        return response(provider === 'torBox' ? 'tb-user' : 'rd-user');
      }),
      remoteDelete: vi.fn()
    };
    const remote = new RemoteBrowser(api, () => {});
    await remote.load(['realDebrid', 'torBox', 'allDebrid']);
    expect(remote.state.rows.map((row) => [row.provider, row.account, row.id])).toEqual([
      ['realDebrid', 'rd-user', 'same-id'], ['torBox', 'tb-user', 'same-id']
    ]);
    expect(new Set(remote.state.rows.map(remoteKey)).size).toBe(2);
    expect(remote.state.errors).toEqual({ allDebrid: { key: 'invalid token', args: [] } });
    expect(remote.state.loading).toBe(false);
  });

  it('ignores a late list after a reload or page exit', async () => {
    const old = deferred<ReturnType<typeof response>>();
    const api = { remoteList: vi.fn().mockReturnValueOnce(old.promise).mockResolvedValue(response('new-user')), remoteDelete: vi.fn() };
    const changed = vi.fn();
    const remote = new RemoteBrowser(api, changed);
    const pending = remote.load(['realDebrid']);
    await remote.load(['torBox']);
    old.resolve(response('old-user'));
    await pending;
    expect(remote.state.rows[0].account).toBe('new-user');
    expect(remote.state.rows[0].provider).toBe('torBox');

    const exiting = deferred<ReturnType<typeof response>>();
    api.remoteList.mockReturnValueOnce(exiting.promise);
    const loading = remote.load(['realDebrid']);
    remote.invalidate();
    changed.mockClear();
    exiting.resolve(response('old-user'));
    await loading;
    expect(changed).not.toHaveBeenCalled();
  });

  it('sends deletion to each captured provider and account, isolating matching IDs and failures', async () => {
    const error = { key: 'token changed', args: [] };
    const api = {
      remoteList: vi.fn(async (provider: string) => response(provider === 'torBox' ? 'tb-user' : 'rd-user')),
      remoteDelete: vi.fn(async (provider: string) => {
        if (provider === 'realDebrid') throw error;
        return [{ id: torrent.id, ok: true, message: null }];
      })
    };
    const remote = new RemoteBrowser(api, () => {});
    await remote.load(['realDebrid', 'torBox']);
    const [rd, tb] = remote.state.rows;
    remote.toggleAll(remote.state.rows);
    remote.review();
    expect(await remote.confirmDelete()).toBe(1);
    expect(api.remoteDelete.mock.calls).toEqual([
      ['realDebrid', ['same-id'], 'rd-user', 'torrent'], ['torBox', ['same-id'], 'tb-user', 'torrent']
    ]);
    expect(remote.state.rows).toEqual([rd]);
    expect(remote.state.selected).toEqual(new Set([remoteKey(rd)]));
    expect(remote.state.failures).toEqual({ [remoteKey(rd)]: error });
    expect(remote.state.failures[remoteKey(tb)]).toBeUndefined();
    expect(remote.state.reviewing).toBeNull();
    expect(remote.state.deleting).toBe(false);
  });

  it('reviews only selected rows and prevents changing the reviewed selection', async () => {
    const api = { remoteList: vi.fn(async (provider: string) => response(provider)), remoteDelete: vi.fn() };
    const remote = new RemoteBrowser(api, () => {});
    await remote.load(['realDebrid', 'torBox']);
    const [rd, tb] = remote.state.rows;
    remote.toggle(rd);
    remote.review();
    remote.toggle(tb);
    remote.toggleAll(remote.state.rows);
    expect(remote.state.reviewing).toEqual([rd]);
    expect(remote.state.selected).toEqual(new Set([remoteKey(rd)]));
    remote.cancelReview();
    remote.clearSelection();
    expect(remote.state.selected.size).toBe(0);
  });

  it('ignores an old delete response after reload and does not delete the remaining groups', async () => {
    const pending = deferred<{ id: string; ok: boolean; message: null }[]>();
    const api = { remoteList: vi.fn(async () => response('user')), remoteDelete: vi.fn(() => pending.promise) };
    const remote = new RemoteBrowser(api, () => {});
    await remote.load(['realDebrid', 'torBox']);
    remote.toggleAll(remote.state.rows);
    remote.review();
    const deleting = remote.confirmDelete();
    await remote.load(['realDebrid', 'torBox']);
    pending.resolve([{ id: torrent.id, ok: true, message: null }]);
    await deleting;
    expect(remote.state.rows).toHaveLength(2);
    expect(api.remoteDelete).toHaveBeenCalledTimes(1);
    expect(remote.state.deleting).toBe(false);
  });

  it('matches queue badges by provider, account and ID, ignoring username casing', () => {
    const row = { ...torrent, provider: 'realDebrid' as const, account: 'rd-user' };
    const job = { provider: 'realDebrid', torrentId: 'same-id', remoteAccount: 'RD-User' } as Job;
    const keys = remoteQueuedKeys([job]);
    expect(keys.has(remoteKey(row))).toBe(true);
    expect(keys.has(remoteKey({ ...row, provider: 'torBox' }))).toBe(false);
    expect(keys.has(remoteKey({ ...row, account: 'someone-else' }))).toBe(false);
    expect(keys.has(remoteKey({ ...row, kind: 'webDownload' }))).toBe(false);
    expect(remoteQueuedKeys([{ ...job, remoteAccount: null }]).size).toBe(0);
  });

  it('reviews and deletes every visible cloud item with its exact type, even when IDs overlap', async () => {
    const api = {
      remoteList: vi.fn(async () => response('user')),
      remoteDelete: vi.fn(async (_provider: string, ids: string[], _account: string, _kind?: CloudKind) => ids.map((id) => ({ id, ok: true, message: null })))
    };
    const remote = new RemoteBrowser(api, () => {});
    await remote.load(['realDebrid', 'torBox'], 'all');
    expect(remote.state.rows).toHaveLength(4);
    expect(new Set(remote.state.rows.map(remoteKey)).size).toBe(4);
    const visible = remote.state.rows.filter((row) => row.provider === 'torBox');
    remote.reviewAll(visible);
    expect(remote.state.reviewing).toEqual(visible);
    expect(await remote.confirmDelete()).toBe(3);
    expect(api.remoteDelete.mock.calls).toEqual([
      ['torBox', ['same-id'], 'user', 'torrent'],
      ['torBox', ['same-id'], 'user', 'webDownload'],
      ['torBox', ['same-id'], 'user', 'usenet']
    ]);
    expect(remote.state.rows.map((row) => row.provider)).toEqual(['realDebrid']);
  });

  it('keeps successful types visible and identifies the failed type separately', async () => {
    const api = {
      remoteList: vi.fn(async (_provider: string, kind?: CloudKind) => {
        if (kind === 'webDownload') throw { key: 'webdl unavailable', args: [] };
        return response('user');
      }), remoteDelete: vi.fn()
    };
    const remote = new RemoteBrowser(api, () => {});
    await remote.load(['torBox'], 'all');
    expect(remote.state.rows.map((row) => row.kind)).toEqual(['torrent', 'usenet']);
    expect(remote.state.errors).toEqual({ 'torBox|webDownload': { key: 'webdl unavailable', args: [] } });
    await remote.load(['realDebrid', 'torBox'], 'usenet');
    expect(remote.state.rows.map((row) => [row.provider, row.kind])).toEqual([['torBox', 'usenet']]);
  });
});
