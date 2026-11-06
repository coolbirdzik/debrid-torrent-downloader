// Builds a portable copy of the app: the bare executable (no installer) plus a `portable` marker file, so the app
// keeps its queue, library and settings in a `data` folder next to the executable. Run with `pnpm build:portable`.
// Output: dist/portable/ and, on Windows, dist/DebridDock-portable-win-x64.zip.
import { spawnSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const windows = process.platform === 'win32';
const run = (command, args) => {
	const result = spawnSync(command, args, { cwd: root, stdio: 'inherit', shell: windows });
	if (result.status !== 0) process.exit(result.status ?? 1);
};

run('pnpm', ['tauri', 'build', '--no-bundle']);

const binary = join(root, 'target', 'release', windows ? 'rdbatch-desktop.exe' : 'rdbatch-desktop');
if (!existsSync(binary)) {
	console.error(`Build finished but ${binary} is missing.`);
	process.exit(1);
}

const folder = join(root, 'dist', 'portable');
rmSync(folder, { recursive: true, force: true });
mkdirSync(folder, { recursive: true });
copyFileSync(binary, join(folder, windows ? 'DebridDock.exe' : 'debriddock'));
writeFileSync(join(folder, 'portable'), 'Presence of this file keeps all app data in the "data" folder next to the executable.\n');

if (windows) {
	const zip = join(root, 'dist', 'DebridDock-portable-win-x64.zip');
	rmSync(zip, { force: true });
	run('powershell', ['-NoProfile', '-Command', `Compress-Archive -Path '${folder}\\*' -DestinationPath '${zip}'`]);
	console.log(`\nPortable zip: ${zip}`);
}
console.log(`Portable folder: ${folder}`);
