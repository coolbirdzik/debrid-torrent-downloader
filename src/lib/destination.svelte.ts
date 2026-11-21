export interface DestinationRequest {
  /** Number of files, when known before the job starts. */
  files?: number;
  /** Folder shown first: the one chosen in Settings. */
  directory: string;
}

let resolver: ((value: string | null) => void) | undefined;
export const destinationDialog = $state<{ open: boolean; files: number | null; directory: string }>({ open: false, files: null, directory: '' });

/** A full path: drive letter, UNC share, or a rooted POSIX path. */
export const isAbsolutePath = (path: string) => /^([A-Za-z]:[\\/]|\\\\|\/)/.test(path.trim());

/** Asks where the files should be saved. Resolves to the chosen folder, or `null` when the user cancels. */
export function askDestination(request: DestinationRequest): Promise<string | null> {
  resolver?.(null);
  destinationDialog.files = request.files ?? null;
  destinationDialog.directory = request.directory;
  destinationDialog.open = true;
  return new Promise((resolve) => (resolver = resolve));
}

export function answerDestination(confirmed: boolean) {
  const directory = destinationDialog.directory.trim();
  if (confirmed && !isAbsolutePath(directory)) return;
  destinationDialog.open = false;
  resolver?.(confirmed ? directory : null);
  resolver = undefined;
}
