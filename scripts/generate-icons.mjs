// Keep the sidebar, browser favicon and desktop bundles on the same source artwork.
import { spawnSync } from 'node:child_process';
import { copyFileSync, mkdtempSync, readdirSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const require = createRequire(import.meta.url);
const cli = join(dirname(require.resolve('@tauri-apps/cli/package.json')), 'tauri.js');
const source = join(root, 'static', 'app-icon.png');
const output = mkdtempSync(join(tmpdir(), 'debriddock-icons-'));

try {
  const result = spawnSync(process.execPath, [cli, 'icon', source, '--output', output], {
    cwd: root,
    stdio: 'inherit'
  });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`Icon generation failed (${result.status}).`);

  // Tauri also emits mobile assets; this application ships desktop bundles only.
  for (const entry of readdirSync(output, { withFileTypes: true })) {
    if (entry.isFile() && /\.(png|ico|icns)$/.test(entry.name)) {
      copyFileSync(join(output, entry.name), join(root, 'src-tauri', 'icons', entry.name));
    }
  }
  copyFileSync(source, join(root, 'src-tauri', 'icons', 'icon.png'));
  copyFileSync(join(output, '32x32.png'), join(root, 'static', 'favicon.png'));
  console.log('Updated desktop icons and favicon from static/app-icon.png.');
} finally {
  rmSync(output, { recursive: true, force: true });
}
