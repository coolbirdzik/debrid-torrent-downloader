import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({ install: vi.fn(), listen: vi.fn(), check: vi.fn(), toast: vi.fn() }));
vi.mock('./api', () => ({
  api: { installUpdate: mocks.install, checkUpdate: mocks.check },
  on: mocks.listen,
  toMsg: (error: unknown) => ({ key: String(error), args: [] })
}));
vi.mock('@tauri-apps/api/app', () => ({ getVersion: vi.fn() }));
import { app } from './app.svelte';

beforeEach(() => {
  vi.resetAllMocks();
  app.update = { current: '0.3.2', latest: '0.4.0', available: true, canInstall: true, pageUrl: null, downloadUrl: null, notes: '' };
  app.updating = false;
  app.checkingUpdate = false;
  app.updateError = null;
  app.updateProgress = null;
  app.toast = mocks.toast;
});

describe('in-app update flow', () => {
  it('subscribes before installation, displays progress, prevents duplicate installs and cleans up', async () => {
    const unlisten = vi.fn();
    let done!: () => void;
    const pending = new Promise<void>((resolve) => { done = resolve; });
    mocks.listen.mockResolvedValue(unlisten);
    mocks.install.mockImplementation(() => {
      expect(mocks.listen).toHaveBeenCalledWith('update-progress', expect.any(Function));
      return pending;
    });
    const updating = app.installUpdate();
    await Promise.resolve();
    expect(app.updating).toBe(true);
    mocks.listen.mock.calls[0][1]({ phase: 'downloading', downloaded: 50, total: 100 });
    expect(app.updateProgress?.downloaded).toBe(50);
    await app.installUpdate();
    await app.checkUpdate();
    expect(mocks.install).toHaveBeenCalledExactlyOnceWith('0.4.0');
    expect(mocks.check).not.toHaveBeenCalled();
    done();
    await updating;
    expect(unlisten).toHaveBeenCalledOnce();
    expect(app.updating).toBe(false);
    expect(app.updateProgress).toBeNull();
  });

  it('keeps the error visible, unsubscribes and permits retry after a failed download', async () => {
    const unlisten = vi.fn();
    mocks.listen.mockResolvedValue(unlisten);
    mocks.install.mockRejectedValueOnce('Network unavailable').mockResolvedValueOnce(undefined);
    await app.installUpdate();
    expect(app.updateError?.key).toBe('Network unavailable');
    expect(app.updating).toBe(false);
    expect(unlisten).toHaveBeenCalledOnce();
    await app.installUpdate();
    expect(mocks.install).toHaveBeenCalledTimes(2);
    expect(app.updateError).toBeNull();
  });

  it('does not install an unsigned legacy release or proceed when progress subscription fails', async () => {
    app.update!.canInstall = false;
    await app.installUpdate();
    expect(mocks.install).not.toHaveBeenCalled();
    app.update!.canInstall = true;
    mocks.listen.mockRejectedValueOnce('Listener unavailable');
    await app.installUpdate();
    expect(mocks.install).not.toHaveBeenCalled();
    expect(app.updateError?.key).toBe('Listener unavailable');
    expect(app.updating).toBe(false);
  });
});
