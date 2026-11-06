// Pure logic for the Library browser: which rows to show for a location, their completion marks, sorting, and the file
// sets behind queue/remove actions. Kept free of UI so it can be unit-tested and stays fast on 30,000-file libraries.

import type { Draft, Job, LibraryPatch, Provider, TorrentFile } from './types';

export type SortMode = 'name' | 'added-desc' | 'added-asc';
export type HeaderSort = { column: 'name' | 'size' | 'status'; direction: 'asc' | 'desc' } | null;

export interface Location {
  /** `draftId` of the open torrent, or null for the list of all torrents. */
  draft: string | null;
  path: string;
  flat: boolean;
}

export interface Entry {
  key: string;
  kind: 'folder' | 'file';
  draft: Draft;
  file: TorrentFile | null;
  /** For a folder row: its path inside the torrent (empty for a torrent root). */
  folderPath: string;
  name: string;
  location: string;
  size: number;
  fileCount: number;
  addedAt: number;
  total: number;
  downloaded: number;
  partial: number;
  queued: boolean;
  pending: boolean;
  videoKind: Kind;
}

export type Kind = 'folder' | 'video' | 'audio' | 'image' | 'archive' | 'document' | 'other';

const VIDEO = new Set(['mp4', 'mkv', 'avi', 'mov', 'wmv', 'webm', 'm4v', 'ts', 'm2ts', 'mts', 'mpg', 'mpeg', 'flv', 'vob', 'ogv', '3gp', 'divx', 'rmvb']);
const AUDIO = new Set(['mp3', 'flac', 'wav', 'aac', 'm4a', 'ogg', 'opus', 'wma']);
const IMAGE = new Set(['jpg', 'jpeg', 'png', 'gif', 'webp', 'bmp', 'tif', 'tiff', 'heic']);
const ARCHIVE = new Set(['zip', 'rar', '7z', 'tar', 'gz', 'bz2', 'xz', 'iso']);
const DOCUMENT = new Set(['txt', 'nfo', 'pdf', 'doc', 'docx', 'srt', 'ass', 'sub', 'epub']);

export const normalize = (path: string) => path.replace(/\\/g, '/');
export const baseName = (path: string) => normalize(path).split('/').filter(Boolean).pop() ?? '';

export function extension(path: string): string {
  const name = baseName(path);
  const dot = name.lastIndexOf('.');
  return dot >= 0 && dot + 1 < name.length ? name.slice(dot + 1).toLowerCase() : '';
}

export const isVideo = (path: string) => VIDEO.has(extension(path));

export function kindOf(path: string): Kind {
  const ext = extension(path);
  return VIDEO.has(ext) ? 'video' : AUDIO.has(ext) ? 'audio' : IMAGE.has(ext) ? 'image' : ARCHIVE.has(ext) ? 'archive' : DOCUMENT.has(ext) ? 'document' : 'other';
}

export const draftId = (draft: { provider: Provider; sourceKey: string }) => `${draft.provider}|${draft.sourceKey.toLowerCase()}`;

// Caches keyed by object identity: a draft is replaced (not mutated) whenever the backend sends an update.
const excludedCache = new WeakMap<number[], Set<number>>();
const lowerPathCache = new WeakMap<TorrentFile, string>();

function setOf(ids: number[]): Set<number> {
  let set = excludedCache.get(ids);
  if (!set) excludedCache.set(ids, (set = new Set(ids)));
  return set;
}

const lowerPath = (file: TorrentFile) => {
  let value = lowerPathCache.get(file);
  if (value === undefined) lowerPathCache.set(file, (value = normalize(file.path).toLowerCase()));
  return value;
};

export function visibleFiles(draft: Draft): TorrentFile[] {
  if (draft.isHidden) return [];
  if (draft.excludedFileIds.length === 0) return draft.files;
  const excluded = setOf(draft.excludedFileIds);
  return draft.files.filter((file) => !excluded.has(file.id));
}

/** Torrents shown at Home: current provider, not hidden, and either still being read or holding visible files. */
export function activeDrafts(library: Draft[], provider: Provider): Draft[] {
  return library.filter((d) => d.provider === provider && !d.isHidden && (d.files.length === 0 || visibleFiles(d).length > 0));
}

export type QueuedLookup = (draft: Draft, fileId: number) => boolean;

export function queuedLookup(jobs: Job[]): QueuedLookup {
  const map = new Map<string, 'all' | Set<number>>();
  for (const job of jobs) {
    const id = draftId(job);
    if (job.selectedFileIds === null) map.set(id, 'all');
    else if (map.get(id) !== 'all') {
      const set = (map.get(id) as Set<number> | undefined) ?? new Set<number>();
      for (const fileId of job.selectedFileIds) set.add(fileId);
      map.set(id, set);
    }
  }
  return (draft, fileId) => {
    const entry = map.get(draftId(draft));
    return entry === 'all' || (entry?.has(fileId) ?? false);
  };
}

function prefixOf(folder: string): string {
  const trimmed = normalize(folder).replace(/^\/+|\/+$/g, '');
  return trimmed ? trimmed.toLowerCase() + '/' : '';
}

export function filterFiles(files: TorrentFile[], query: string, videosOnly: boolean, folder = ''): TorrentFile[] {
  const prefix = prefixOf(folder);
  const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
  return files.filter((file) => {
    const path = lowerPath(file);
    return path.replace(/^\/+/, '').startsWith(prefix) && (!videosOnly || isVideo(file.path)) && terms.every((term) => path.includes(term));
  });
}

interface Aggregate {
  total: number;
  downloaded: number;
  partial: number;
  bytes: number;
}

/** One pass over a torrent's visible files: counts for every folder prefix. */
function aggregate(draft: Draft, files: TorrentFile[]): Map<string, Aggregate> {
  const downloaded = setOf(draft.downloadedFileIds);
  const partial = setOf(draft.partialFileIds);
  const map = new Map<string, Aggregate>();
  const bump = (key: string, file: TorrentFile) => {
    const entry = map.get(key) ?? { total: 0, downloaded: 0, partial: 0, bytes: 0 };
    entry.total++;
    entry.bytes += file.bytes;
    if (downloaded.has(file.id)) entry.downloaded++;
    else if (partial.has(file.id)) entry.partial++;
    map.set(key, entry);
  };
  for (const file of files) {
    const parts = normalize(file.path).split('/').filter(Boolean);
    let prefix = '';
    bump('', file);
    for (let i = 0; i < parts.length - 1; i++) {
      prefix = prefix ? `${prefix}/${parts[i]}` : parts[i];
      bump(prefix.toLowerCase(), file);
    }
  }
  return map;
}

function displayPath(draft: Draft, file: TorrentFile): string {
  const path = normalize(file.path).replace(/^\/+|\/+$/g, '');
  const head = draft.name + '/';
  return path.toLowerCase().startsWith(head.toLowerCase()) ? path.slice(head.length) : path;
}

export interface BrowserQuery {
  location: Location;
  search: string;
  videosOnly: boolean;
  sort: SortMode;
  header: HeaderSort;
}

export function isStatusDone(entry: Entry) {
  return entry.total > 0 && entry.downloaded === entry.total;
}
export function isStatusPartial(entry: Entry) {
  return !isStatusDone(entry) && (entry.downloaded > 0 || entry.partial > 0);
}

export function buildEntries(library: Draft[], provider: Provider, jobs: Job[], query: BrowserQuery): Entry[] {
  const isQueued = queuedLookup(jobs);
  const drafts = activeDrafts(library, provider);
  const { location } = query;
  const open = location.draft ? drafts.find((d) => draftId(d) === location.draft) ?? null : null;
  const recursive = location.flat || query.videosOnly || query.search.trim() !== '';
  const entries: Entry[] = [];
  const fileEntry = (draft: Draft, file: TorrentFile, downloaded: Set<number>, partial: Set<number>): Entry => {
    const isDone = downloaded.has(file.id);
    return {
      key: `file:${draftId(draft)}:${file.id}`, kind: 'file', draft, file, folderPath: '', name: baseName(file.path), location: `${draft.name} / ${displayPath(draft, file)}`,
      size: file.bytes, fileCount: 1, addedAt: draft.addedAt ? Date.parse(draft.addedAt) : 0, total: 1, downloaded: isDone ? 1 : 0, partial: !isDone && partial.has(file.id) ? 1 : 0,
      queued: isQueued(draft, file.id), pending: false, videoKind: kindOf(file.path)
    };
  };
  const folderEntry = (draft: Draft, path: string, name: string, stats: Aggregate | undefined, pending: boolean): Entry => ({
    key: `folder:${draftId(draft)}:${path.toLowerCase()}`, kind: 'folder', draft, file: null, folderPath: path, name, location: draft.name, size: stats?.bytes ?? 0,
    fileCount: stats?.total ?? 0, addedAt: draft.addedAt ? Date.parse(draft.addedAt) : 0, total: stats?.total ?? 0, downloaded: stats?.downloaded ?? 0, partial: stats?.partial ?? 0,
    queued: false, pending, videoKind: 'folder'
  });

  if (!open && !recursive) {
    // Home: one row per torrent.
    for (const draft of drafts) {
      const files = visibleFiles(draft);
      entries.push(folderEntry(draft, '', draft.name, aggregate(draft, files).get(''), draft.files.length === 0));
    }
  } else {
    const scope = open ? [open] : drafts;
    for (const draft of scope) {
      const files = visibleFiles(draft);
      const downloaded = setOf(draft.downloadedFileIds);
      const partial = setOf(draft.partialFileIds);
      if (!recursive) {
        const base = location.path;
        const prefix = prefixOf(base);
        const stats = aggregate(draft, files);
        const folders = new Map<string, string>();
        for (const file of filterFiles(files, '', false, base)) {
          const rest = normalize(file.path).replace(/^\/+/, '').slice(prefix.length);
          const slash = rest.indexOf('/');
          if (slash < 0) entries.push(fileEntry(draft, file, downloaded, partial));
          else {
            const name = rest.slice(0, slash);
            const path = (base ? normalize(base).replace(/^\/+|\/+$/g, '') + '/' : '') + name;
            folders.set(path.toLowerCase(), name + '\u0000' + path);
          }
        }
        for (const packed of folders.values()) {
          const [name, path] = packed.split('\u0000');
          entries.push(folderEntry(draft, path, name, stats.get(path.toLowerCase()), false));
        }
      } else {
        for (const file of filterFiles(files, query.search, query.videosOnly, open ? location.path : '')) entries.push(fileEntry(draft, file, downloaded, partial));
      }
    }
  }
  return sortEntries(entries, drafts, query);
}

export function sortEntries(entries: Entry[], drafts: Draft[], query: Pick<BrowserQuery, 'sort' | 'header'>): Entry[] {
  const position = new Map(drafts.map((d, index) => [draftId(d), index]));
  const collator = new Intl.Collator(undefined, { sensitivity: 'base', numeric: false });
  return entries.sort((left, right) => {
    const done = Number(isStatusDone(left)) - Number(isStatusDone(right));
    if (done) return done;
    const partial = Number(isStatusPartial(left)) - Number(isStatusPartial(right));
    if (partial) return partial;
    const folders = Number(right.kind === 'folder') - Number(left.kind === 'folder');
    if (folders) return folders;
    if (query.header) {
      const { column, direction } = query.header;
      const value =
        column === 'size' ? (left.kind === 'folder' ? left.total - right.total : left.size - right.size) : column === 'status' ? collator.compare(statusText(left), statusText(right)) : collator.compare(left.name, right.name);
      if (value) return direction === 'desc' ? -value : value;
    } else if (query.sort !== 'name') {
      let date = left.addedAt - right.addedAt;
      if (!date) date = (position.get(draftId(left.draft)) ?? 0) - (position.get(draftId(right.draft)) ?? 0);
      if (date) return query.sort === 'added-desc' ? -date : date;
    }
    return collator.compare(left.name, right.name) || collator.compare(left.location, right.location);
  });
}

export const statusText = (entry: Entry) => (isStatusDone(entry) ? 'done' : isStatusPartial(entry) ? 'partial' : entry.queued ? 'queued' : 'new');

/** Every visible file an entry stands for (a folder expands to its descendants). */
export function filesOf(entry: Entry): TorrentFile[] {
  if (entry.kind === 'file') return entry.file ? [entry.file] : [];
  return filterFiles(visibleFiles(entry.draft), '', false, entry.folderPath);
}

export interface QueueRequest {
  provider: Provider;
  sourceKey: string;
  fileIds: number[];
}

export function queueRequests(entries: Entry[], isQueued: QueuedLookup): QueueRequest[] {
  const groups = new Map<string, QueueRequest>();
  for (const entry of entries) {
    for (const file of filesOf(entry)) {
      if (isQueued(entry.draft, file.id)) continue;
      const id = draftId(entry.draft);
      const group = groups.get(id) ?? { provider: entry.draft.provider, sourceKey: entry.draft.sourceKey, fileIds: [] };
      if (!group.fileIds.includes(file.id)) group.fileIds.push(file.id);
      groups.set(id, group);
    }
  }
  return [...groups.values()];
}

export function remainingRequests(library: Draft[], provider: Provider, jobs: Job[]): QueueRequest[] {
  const isQueued = queuedLookup(jobs);
  const out: QueueRequest[] = [];
  for (const draft of activeDrafts(library, provider)) {
    const ids = visibleFiles(draft).filter((file) => !isQueued(draft, file.id)).map((file) => file.id);
    if (ids.length) out.push({ provider: draft.provider, sourceKey: draft.sourceKey, fileIds: ids });
  }
  return out;
}

export const remainingCount = (library: Draft[], provider: Provider, jobs: Job[]) => remainingRequests(library, provider, jobs).reduce((sum, r) => sum + r.fileIds.length, 0);

/** Patches that hide the given entries from the library, plus the patches that restore the previous state. */
export function removalPatches(entries: Entry[]): { apply: LibraryPatch[]; undo: LibraryPatch[] } {
  const byDraft = new Map<string, { draft: Draft; excluded: Set<number>; hidden: boolean }>();
  for (const entry of entries) {
    const id = draftId(entry.draft);
    const state = byDraft.get(id) ?? { draft: entry.draft, excluded: new Set(entry.draft.excludedFileIds), hidden: entry.draft.isHidden };
    if (entry.kind === 'folder' && entry.folderPath === '') state.hidden = true;
    else for (const file of filesOf(entry)) state.excluded.add(file.id);
    byDraft.set(id, state);
  }
  const apply: LibraryPatch[] = [];
  const undo: LibraryPatch[] = [];
  for (const { draft, excluded, hidden } of byDraft.values()) {
    apply.push({ provider: draft.provider, sourceKey: draft.sourceKey, excludedFileIds: [...excluded], isHidden: hidden });
    undo.push({ provider: draft.provider, sourceKey: draft.sourceKey, excludedFileIds: [...draft.excludedFileIds], isHidden: draft.isHidden });
  }
  return { apply, undo };
}

export interface Crumb {
  label: string | null;
  location: Location;
}

export function breadcrumbs(location: Location, drafts: Draft[]): Crumb[] {
  const crumbs: Crumb[] = [{ label: null, location: { draft: null, path: '', flat: false } }];
  const open = location.draft ? drafts.find((d) => draftId(d) === location.draft) : undefined;
  if (!open) return crumbs;
  crumbs.push({ label: open.name, location: { draft: location.draft, path: '', flat: false } });
  let path = '';
  for (const part of normalize(location.path).split('/').filter(Boolean)) {
    path = path ? `${path}/${part}` : part;
    crumbs.push({ label: part, location: { draft: location.draft, path, flat: false } });
  }
  return crumbs;
}

export function formatBytes(bytes: number): string {
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  return `${Number(value.toFixed(2))} ${units[unit]}`;
}
