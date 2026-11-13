// Pure logic for the Library browser: which rows to show for a location, their completion marks, sorting, and the file
// sets behind queue/remove actions. Kept free of UI so it can be unit-tested and stays fast on 30,000-file libraries.

import { providerLabel, type Draft, type Job, type LibraryPatch, type Provider, type TorrentFile } from './types';

export type SortMode = 'name' | 'added-desc' | 'added-asc';
export type HeaderSort = { column: 'name' | 'source' | 'size' | 'added' | 'status'; direction: 'asc' | 'desc' } | null;
/** Which service's torrents the Library shows. */
export type SourceFilter = Provider | 'all';

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

/** Torrents shown at Home: from the chosen service(s), not hidden, and either still being read or holding visible files. */
export function activeDrafts(library: Draft[], source: SourceFilter = 'all'): Draft[] {
  return library.filter((d) => (source === 'all' || d.provider === source) && !d.isHidden && (d.files.length === 0 || visibleFiles(d).length > 0));
}

const identityCache = new WeakMap<TorrentFile[], Map<TorrentFile, string>>();

/**
 * The same file of the same torrent on another service. File IDs differ per service, and so may the folder a path
 * starts with, so the file name and size identify it; names that repeat inside the torrent (`S01/01.mkv`,
 * `S02/01.mkv`) also take their parent folder.
 */
export function fileIdentity(draft: Draft, file: TorrentFile): string {
  let names = identityCache.get(draft.files);
  if (!names) {
    const plain = (f: TorrentFile) => `${baseName(f.path).toLowerCase()}|${f.bytes}`;
    const counts = new Map<string, number>();
    for (const f of draft.files) counts.set(plain(f), (counts.get(plain(f)) ?? 0) + 1);
    names = new Map();
    for (const f of draft.files) {
      const parts = normalize(f.path).split('/').filter(Boolean);
      const parent = parts.length > 1 ? parts[parts.length - 2].toLowerCase() : '';
      names.set(f, (counts.get(plain(f)) ?? 0) > 1 ? `${plain(f)}|${parent}` : plain(f));
    }
    identityCache.set(draft.files, names);
  }
  const name = names.get(file) ?? `${baseName(file.path).toLowerCase()}|${file.bytes}`;
  return `${draft.sourceKey.toLowerCase()}|${name}`;
}

/** Source keys that the Library holds on more than one service. */
function sharedSources(library: Draft[]): Set<string> {
  const seen = new Map<string, Provider>();
  const shared = new Set<string>();
  for (const draft of library) {
    const key = draft.sourceKey.toLowerCase();
    const first = seen.get(key);
    if (first === undefined) seen.set(key, draft.provider);
    else if (first !== draft.provider) shared.add(key);
  }
  return shared;
}

/** `file` lets a lookup see the same file queued from another service. */
export type QueuedLookup = (draft: Draft, fileId: number, file?: TorrentFile) => boolean;

export function queuedLookup(jobs: Job[], library: Draft[] = []): QueuedLookup {
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
  // Only torrents held on several services need the slower match by name and size.
  const shared = sharedSources(library);
  const identities = new Set<string>();
  if (shared.size) {
    const byId = new Map(library.map((d) => [draftId(d), d]));
    for (const job of jobs) {
      const draft = shared.has(job.sourceKey.toLowerCase()) ? byId.get(draftId(job)) : undefined;
      if (!draft) continue;
      const wanted = job.selectedFileIds === null ? null : new Set(job.selectedFileIds);
      for (const file of draft.files) if (!wanted || wanted.has(file.id)) identities.add(fileIdentity(draft, file));
    }
  }
  return (draft, fileId, file) => {
    const entry = map.get(draftId(draft));
    if (entry === 'all' || entry?.has(fileId)) return true;
    return file !== undefined && identities.size > 0 && shared.has(draft.sourceKey.toLowerCase()) && identities.has(fileIdentity(draft, file));
  };
}

/** Higher means a better service to download a torrent from (for example: already complete in its cloud). */
export type Rank = (draft: Draft) => number;

/**
 * When the same file is offered by several services, keep only the best-ranked one (the first on a tie), so a
 * torrent added to every service is still downloaded once.
 */
function oneServicePerFile(candidates: { draft: Draft; file: TorrentFile }[], rank?: Rank): (draft: Draft, file: TorrentFile) => boolean {
  const best = new Map<string, { provider: Provider; score: number }>();
  for (const { draft, file } of candidates) {
    const id = fileIdentity(draft, file);
    const score = rank?.(draft) ?? 0;
    const current = best.get(id);
    if (!current || score > current.score) best.set(id, { provider: draft.provider, score });
  }
  return (draft, file) => best.get(fileIdentity(draft, file))?.provider === draft.provider;
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

export function buildEntries(library: Draft[], source: SourceFilter, jobs: Job[], query: BrowserQuery): Entry[] {
  const isQueued = queuedLookup(jobs, library);
  const drafts = activeDrafts(library, source);
  const { location } = query;
  const open = location.draft ? drafts.find((d) => draftId(d) === location.draft) ?? null : null;
  const recursive = location.flat || query.videosOnly || query.search.trim() !== '';
  const entries: Entry[] = [];
  const fileEntry = (draft: Draft, file: TorrentFile, downloaded: Set<number>, partial: Set<number>): Entry => {
    const isDone = downloaded.has(file.id);
    return {
      key: `file:${draftId(draft)}:${file.id}`, kind: 'file', draft, file, folderPath: '', name: baseName(file.path), location: `${draft.name} / ${displayPath(draft, file)}`,
      size: file.bytes, fileCount: 1, addedAt: draft.addedAt ? Date.parse(draft.addedAt) : 0, total: 1, downloaded: isDone ? 1 : 0, partial: !isDone && partial.has(file.id) ? 1 : 0,
      queued: isQueued(draft, file.id, file), pending: false, videoKind: kindOf(file.path)
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
  const collator = new Intl.Collator(undefined, { sensitivity: 'base', numeric: true });
  // Insertion order breaks ties between torrents added in the same instant (or without a date): later entries are newer.
  const added = (left: Entry, right: Entry) => left.addedAt - right.addedAt || (position.get(draftId(left.draft)) ?? 0) - (position.get(draftId(right.draft)) ?? 0);
  // The chosen order is the primary key: nothing (not even download status) may reorder rows ahead of it. Folders stay
  // above files, as in a file explorer.
  return entries.sort((left, right) => {
    const folders = Number(right.kind === 'folder') - Number(left.kind === 'folder');
    if (folders) return folders;
    if (query.header) {
      const { column, direction } = query.header;
      const value =
        column === 'size' ? (left.kind === 'folder' ? left.total - right.total : left.size - right.size)
        : column === 'added' ? added(left, right)
        : column === 'status' ? collator.compare(statusText(left), statusText(right))
        : column === 'source' ? collator.compare(providerLabel(left.draft.provider), providerLabel(right.draft.provider))
        : collator.compare(left.name, right.name);
      if (value) return direction === 'desc' ? -value : value;
    } else if (query.sort !== 'name') {
      const date = added(left, right);
      if (date) return query.sort === 'added-desc' ? -date : date;
    }
    return collator.compare(left.name, right.name) || collator.compare(left.location, right.location);
  });
}

/** A magnet link for a torrent: the original one when it was added as a magnet (keeps its trackers), else rebuilt from its hash. */
export function magnetOf(draft: Draft): string | null {
  if (/^magnet:/i.test(draft.source)) return draft.source;
  const hash = /^btih:([0-9a-z]+)$/i.exec(draft.sourceKey)?.[1];
  return hash ? `magnet:?xt=urn:btih:${hash}&dn=${encodeURIComponent(draft.name)}` : null;
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

/** Count already queued files once, even when selected through overlapping folders or several services. */
export function queuedFileCount(entries: Entry[], isQueued: QueuedLookup): number {
  const duplicates = new Set<string>();
  for (const entry of entries) for (const file of filesOf(entry)) {
    if (isQueued(entry.draft, file.id, file)) duplicates.add(fileIdentity(entry.draft, file));
  }
  return duplicates.size;
}

export function queueRequests(entries: Entry[], isQueued: QueuedLookup, rank?: Rank): QueueRequest[] {
  const candidates: { draft: Draft; file: TorrentFile }[] = [];
  for (const entry of entries) for (const file of filesOf(entry)) if (!isQueued(entry.draft, file.id, file)) candidates.push({ draft: entry.draft, file });
  const chosen = oneServicePerFile(candidates, rank);
  const groups = new Map<string, QueueRequest>();
  for (const entry of entries) {
    for (const file of filesOf(entry)) {
      if (isQueued(entry.draft, file.id, file) || !chosen(entry.draft, file)) continue;
      const id = draftId(entry.draft);
      const group = groups.get(id) ?? { provider: entry.draft.provider, sourceKey: entry.draft.sourceKey, fileIds: [] };
      if (!group.fileIds.includes(file.id)) group.fileIds.push(file.id);
      groups.set(id, group);
    }
  }
  return [...groups.values()];
}

export function remainingRequests(library: Draft[], source: SourceFilter, jobs: Job[], rank?: Rank): QueueRequest[] {
  const isQueued = queuedLookup(jobs, library);
  const drafts = activeDrafts(library, source);
  const candidates = drafts.flatMap((draft) => visibleFiles(draft).filter((file) => !isQueued(draft, file.id, file)).map((file) => ({ draft, file })));
  const chosen = oneServicePerFile(candidates, rank);
  const out: QueueRequest[] = [];
  for (const draft of drafts) {
    const ids = visibleFiles(draft).filter((file) => !isQueued(draft, file.id, file) && chosen(draft, file)).map((file) => file.id);
    if (ids.length) out.push({ provider: draft.provider, sourceKey: draft.sourceKey, fileIds: ids });
  }
  return out;
}

export const remainingCount = (library: Draft[], source: SourceFilter, jobs: Job[]) => remainingRequests(library, source, jobs).reduce((sum, r) => sum + r.fileIds.length, 0);

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

/**
 * Everything the Library holds that is already saved on this computer, ready for `removalPatches`: the finished files
 * of each torrent, or the whole torrent when every one of its files is finished. Works per file, independent of the
 * folder being browsed, so a half-downloaded torrent still gets its finished files removed.
 */
export function downloadedRemoval(library: Draft[], source: SourceFilter): { entries: Entry[]; files: number } {
  const entries: Entry[] = [];
  let files = 0;
  for (const draft of activeDrafts(library, source)) {
    const visible = visibleFiles(draft);
    const downloaded = setOf(draft.downloadedFileIds);
    const finished = visible.filter((file) => downloaded.has(file.id));
    if (finished.length === 0) continue;
    files += finished.length;
    if (finished.length === visible.length) entries.push({ kind: 'folder', draft, folderPath: '' } as Entry);
    else for (const file of finished) entries.push({ kind: 'file', draft, file } as Entry);
  }
  return { entries, files };
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
