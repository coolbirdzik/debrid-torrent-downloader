import type { Provider } from './types';

export interface ProviderChoice {
  providers: Provider[];
  /** Reuse this choice next time without asking. */
  remember: boolean;
}

let resolver: ((value: ProviderChoice | null) => void) | undefined;

/** The "add to which service?" dialog shown when torrents are added to the Library. */
export const providerDialog = $state<{ open: boolean; count: number; options: Provider[]; chosen: Provider[]; remember: boolean }>({
  open: false,
  count: 0,
  options: [],
  chosen: [],
  remember: false
});

/**
 * Which services the next Library import goes to. With one usable service (or none yet) there is nothing to ask; a
 * remembered choice is reused as long as one of its services still has a token.
 */
export function chooseProviders(request: { count: number; usable: Provider[]; fallback: Provider; saved: Provider[]; remember: boolean }): Promise<ProviderChoice | null> {
  const { usable, saved } = request;
  if (usable.length === 0) return Promise.resolve({ providers: [request.fallback], remember: request.remember });
  if (usable.length === 1) return Promise.resolve({ providers: usable, remember: request.remember });
  const kept = saved.filter((p) => usable.includes(p));
  if (request.remember && kept.length) return Promise.resolve({ providers: kept, remember: true });
  resolver?.(null);
  providerDialog.count = request.count;
  providerDialog.options = usable;
  providerDialog.chosen = kept.length ? kept : usable.includes(request.fallback) ? [request.fallback] : [usable[0]];
  providerDialog.remember = request.remember;
  providerDialog.open = true;
  return new Promise((resolve) => (resolver = resolve));
}

export function toggleProvider(provider: Provider) {
  const chosen = providerDialog.chosen;
  providerDialog.chosen = chosen.includes(provider) ? chosen.filter((p) => p !== provider) : providerDialog.options.filter((p) => p === provider || chosen.includes(p));
}

export function toggleAllProviders() {
  providerDialog.chosen = providerDialog.chosen.length === providerDialog.options.length ? [] : [...providerDialog.options];
}

export function answerProviders(confirmed: boolean) {
  if (confirmed && providerDialog.chosen.length === 0) return;
  providerDialog.open = false;
  resolver?.(confirmed ? { providers: [...providerDialog.chosen], remember: providerDialog.remember } : null);
  resolver = undefined;
}
