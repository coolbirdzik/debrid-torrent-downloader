import { getVersion } from '@tauri-apps/api/app';
import { api, on, toMsg } from './api';
import { setLanguage } from './i18n.svelte';
import type { Draft, Job, Marks, Msg, Provider, Settings, Snapshot, UpdateInfo, User } from './types';
import { canRetry, canResume } from './types';

export type Page = 'library' | 'queue' | 'remote' | 'settings';

const defaults: Settings = {
  provider: 'realDebrid', language: 'en', appearance: 'system', browserView: 'list', browserSort: 'name',
  outputDirectory: '', flattenFiles: true, concurrency: 3, rememberToken: true, libraryImportPaused: false, checkUpdates: true
};

export interface Toast {
  id: number;
  tone: 'info' | 'error' | 'success';
  message: Msg;
}

class AppState {
  ready = $state(false);
  page = $state<Page>('library');
  settings = $state<Settings>({ ...defaults });
  // Large and replaced wholesale on every update, so keep them out of Svelte's deep proxies.
  library = $state.raw<Draft[]>([]);
  jobs = $state.raw<Job[]>([]);
  hasToken = $state(false);
  user = $state<User | null>(null);
  warning = $state<Msg | null>(null);
  dataFile = $state('');
  status = $state<Msg>({ key: 'Sẵn sàng', args: [] });
  inspect = $state({ running: false, done: 0, total: 0 });
  toasts = $state<Toast[]>([]);
  connecting = $state(false);
  version = $state('');
  update = $state<UpdateInfo | null>(null);
  checkingUpdate = $state(false);

  jobsHere = $derived(this.jobs.filter((job) => job.provider === this.settings.provider));
  activeCount = $derived(this.jobsHere.filter((job) => job.state !== 'completed').length);
  completedCount = $derived(this.jobsHere.filter((job) => job.state === 'completed').length);
  retryable = $derived(this.jobsHere.filter(canRetry).length);
  anyActive = $derived(this.jobsHere.some((job) => job.isActive));
  anyResumable = $derived(this.jobsHere.some(canResume));

  private toastId = 0;

  async init() {
    const snapshot = await api.bootstrap();
    this.apply(snapshot);
    this.ready = true;
    getVersion().then((version) => (this.version = version), () => {});
    if (this.settings.checkUpdates) void this.checkUpdate(true);
    await Promise.all([
      on('jobs', (changed) => {
        const map = new Map(this.jobs.map((job) => [job.id, job]));
        for (const job of changed) map.set(job.id, job);
        this.jobs = [...map.values()];
      }),
      on('jobs-removed', (ids) => {
        const gone = new Set(ids);
        this.jobs = this.jobs.filter((job) => !gone.has(job.id));
      }),
      on('draft', (draft) => this.upsertDraft(draft)),
      on('marks', (marks) => this.applyMarks(marks)),
      on('inspect', (state) => (this.inspect = state)),
      on('status', (message) => (this.status = message)),
      on('account', (user) => { this.user = user; this.hasToken = true; })
    ]);
  }

  apply(snapshot: Snapshot) {
    this.settings = snapshot.settings;
    this.library = snapshot.library;
    this.jobs = snapshot.jobs;
    this.hasToken = snapshot.account.hasToken;
    this.user = snapshot.account.user;
    this.warning = snapshot.warning;
    this.dataFile = snapshot.dataFile;
    setLanguage(snapshot.settings.language);
    applyAppearance(snapshot.settings.appearance);
  }

  private upsertDraft(draft: Draft) {
    const same = (d: Draft) => d.provider === draft.provider && d.sourceKey.toLowerCase() === draft.sourceKey.toLowerCase();
    const index = this.library.findIndex(same);
    if (index < 0) this.library = [...this.library, draft];
    else {
      const next = this.library.slice();
      next[index] = draft;
      this.library = next;
    }
  }

  private applyMarks(marks: Marks[]) {
    const next = this.library.slice();
    for (const mark of marks) {
      const index = next.findIndex((d) => d.provider === mark.provider && d.sourceKey.toLowerCase() === mark.sourceKey.toLowerCase());
      if (index >= 0) next[index] = { ...next[index], downloadedFileIds: mark.downloadedFileIds, partialFileIds: mark.partialFileIds };
    }
    this.library = next;
  }

  toast(message: Msg, tone: Toast['tone'] = 'info') {
    const id = ++this.toastId;
    this.toasts = [...this.toasts, { id, tone, message }];
    setTimeout(() => (this.toasts = this.toasts.filter((toast) => toast.id !== id)), tone === 'error' ? 8000 : 4000);
  }

  /** Run a backend call and surface a failure as an error toast instead of an unhandled rejection. */
  async run<T>(work: () => Promise<T>): Promise<T | undefined> {
    try {
      return await work();
    } catch (error) {
      const message = toMsg(error);
      this.status = message;
      this.toast(message, 'error');
      return undefined;
    }
  }

  async setSettings(patch: Partial<Settings>) {
    this.settings = { ...this.settings, ...patch };
    if (patch.language) setLanguage(patch.language);
    if (patch.appearance) applyAppearance(patch.appearance);
    const saved = await this.run(() => api.updateSettings(patch));
    if (saved) this.settings = saved;
    if (patch.provider) {
      this.user = null;
      const snapshot = await api.bootstrap();
      this.hasToken = snapshot.account.hasToken;
    }
  }

  async connect() {
    this.connecting = true;
    const user = await this.run(() => api.connect());
    this.connecting = false;
    if (user) {
      this.user = user;
      this.toast({ key: 'Đã kết nối: {0}', args: [`${user.username} · ${user.kind}`] }, 'success');
    }
  }

  async saveToken(token: string) {
    const warning = await this.run(() => api.saveToken(token));
    this.hasToken = token.trim().length > 0;
    this.user = null;
    if (warning) this.toast(warning, 'error');
  }

  /** `quiet` is the startup check: it never reports a failure or an up-to-date result. */
  async checkUpdate(quiet = false) {
    if (this.checkingUpdate) return;
    this.checkingUpdate = true;
    try {
      const info = await api.checkUpdate();
      this.update = info;
      if (!quiet && !info.available) {
        this.toast(info.latest ? { key: 'Bạn đang dùng bản mới nhất.', args: [] } : { key: 'Chưa có bản phát hành nào trên GitHub.', args: [] }, 'success');
      }
    } catch (error) {
      if (!quiet) this.toast(toMsg(error), 'error');
    } finally {
      this.checkingUpdate = false;
    }
  }

  openUpdate() {
    const url = this.update?.downloadUrl ?? this.update?.pageUrl;
    if (url) return this.run(() => api.openRelease(url));
  }

  switchProvider(provider: Provider) {
    return this.setSettings({ provider });
  }
}

export const app = new AppState();

export function applyAppearance(appearance: Settings['appearance']) {
  if (typeof document === 'undefined') return;
  if (appearance === 'system') document.documentElement.removeAttribute('data-theme');
  else document.documentElement.setAttribute('data-theme', appearance);
}
