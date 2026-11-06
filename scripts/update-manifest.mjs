// Assemble the updater feed only after all three platform builds have arrived.
import { readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

export function createManifest(files, read, version, repository) {
  if (!/^\d+\.\d+\.\d+(?:-[\w.-]+)?$/.test(version)) throw new Error('Invalid release version');
  if (repository !== 'coolbirdzik/realdebrid') throw new Error('Unexpected updater repository');
  const platforms = {};
  const add = (targets, pattern) => {
    const matches = files.filter((name) => pattern.test(name));
    if (matches.length !== 1) throw new Error(`Expected one updater artifact for ${targets.join(', ')}; found ${matches.length}`);
    const name = matches[0];
    if (!files.includes(`${name}.sig`)) throw new Error(`Missing signature: ${name}`);
    const signature = read(`${name}.sig`).trim();
    if (!signature) throw new Error(`Empty signature: ${name}`);
    for (const target of targets) platforms[target] = {
      url: `https://github.com/${repository}/releases/download/v${version}/${encodeURIComponent(name)}`,
      signature
    };
  };
  add(['windows-x86_64', 'windows-x86_64-nsis'], /_x64-setup\.exe$/);
  add(['windows-x86_64-msi'], /_x64[^/]*\.msi$/);
  add(['windows-x86_64-portable'], /^DebridDock-portable-win-x64\.exe$/);
  add(['linux-x86_64', 'linux-x86_64-appimage'], /_(?:amd64|x86_64)\.AppImage$/);
  add(['linux-x86_64-deb'], /_(?:amd64|x86_64)\.deb$/);
  add(['linux-x86_64-rpm'], /(?:_|\.)(?:amd64|x86_64)\.rpm$/);
  add(['darwin-aarch64', 'darwin-x86_64'], /\.app\.tar\.gz$/);
  return { version, notes: `DebridDock ${version}`, pub_date: new Date().toISOString(), platforms };
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const folder = resolve(process.argv[2] ?? 'release');
  const { version } = JSON.parse(readFileSync('package.json', 'utf8'));
  const manifest = createManifest(readdirSync(folder), (name) => readFileSync(join(folder, name), 'utf8'), version, process.env.GITHUB_REPOSITORY ?? 'coolbirdzik/realdebrid');
  writeFileSync(join(folder, 'latest.json'), `${JSON.stringify(manifest, null, 2)}\n`);
}
