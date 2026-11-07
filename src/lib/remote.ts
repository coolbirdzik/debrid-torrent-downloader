import { CLOUD_KINDS, type AccountInfo, type CloudFilter, type CloudKind, type DeleteOutcome, type Job, type Msg, type Provider, type RemoteTorrent } from './types';
import { emptySelection, reconcileSelection, selectAll, selectItem, selectionKey, type Selection } from './selection';

export interface RemoteRow extends RemoteTorrent {
  provider: Provider;
  account: string;
  kind?: CloudKind;
}

export type RemoteSortColumn = 'name' | 'size' | 'status' | 'added';
export type RemoteSort = { column: RemoteSortColumn; direction: 'asc' | 'desc' } | null;

/** Match Library headers: ascending, descending, then back to the default newest-first list. */
export const nextRemoteSort = (current: RemoteSort, column: RemoteSortColumn): RemoteSort =>
  current?.column !== column ? { column, direction: 'asc' }
    : current.direction === 'asc' ? { column, direction: 'desc' } : null;

/** Sort a display copy, keeping service/account/type identities and the backing list intact for deletion. */
export function sortRemoteRows(rows: RemoteRow[], sort: RemoteSort, locale?: string): RemoteRow[] {
  if (!sort) return rows;
  const collator = new Intl.Collator(locale, { numeric: true, sensitivity: 'base' });
  const name = (row: RemoteRow) => row.filename || row.id;
  const added = (row: RemoteRow) => row.added ? Date.parse(row.added) : NaN;
  return [...rows].sort((left, right) => {
    let value = 0;
    if (sort.column === 'added') {
      const a = added(left), b = added(right);
      // Unknown dates stay at the bottom in both directions.
      if (Number.isFinite(a) !== Number.isFinite(b)) return Number.isFinite(a) ? -1 : 1;
      if (Number.isFinite(a)) value = a - b;
    } else if (sort.column === 'size') {
      value = left.bytes - right.bytes;
    } else if (sort.column === 'status') {
      value = collator.compare(left.status, right.status) || left.progress - right.progress;
    } else {
      value = collator.compare(name(left), name(right)) || collator.compare(left.id, right.id);
    }
    return (sort.direction === 'desc' ? -value : value)
      || collator.compare(name(left), name(right)) || collator.compare(remoteKey(left), remoteKey(right));
  });
}

export const remoteKey = (row: Pick<RemoteRow, 'provider' | 'account' | 'id' | 'kind'>) =>
  JSON.stringify([row.provider, row.account.toLowerCase(), row.kind ?? 'torrent', row.id]);

export const remoteScope = (provider: Provider, kind: CloudKind) => kind === 'torrent' ? provider : `${provider}|${kind}`;
export const parseRemoteScope = (key: string) => {
  const [provider, kind = 'torrent'] = key.split('|');
  return { provider: provider as Provider, kind: kind as CloudKind };
};

export const remoteQueuedKeys = (jobs: Job[]) => new Set(jobs.flatMap((job) =>
  job.torrentId && job.remoteAccount ? [remoteKey({ provider: job.provider, account: job.remoteAccount, id: job.torrentId })] : []));

interface RemoteApi {
  remoteList(provider: Provider, kind?: CloudKind): Promise<{ account: string; torrents: RemoteTorrent[] }>;
  remoteDelete(provider: Provider, ids: string[], listAccount: string, kind?: CloudKind): Promise<DeleteOutcome[]>;
}

export interface RemoteState extends Selection {
  rows: RemoteRow[];
  loading: boolean;
  errors: Partial<Record<string, Msg>>;
  failures: Record<string, Msg>;
  reviewing: RemoteRow[] | null;
  deleting: boolean;
}

export const emptyRemoteState = (): RemoteState => ({
  ...emptySelection(), rows: [], loading: false, errors: {}, failures: {}, reviewing: null, deleting: false
});

const message = (error: unknown): Msg => error && typeof error === 'object' && 'key' in error
  ? error as Msg : { key: String(error), args: [] };

/** Every row and request retains its service and verified account, even if Settings changes in the meantime. */
export class RemoteBrowser {
  state = emptyRemoteState();
  private generation = 0;
  private accounts?: { provider: Provider; hasToken: boolean; account: string | null }[];
  private filter: CloudFilter = 'torrent';

  constructor(private api: RemoteApi, private changed: (state: RemoteState) => void) {}

  private update(patch: Partial<RemoteState>) {
    this.state = { ...this.state, ...patch };
    this.changed(this.state);
  }

  invalidate() { this.generation++; }

  syncAccounts(accounts: AccountInfo[], filter: CloudFilter = this.filter) {
    const next = accounts.map((a) => ({ provider: a.provider, hasToken: a.hasToken, account: a.user?.username.toLowerCase() ?? null }));
    const changed = filter !== this.filter || !this.accounts || this.accounts.length !== next.length || next.some((account) => {
      const previous = this.accounts?.find((a) => a.provider === account.provider);
      // First verification emits a username while the list is loading; that identity is already in the response.
      return !previous || previous.hasToken !== account.hasToken || (previous.account !== null && previous.account !== account.account);
    });
    this.accounts = next;
    return changed ? this.load(next.filter((a) => a.hasToken).map((a) => a.provider), filter) : Promise.resolve();
  }

  async load(providers: Provider[], filter: CloudFilter = this.filter) {
    const generation = ++this.generation;
    this.filter = filter;
    const scopes = providers.flatMap((provider) => (filter === 'all' ? provider === 'torBox' ? CLOUD_KINDS : ['torrent' as const] : provider === 'torBox' || filter === 'torrent' ? [filter] : []).map((kind) => ({ provider, kind })));
    this.update({ ...emptyRemoteState(), loading: scopes.length > 0 });
    await Promise.all(scopes.map(async ({ provider, kind }) => {
      try {
        const result = await this.api.remoteList(provider, kind);
        if (generation !== this.generation) return;
        const rows = [...this.state.rows, ...result.torrents.map((torrent) => ({ ...torrent, provider, kind, account: result.account }))];
        const added = (row: RemoteRow) => {
          const time = row.added ? Date.parse(row.added) : NaN;
          return Number.isFinite(time) ? time : -Infinity;
        };
        rows.sort((a, b) => {
          const left = added(a), right = added(b);
          if (left !== right) return right - left;
          return providers.indexOf(a.provider) - providers.indexOf(b.provider) || CLOUD_KINDS.indexOf(a.kind ?? 'torrent') - CLOUD_KINDS.indexOf(b.kind ?? 'torrent');
        });
        this.update({ rows });
      } catch (error) {
        if (generation === this.generation) this.update({ errors: { ...this.state.errors, [remoteScope(provider, kind)]: message(error) } });
      }
    }));
    if (generation === this.generation) this.update({ loading: false });
  }

  clearSelection() {
    if (!this.state.deleting) this.update({ ...emptySelection(), reviewing: null });
  }

  pick(row: RemoteRow, rows: RemoteRow[], modifiers: { ctrl?: boolean; shift?: boolean }, checkbox = false) {
    if (this.state.loading || this.state.reviewing || this.state.deleting) return;
    this.update(selectItem(this.state, rows.map(remoteKey), remoteKey(row), modifiers, checkbox));
  }

  toggle(row: RemoteRow, shift = false, rows = this.state.rows) {
    this.pick(row, rows, { shift }, true);
  }

  keydown(rows: RemoteRow[], event: KeyboardEvent) {
    if (this.state.loading || this.state.reviewing || this.state.deleting) return null;
    const result = selectionKey(this.state, rows.map(remoteKey), event);
    if (result) this.update(result.selection);
    return result;
  }

  /** Selects exactly the given rows that the service has finished fetching. */
  selectDownloaded(rows: RemoteRow[]) {
    if (this.state.loading || this.state.reviewing || this.state.deleting) return;
    this.update(selectAll(rows.filter((row) => row.status === 'downloaded').map(remoteKey)));
  }

  toggleAll(rows: RemoteRow[]) {
    if (this.state.loading || this.state.reviewing || this.state.deleting) return;
    const allSelected = rows.every((row) => this.state.selected.has(remoteKey(row)));
    this.update(allSelected ? emptySelection() : selectAll(rows.map(remoteKey)));
  }

  review() {
    if (this.state.loading || this.state.deleting) return;
    const reviewing = this.state.rows.filter((row) => this.state.selected.has(remoteKey(row)));
    if (reviewing.length) this.update({ reviewing });
  }

  reviewAll(rows: RemoteRow[]) {
    if (this.state.loading || this.state.reviewing || this.state.deleting || !rows.length) return;
    const current = new Set(this.state.rows.map(remoteKey));
    const reviewing = rows.filter((row) => current.has(remoteKey(row)));
    this.update({ ...selectAll(reviewing.map(remoteKey)), reviewing });
  }

  cancelReview() {
    if (!this.state.deleting) this.update({ reviewing: null });
  }

  async confirmDelete(): Promise<number> {
    if (!this.state.reviewing || this.state.deleting) return 0;
    const generation = this.generation;
    const groups = new Map<string, RemoteRow[]>();
    for (const row of this.state.reviewing) {
      const key = JSON.stringify([row.provider, row.account.toLowerCase(), row.kind ?? 'torrent']);
      const group = groups.get(key);
      if (group) group.push(row);
      else groups.set(key, [row]);
    }
    this.update({ deleting: true, failures: {} });
    let count = 0;
    for (const rows of groups.values()) {
      if (generation !== this.generation) break;
      const { provider, account, kind = 'torrent' } = rows[0];
      const done = new Set<string>();
      const failures: Record<string, Msg> = {};
      try {
        const outcomes = await this.api.remoteDelete(provider, rows.map((row) => row.id), account, kind);
        const byId = new Map(outcomes.map((outcome) => [outcome.id, outcome]));
        for (const row of rows) {
          const outcome = byId.get(row.id);
          if (outcome?.ok) done.add(remoteKey(row));
          else if (outcome?.message) failures[remoteKey(row)] = outcome.message;
        }
      } catch (error) {
        for (const row of rows) failures[remoteKey(row)] = message(error);
      }
      if (generation !== this.generation) break;
      count += done.size;
      const remaining = this.state.rows.filter((row) => !done.has(remoteKey(row)));
      this.update({
        rows: remaining,
        ...reconcileSelection(this.state, remaining.map(remoteKey)),
        failures: { ...this.state.failures, ...failures }
      });
    }
    if (generation === this.generation) this.update({ deleting: false, reviewing: null });
    return count;
  }
}
