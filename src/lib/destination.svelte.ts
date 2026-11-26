import { app } from './app.svelte';

export interface DestinationRequest {
  /** Number of files, when known before the job starts. */
  files?: number;
  /** Files already saved on this computer: skipped unless the user picks Force download. */
  downloaded?: number;
  /** Folder shown first: the one chosen in Settings. */
  directory: string;
}

export interface DestinationAnswer {
  directory: string;
  /** Also queue the downloaded files again, as new files next to the existing ones. */
  force: boolean;
}

let resolver: ((value: DestinationAnswer | null) => void) | undefined;
export const destinationDialog = $state<{ open: boolean; files: number | null; downloaded: number; directory: string; makeDefault: boolean }>({
  open: false, files: null, downloaded: 0, directory: '', makeDefault: false
});

/** A full path: drive letter, UNC share, or a rooted POSIX path. */
export const isAbsolutePath = (path: string) => /^([A-Za-z]:[\\/]|\\\\|\/)/.test(path.trim());

/** The typed folder differs from the Settings folder, so it can be saved as the new default. */
export const canMakeDefault = (directory: string) => isAbsolutePath(directory) && directory.trim() !== app.settings.outputDirectory.trim();

/** Asks where the files should be saved. Resolves to the chosen folder, or `null` when the user cancels. */
export function askDestination(request: DestinationRequest): Promise<DestinationAnswer | null> {
  resolver?.(null);
  destinationDialog.files = request.files ?? null;
  destinationDialog.downloaded = request.downloaded ?? 0;
  destinationDialog.directory = request.directory;
  destinationDialog.makeDefault = false;
  destinationDialog.open = true;
  return new Promise((resolve) => (resolver = resolve));
}

/** `force` is only ever sent by the Force download button, never by Enter. */
export function answerDestination(confirmed: boolean, force = false) {
  const directory = destinationDialog.directory.trim();
  if (confirmed && !isAbsolutePath(directory)) return;
  // Nothing new to add: only Force download can confirm.
  if (confirmed && !force && destinationDialog.files === 0) return;
  if (confirmed && destinationDialog.makeDefault && canMakeDefault(directory)) void app.setSettings({ outputDirectory: directory });
  destinationDialog.open = false;
  resolver?.(confirmed ? { directory, force: force && destinationDialog.downloaded > 0 } : null);
  resolver = undefined;
}
