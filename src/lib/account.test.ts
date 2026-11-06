import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { User } from './types';

const mocks = vi.hoisted(() => ({ connect: vi.fn(), saveToken: vi.fn() }));
vi.mock('./api', () => ({ api: mocks, toMsg: (error: unknown) => ({ key: String(error), args: [] }) }));
vi.mock('@tauri-apps/api/app', () => ({ getVersion: vi.fn() }));
import { app } from './app.svelte';

const user = (username = 'rd-user'): User => ({ username, kind: 'premium', premium: 3600, allows_downloads: false, email: 'me@example.com', expiration: '2026-12-07T12:00:00Z', points: 0 });
const pending = () => {
  let resolve!: (user: User) => void;
  const promise = new Promise<User>((done) => { resolve = done; });
  return { promise, resolve };
};

beforeEach(() => {
  vi.resetAllMocks();
  const accounts = [
    { provider: 'realDebrid' as const, hasToken: true, user: user() },
    { provider: 'torBox' as const, hasToken: true, user: { ...user('tb-user'), kind: 'TorBox · Pro' } }
  ];
  app.apply({ settings: { ...app.settings, provider: 'torBox' }, accounts, account: accounts[1], library: [], jobs: [], warning: null, dataFile: 'account-test' });
});

describe('provider account refresh', () => {
  it('refreshes Real-Debrid independently of the service selected in Settings', async () => {
    mocks.connect.mockResolvedValue(user('refreshed-rd'));
    await app.refreshAccount('realDebrid');
    expect(mocks.connect).toHaveBeenCalledExactlyOnceWith('realDebrid');
    expect(app.accounts[0].user?.username).toBe('refreshed-rd');
    expect(app.accounts[0].user?.points).toBe(0);
    expect(app.settings.provider).toBe('torBox');
    expect(app.user?.username).toBe('tb-user');
  });

  it('prevents concurrent refreshes of the same account', async () => {
    const response = pending();
    mocks.connect.mockReturnValue(response.promise);
    const first = app.refreshAccount('realDebrid');
    await app.refreshAccount('realDebrid');
    expect(mocks.connect).toHaveBeenCalledTimes(1);
    expect(app.accountRefreshing.realDebrid).toBe(true);
    response.resolve(user());
    await first;
    expect(app.accountRefreshing.realDebrid).toBe(false);
  });

  it('keeps the known account visible on an error and clears the error after retry', async () => {
    mocks.connect.mockRejectedValueOnce('Network unavailable').mockResolvedValueOnce(user('retried'));
    await app.refreshAccount('realDebrid');
    expect(app.accounts[0].user?.username).toBe('rd-user');
    expect(app.accountErrors.realDebrid?.key).toBe('Network unavailable');
    await app.refreshAccount('realDebrid');
    expect(app.accountErrors.realDebrid).toBeNull();
    expect(app.accounts[0].user?.username).toBe('retried');
  });

  it('ignores responses for a replaced token without clearing the new request loading state', async () => {
    app.settings = { ...app.settings, provider: 'realDebrid' };
    const old = pending(), fresh = pending();
    mocks.connect.mockReturnValueOnce(old.promise).mockReturnValueOnce(fresh.promise);
    mocks.saveToken.mockResolvedValue(null);
    const first = app.refreshAccount('realDebrid');
    await app.saveToken('replacement-token');
    const second = app.refreshAccount('realDebrid');
    old.resolve(user('old-account'));
    await first;
    expect(app.accounts[0].user).toBeNull();
    expect(app.accountRefreshing.realDebrid).toBe(true);
    fresh.resolve(user('new-account'));
    await second;
    expect(app.accounts[0].user?.username).toBe('new-account');
    expect(app.accountRefreshing.realDebrid).toBe(false);
  });

  it('does not request disconnected accounts', async () => {
    app.accounts = app.accounts.map((account) => ({ ...account, hasToken: false, user: null }));
    await app.refreshAccount('realDebrid');
    expect(mocks.connect).not.toHaveBeenCalled();
  });
});
