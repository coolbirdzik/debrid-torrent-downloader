// Mirrors the Rust models (camelCase over IPC), except `User`, which keeps snake_case from the core crate.

import type { Language } from './languages';

export type Provider = 'realDebrid' | 'torBox' | 'allDebrid';
export const PROVIDERS: { id: Provider; label: string }[] = [
  { id: 'realDebrid', label: 'Real-Debrid' },
  { id: 'torBox', label: 'TorBox' },
  { id: 'allDebrid', label: 'AllDebrid' }
];
export const providerLabel = (provider: Provider) => PROVIDERS.find((p) => p.id === provider)?.label ?? provider;
/** Two-letter tag for tight spots (grid tiles, queue rows). */
export const providerShort = (provider: Provider) => ({ realDebrid: 'RD', torBox: 'TB', allDebrid: 'AD' })[provider] ?? provider;

/** A message the UI translates: `key` is the Vietnamese source text. */
export interface Msg {
  key: string;
  args: string[];
}

export type JobState = 'queued' | 'adding' | 'waiting' | 'downloading' | 'paused' | 'completed' | 'failed' | 'stopped';

export interface JobFile {
  torrentFileId: number | null;
  linkIndex: number;
  link: string;
  relativePath: string;
  expectedBytes: number;
  downloadedBytes: number;
  completed: boolean;
}

export interface Job {
  id: string;
  provider: Provider;
  source: string;
  sourceKey: string;
  torrentId: string | null;
  remoteAccount: string | null;
  selectedFileIds: number[] | null;
  outputDirectory: string | null;
  /** Folder chosen when the job was queued; `null` means the Settings folder. */
  targetDirectory: string | null;
  flattenFiles: boolean | null;
  files: JobFile[];
  name: string;
  state: JobState;
  detail: Msg;
  remoteProgress: number;
  localProgress: number;
  totalBytes: number;
  isActive: boolean;
  isStopping: boolean;
  isRemoving: boolean;
}

export interface TorrentFile {
  id: number;
  path: string;
  bytes: number;
  selected: number;
}

export interface Draft {
  addedAt: string | null;
  provider: Provider;
  source: string;
  sourceKey: string;
  name: string;
  torrentId: string | null;
  remoteAccount: string | null;
  files: TorrentFile[];
  excludedFileIds: number[];
  downloadedFileIds: number[];
  partialFileIds: number[];
  isHidden: boolean;
  inspectionAttempts: number;
  nextInspectionAt: string | null;
  inspectionError: Msg | null;
  cloud: CloudState | null;
}

/** How far a service has fetched a torrent into its own storage. */
export interface CloudState {
  status: string;
  progress: number;
  speed: number | null;
  seeders: number | null;
}

export interface CloudUpdate {
  provider: Provider;
  sourceKey: string;
  cloud: CloudState | null;
}

export interface Settings {
  provider: Provider;
  language: Language;
  appearance: 'system' | 'light' | 'dark';
  browserView: 'list' | 'grid';
  browserSort: 'name' | 'added-desc' | 'added-asc';
  outputDirectory: string;
  flattenFiles: boolean;
  concurrency: number;
  independentCloudDownloads: boolean;
  rememberToken: boolean;
  libraryImportPaused: boolean;
  checkUpdates: boolean;
  /** Services the last Library import used. */
  libraryProviders: Provider[];
  /** Reuse `libraryProviders` without asking. */
  rememberLibraryProviders: boolean;
}

export interface UpdateInfo {
  current: string;
  /** `null` when GitHub has no published release yet. */
  latest: string | null;
  available: boolean;
  canInstall: boolean;
  pageUrl: string | null;
  downloadUrl: string | null;
  notes: string;
}

export interface UpdateProgress {
  phase: 'downloading' | 'installing';
  downloaded: number;
  total: number | null;
}

export interface User {
  username: string;
  kind: string;
  premium: number;
  allows_downloads: boolean;
  email?: string | null;
  expiration?: string | null;
  points?: number | null;
}

export interface AccountInfo {
  provider: Provider;
  hasToken: boolean;
  user: User | null;
}

export type CloudKind = 'torrent' | 'webDownload' | 'usenet';
export type CloudFilter = CloudKind | 'all';
export const CLOUD_KINDS: CloudKind[] = ['torrent', 'webDownload', 'usenet'];
export const cloudKindLabel = (kind: CloudKind) => ({ torrent: 'Torrent', webDownload: 'Web Downloads', usenet: 'Usenet' })[kind];

export interface TorBoxAccount {
  account: string;
  email: string | null;
  plan: string;
  expiresAt: string | null;
  airlockUsedBytes: number | null;
  airlockLimitBytes: number | null;
  airlockRemainingBytes: number | null;
  bandwidth30dBytes: number | null;
  downloadedBytes: number | null;
  uploadedBytes: number | null;
  warning: Msg | null;
}

export interface Snapshot {
  settings: Settings;
  library: Draft[];
  jobs: Job[];
  account: AccountInfo;
  accounts: AccountInfo[];
  inspect?: { running: boolean; done: number; total: number };
  warning: Msg | null;
  dataFile: string;
}

export interface Marks {
  provider: Provider;
  sourceKey: string;
  downloadedFileIds: number[];
  partialFileIds: number[];
}

export interface RemoteTorrent {
  id: string;
  filename: string;
  bytes: number;
  progress: number;
  status: string;
  added: string | null;
  speed: number | null;
  seeders: number | null;
}

export interface AddResult {
  added: number;
  duplicates: number;
  errors: Msg[];
}

export interface DeleteOutcome {
  id: string;
  ok: boolean;
  message: Msg | null;
}

export interface LibraryPatch {
  provider: Provider;
  sourceKey: string;
  excludedFileIds: number[];
  isHidden: boolean;
}

export const canRetry = (job: Job) => !job.isRemoving && !job.isActive && !job.isStopping && job.state === 'failed';
export const canResume = (job: Job) => !job.isRemoving && !job.isActive && !job.isStopping && ['queued', 'paused', 'stopped'].includes(job.state);
export const canStop = (job: Job) => !job.isRemoving && !job.isStopping && (job.isActive || ['queued', 'paused'].includes(job.state));
export const canRemove = (job: Job) => !job.isRemoving && !job.isStopping;
