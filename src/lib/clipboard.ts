import { app } from './app.svelte';
import type { Msg } from './types';

/** Copies text and reports the result with a toast. */
export async function copyText(text: string, done: Msg): Promise<void> {
  try {
    await navigator.clipboard.writeText(text);
    app.toast(done, 'success');
  } catch {
    app.toast({ key: 'Không sao chép được vào clipboard.', args: [] }, 'error');
  }
}
