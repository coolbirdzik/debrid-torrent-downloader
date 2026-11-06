export interface DestinationRequest {
  /** Number of files that will be added to the queue. */
  files: number;
  /** Folder shown first: the one chosen in Settings. */
  directory: string;
}

let resolver: ((value: string | null) => void) | undefined;
export const destinationDialog = $state<{ open: boolean; files: number; directory: string }>({ open: false, files: 0, directory: '' });

/** A full path: drive letter, UNC share, or a rooted POSIX path. */
export const isAbsolutePath = (path: string) => /^([A-Za-z]:[\\/]|\\\\|\/)/.test(path.trim());

/** Asks where the files should be saved. Resolves to the chosen folder, or `null` when the user cancels. */
export function askDestination(request: DestinationRequest): Promise<string | null> {
  resolver?.(null);
  destinationDialog.files = request.files;
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
