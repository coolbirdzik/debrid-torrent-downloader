// Assemble the updater feed only after all three platform builds have arrived.
import { readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

// The published feed and its assets must use the same repository as the app's updater endpoint.
const config = JSON.parse(readFileSync(new URL('../src-tauri/tauri.conf.json', import.meta.url), 'utf8'));
const endpoint = config.plugins?.updater?.endpoints?.[0];
const repositoryMatch = typeof endpoint === 'string' && /^https:\/\/github\.com\/([\w-]+\/[\w.-]+)\/releases\/latest\/download\/latest\.json$/.exec(endpoint);
if (!repositoryMatch) throw new Error('Expected a GitHub latest.json updater endpoint in src-tauri/tauri.conf.json');
export const UPDATER_REPOSITORY = repositoryMatch[1];

export function createManifest(files, read, version, repository = UPDATER_REPOSITORY) {
  if (!/^\d+\.\d+\.\d+(?:-[\w.-]+)?$/.test(version)) throw new Error('Invalid release version');
  if (repository !== UPDATER_REPOSITORY) throw new Error(`Unexpected updater repository: ${repository}; expected ${UPDATER_REPOSITORY} from src-tauri/tauri.conf.json`);
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
  const { version } = JSON.parse(readFileSync(new URL('../package.json', import.meta.url), 'utf8'));
  const manifest = createManifest(readdirSync(folder), (name) => readFileSync(join(folder, name), 'utf8'), version, process.env.GITHUB_REPOSITORY ?? UPDATER_REPOSITORY);
  writeFileSync(join(folder, 'latest.json'), `${JSON.stringify(manifest, null, 2)}\n`);
}
