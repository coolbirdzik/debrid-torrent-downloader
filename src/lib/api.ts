import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { AccountInfo, AddResult, CloudKind, CloudUpdate, DeleteOutcome, Draft, Job, LibraryPatch, Marks, Msg, Provider, RemoteTorrent, Settings, Snapshot, TorBoxAccount, UpdateInfo, User } from './types';

/** The backend rejects with `{ key, args }`; anything else (a runtime failure) becomes a plain message. */
export function toMsg(error: unknown): Msg {
  if (error && typeof error === 'object' && 'key' in error) return error as Msg;
  return { key: String(error), args: [] };
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw toMsg(error);
  }
}

export const api = {
  bootstrap: () => call<Snapshot>('bootstrap'),
  updateSettings: (patch: Partial<Settings>) => call<Settings>('update_settings', { patch }),
  saveToken: (token: string) => call<Msg | null>('save_token', { token }),
  connect: (provider?: Provider) => call<User>('connect', { provider: provider ?? null }),
  libraryAdd: (text: string, providers: Provider[]) => call<AddResult>('library_add', { text, providers }),
  libraryUpdate: (patches: LibraryPatch[]) => call<void>('library_update', { patches }),
  libraryReset: () => call<void>('library_reset'),
  libraryCancelInspect: () => call<void>('library_cancel_inspect'),
  libraryRetry: () => call<void>('library_retry'),
  queueAdd: (items: { provider: Provider; sourceKey: string; fileIds: number[]; outputDirectory?: string }[]) => call<string[]>('queue_add', { items }),
  queueStart: (ids?: string[]) => call<void>('queue_start', { ids: ids ?? null }),
  queuePause: () => call<void>('queue_pause'),
  queueStop: (ids?: string[]) => call<void>('queue_stop', { ids: ids ?? null }),
  queueRemove: (ids: string[]) => call<string[]>('queue_remove', { ids }),
  queueClear: (onlyCompleted: boolean) => call<string[]>('queue_clear', { onlyCompleted }),
  torboxAccount: () => call<TorBoxAccount>('torbox_account'),
  remoteList: (provider: Provider, kind: CloudKind = 'torrent') => call<{ account: string; torrents: RemoteTorrent[] }>('remote_list', { provider, kind }),
  remoteDelete: (provider: Provider, ids: string[], listAccount: string, kind: CloudKind = 'torrent') => call<DeleteOutcome[]>('remote_delete', { provider, kind, ids, listAccount }),
  openFolder: (path: string) => call<void>('open_folder', { path }),
  openTokenPage: (provider: Provider) => call<void>('open_token_page', { provider }),
  checkUpdate: () => call<UpdateInfo>('check_update'),
  installUpdate: (version: string) => call<void>('install_update', { version }),
  openRelease: (url: string) => call<void>('open_release', { url })
};

export interface Events {
  'update-progress': import('./types').UpdateProgress;
  jobs: Job[];
  'jobs-removed': string[];
  draft: Draft;
  marks: Marks[];
  inspect: { running: boolean; done: number; total: number };
  status: Msg;
  account: AccountInfo;
  cloud: CloudUpdate[];
}

export async function on<K extends keyof Events>(event: K, handler: (payload: Events[K]) => void): Promise<UnlistenFn> {
  return listen<Events[K]>(event, (e) => handler(e.payload));
}
