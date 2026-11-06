import type { Msg } from './types';

export interface ConfirmRequest {
  title: Msg;
  body: Msg;
  confirm: Msg;
  danger?: boolean;
}

let resolver: ((value: boolean) => void) | undefined;
export const dialog = $state<{ open: boolean; request: ConfirmRequest | null }>({ open: false, request: null });

/** Cancel is the default and Escape cancels: destructive actions need an explicit confirmation. */
export function confirmAction(request: ConfirmRequest): Promise<boolean> {
  resolver?.(false);
  dialog.request = request;
  dialog.open = true;
  return new Promise((resolve) => (resolver = resolve));
}

export function answer(value: boolean) {
  dialog.open = false;
  resolver?.(value);
  resolver = undefined;
}
