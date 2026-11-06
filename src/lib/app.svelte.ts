import { getVersion } from '@tauri-apps/api/app';
import { api, on, toMsg } from './api';
import { setLanguage } from './i18n.svelte';
import { activeDrafts, fileIdentity, visibleFiles } from './browser';
import type { AccountInfo, CloudState, Draft, Job, Marks, Msg, Provider, Settings, Snapshot, TorBoxAccount, UpdateInfo, User } from './types';
import { canRetry, canResume, PROVIDERS } from './types';

export type Page = 'library' | 'queue' | 'remote' | 'settings';

const defaults: Settings = {
  provider: 'realDebrid', language: 'en', appearance: 'system', browserView: 'list', browserSort: 'added-desc',
  outputDirectory: '', flattenFiles: true, concurrency: 3, independentCloudDownloads: true, rememberToken: true, libraryImportPaused: false, checkUpdates: true,
  libraryProviders: [], rememberLibraryProviders: false
};

const cloudKey = (draft: { provider: Provider; sourceKey: string }) => `${draft.provider}|${draft.sourceKey.toLowerCase()}`;

export interface Toast {
  id: number;
  tone: 'info' | 'error' | 'success' | 'warning';
  message: Msg;
}

class AppState {
  ready = $state(false);
  page = $state<Page>('library');
  settings = $state<Settings>({ ...defaults });
  // Large and replaced wholesale on every update, so keep them out of Svelte's deep proxies.
  library = $state.raw<Draft[]>([]);
  private seenLibraryFiles = $state.raw<Set<string>>(new Set());
  private clearedLibraryDrafts = new Map<string, string | null>();
  /** Navigation counts always cover every service, independent of the Library's current filters. */
  libraryBadges = $derived.by(() => {
    let pending = 0;
    const unread = new Set<string>();
    for (const draft of activeDrafts(this.library)) {
      if (draft.files.length === 0) pending++;
      else if (this.page !== 'library') {
        for (const file of visibleFiles(draft)) {
          const key = fileIdentity(draft, file);
          if (!this.seenLibraryFiles.has(key)) unread.add(key);
        }
      }
    }
    return { pending, newFiles: unread.size };
  });
  jobs = $state.raw<Job[]>([]);
  /** Cloud progress per Library torrent (key: `provider|sourcekey`), kept apart so it never rebuilds the rows. */
  cloud = $state.raw<Map<string, CloudState>>(new Map());
  /** Every service; `hasToken`/`user` below mirror the one chosen in Settings. */
  accounts = $state<AccountInfo[]>(PROVIDERS.map((p) => ({ provider: p.id, hasToken: false, user: null })));
  accountRefreshing = $state<Partial<Record<Provider, boolean>>>({});
  accountErrors = $state<Partial<Record<Provider, Msg | null>>>({});
  private accountRefreshGeneration = new Map<Provider, number>();
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
  updating = $state(false);
  updateProgress = $state<import('./types').UpdateProgress | null>(null);
  updateError = $state<Msg | null>(null);
  torboxAccount = $state.raw<TorBoxAccount | null>(null);
  torboxAccountLoading = $state(false);
  torboxAccountError = $state<Msg | null>(null);
  private torboxAccountGeneration = 0;
  private torboxAccountUpdated = 0;

  /** Services with a token: the ones the Library and the queue can use. */
  usableProviders = $derived(this.accounts.filter((a) => a.hasToken).map((a) => a.provider));
  activeCount = $derived(this.jobs.filter((job) => job.state !== 'completed').length);
  completedCount = $derived(this.jobs.filter((job) => job.state === 'completed').length);
  retryable = $derived(this.jobs.filter(canRetry).length);
  anyActive = $derived(this.jobs.some((job) => job.isActive));
  anyResumable = $derived(this.jobs.some(canResume));

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
      on('account', (account) => this.setAccount(account)),
      on('cloud', (updates) => {
        const next = new Map(this.cloud);
        for (const update of updates) {
          if (update.cloud) next.set(cloudKey(update), update.cloud);
          else next.delete(cloudKey(update));
        }
        this.cloud = next;
      })
    ]);
  }

  apply(snapshot: Snapshot) {
    for (const { id } of PROVIDERS) this.accountRefreshGeneration.set(id, (this.accountRefreshGeneration.get(id) ?? 0) + 1);
    this.accountRefreshing = {};
    this.accountErrors = {};
    this.settings = snapshot.settings;
    this.library = snapshot.library;
    this.inspect = snapshot.inspect ?? { running: false, done: 0, total: 0 };
    this.clearedLibraryDrafts.clear();
    // Existing files are the startup baseline; later metadata arrivals can be new while another page is open.
    this.seenLibraryFiles = new Set();
    this.markLibrarySeen();
    this.jobs = snapshot.jobs;
    this.cloud = new Map(snapshot.library.filter((d) => d.cloud).map((d) => [cloudKey(d), d.cloud!]));
    this.accounts = snapshot.accounts;
    this.clearTorboxAccount();
    this.hasToken = snapshot.account.hasToken;
    this.user = snapshot.account.user;
    this.warning = snapshot.warning;
    this.dataFile = snapshot.dataFile;
    setLanguage(snapshot.settings.language);
    applyAppearance(snapshot.settings.appearance);
  }

  private setAccount(account: AccountInfo) {
    if (!account.hasToken || !account.user) {
      this.accountRefreshGeneration.set(account.provider, (this.accountRefreshGeneration.get(account.provider) ?? 0) + 1);
      this.accountRefreshing[account.provider] = false;
      this.accountErrors[account.provider] = null;
    }
    if (account.provider === 'torBox') {
      const previous = this.accounts.find((a) => a.provider === 'torBox');
      if (!account.hasToken || (previous?.user && previous.user.username.toLowerCase() !== account.user?.username.toLowerCase())) this.clearTorboxAccount();
    }
    this.accounts = this.accounts.map((a) => (a.provider === account.provider ? account : a));
    if (account.provider === this.settings.provider) {
      this.user = account.user;
      this.hasToken = account.hasToken;
    }
  }

  /** Refresh only this provider; never changes the service selected in Settings. */
  async refreshAccount(provider: Provider) {
    if (!this.accounts.some((account) => account.provider === provider && account.hasToken) || this.accountRefreshing[provider]) return;
    const generation = (this.accountRefreshGeneration.get(provider) ?? 0) + 1;
    this.accountRefreshGeneration.set(provider, generation);
    this.accountRefreshing[provider] = true;
    this.accountErrors[provider] = null;
    try {
      const user = await api.connect(provider);
      if (generation !== this.accountRefreshGeneration.get(provider)) return;
      this.setAccount({ provider, hasToken: true, user });
      return user;
    } catch (error) {
      if (generation === this.accountRefreshGeneration.get(provider)) this.accountErrors[provider] = toMsg(error);
    } finally {
      if (generation === this.accountRefreshGeneration.get(provider)) this.accountRefreshing[provider] = false;
    }
  }

  private clearTorboxAccount() {
    this.torboxAccountGeneration++;
    this.torboxAccount = null;
    this.torboxAccountError = null;
    this.torboxAccountLoading = false;
    this.torboxAccountUpdated = 0;
  }

  async refreshTorboxAccount(force = false) {
    if (!this.accounts.some((a) => a.provider === 'torBox' && a.hasToken) || this.torboxAccountLoading) return;
    if (!force && this.torboxAccount && Date.now() - this.torboxAccountUpdated < 60_000) return;
    const generation = ++this.torboxAccountGeneration;
    this.torboxAccountLoading = true;
    this.torboxAccountError = null;
    try {
      const details = await api.torboxAccount();
      if (generation !== this.torboxAccountGeneration) return;
      this.torboxAccount = details;
      this.torboxAccountUpdated = Date.now();
    } catch (error) {
      if (generation === this.torboxAccountGeneration) this.torboxAccountError = toMsg(error);
    } finally {
      if (generation === this.torboxAccountGeneration) this.torboxAccountLoading = false;
    }
  }

  cloudOf(draft: Draft): CloudState | undefined {
    return this.cloud.get(cloudKey(draft));
  }

  /** Opening the Library acknowledges all currently visible files, including copies on another service. */
  markLibrarySeen() {
    const seen = new Set(this.seenLibraryFiles);
    for (const draft of activeDrafts(this.library)) {
      for (const file of visibleFiles(draft)) seen.add(fileIdentity(draft, file));
    }
    if (seen.size !== this.seenLibraryFiles.size) this.seenLibraryFiles = seen;
  }

  resetLibraryActivity() {
    this.seenLibraryFiles = new Set();
  }

  clearLibraryView() {
    for (const draft of this.library) this.clearedLibraryDrafts.set(cloudKey(draft), draft.addedAt);
    this.library = [];
    this.inspect = { running: false, done: 0, total: 0 };
    this.cloud = new Map();
    this.resetLibraryActivity();
  }

  private upsertDraft(draft: Draft) {
    const key = cloudKey(draft);
    // An inspection that finished during reset must not resurrect a removed row. A deliberate re-import has a new date.
    if (this.clearedLibraryDrafts.has(key)) {
      if (this.clearedLibraryDrafts.get(key) === draft.addedAt) return;
      this.clearedLibraryDrafts.delete(key);
    }
    if ((draft.cloud ?? undefined) !== this.cloud.get(key)) {
      const next = new Map(this.cloud);
      if (draft.cloud) next.set(key, draft.cloud);
      else next.delete(key);
      this.cloud = next;
    }
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
      const account = this.accounts.find((a) => a.provider === patch.provider);
      this.user = account?.user ?? null;
      this.hasToken = account?.hasToken ?? false;
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
    const provider = this.settings.provider;
    const result = await this.run(async () => ({ warning: await api.saveToken(token) }));
    if (!result) return;
    if (provider === 'torBox') this.clearTorboxAccount();
    this.setAccount({ provider, hasToken: token.trim().length > 0, user: null });
    if (result.warning) this.toast(result.warning, 'error');
  }

  /** `quiet` is the startup check: it never reports a failure or an up-to-date result. */
  async checkUpdate(quiet = false) {
    if (this.checkingUpdate || this.updating) return;
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

  async installUpdate() {
    if (this.updating || this.checkingUpdate || !this.update?.available || !this.update.canInstall || !this.update.latest) return;
    this.updating = true;
    this.updateError = null;
    this.updateProgress = { phase: 'downloading', downloaded: 0, total: null };
    let unlisten: (() => void) | undefined;
    try {
      // Subscribe before invoking the backend, including on a fast/cached download.
      unlisten = await on('update-progress', (progress) => (this.updateProgress = progress));
      await api.installUpdate(this.update.latest);
    } catch (error) {
      this.updateError = toMsg(error);
      this.toast(this.updateError, 'error');
    } finally {
      unlisten?.();
      this.updating = false;
      this.updateProgress = null;
    }
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
