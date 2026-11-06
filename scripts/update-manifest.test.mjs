import { describe, it } from 'vitest';
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, dirname, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { createManifest, UPDATER_REPOSITORY } from './update-manifest.mjs';

const artifacts = ['DebridDock_0.4.0_x64-setup.exe', 'DebridDock_0.4.0_x64_en-US.msi', 'DebridDock-portable-win-x64.exe', 'DebridDock_0.4.0_amd64.AppImage', 'DebridDock_0.4.0_amd64.deb', 'DebridDock-0.4.0-1.x86_64.rpm', 'DebridDock.app.tar.gz'];
const files = artifacts.flatMap((name) => [name, `${name}.sig`]);
const manifest = (names = files, read = () => 'signed\n') => createManifest(names, read, '0.4.0', 'coolbirdzik/debrid-torrent-downloader');

describe('release updater manifest', () => {
  it('uses the renamed repository configured in the app and rejects foreign repository input', () => {
    assert.equal(UPDATER_REPOSITORY, 'coolbirdzik/debrid-torrent-downloader');
    const config = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8'));
    assert.equal(config.plugins.updater.endpoints[0], `https://github.com/${UPDATER_REPOSITORY}/releases/latest/download/latest.json`);
    assert.deepEqual(createManifest(files, () => 'signed', '0.4.0').platforms, manifest().platforms);
    for (const repository of ['coolbirdzik/realdebrid', 'someone-else/debrid-torrent-downloader', 'coolbirdzik/another-project', '../bad']) {
      assert.throws(() => createManifest(files, () => 'signed', '0.4.0', repository), /Unexpected updater repository:.*expected coolbirdzik\/debrid-torrent-downloader/);
    }
  });

  it('runs the CI command with GITHUB_REPOSITORY and writes URLs to the renamed release', () => {
    const tempRoot = resolve(tmpdir());
    const folder = mkdtempSync(join(tempRoot, 'debriddock-manifest-'));
    try {
      for (const name of files) writeFileSync(join(folder, name), name.endsWith('.sig') ? 'signed\n' : 'fixture');
      const run = spawnSync(process.execPath, ['scripts/update-manifest.mjs', folder], {
        cwd: resolve('.'), encoding: 'utf8', env: { ...process.env, GITHUB_REPOSITORY: 'coolbirdzik/debrid-torrent-downloader' }
      });
      assert.equal(run.status, 0, run.stderr);
      const feed = JSON.parse(readFileSync(join(folder, 'latest.json'), 'utf8'));
      const { version } = JSON.parse(readFileSync('package.json', 'utf8'));
      assert.equal(feed.version, version);
      assert.ok(Object.values(feed.platforms).every((entry) => entry.url.startsWith(`https://github.com/${UPDATER_REPOSITORY}/releases/download/v${version}/`)));
    } finally {
      assert.equal(dirname(resolve(folder)), tempRoot);
      assert.ok(basename(folder).startsWith('debriddock-manifest-'));
      rmSync(folder, { recursive: true, force: true });
    }
  });

  it('keeps portable separate from the installer and maps both Macs to the universal bundle', () => {
    const feed = manifest();
    assert.match(feed.platforms['windows-x86_64'].url, /_x64-setup\.exe$/);
    assert.match(feed.platforms['windows-x86_64-portable'].url, /portable-win-x64\.exe$/);
    assert.deepEqual(feed.platforms['darwin-aarch64'], feed.platforms['darwin-x86_64']);
    assert.equal(feed.platforms['linux-x86_64'].signature, 'signed');
    assert.match(feed.platforms['linux-x86_64-deb'].url, /\.deb$/);
    assert.match(feed.platforms['linux-x86_64-rpm'].url, /\.rpm$/);
    assert.match(feed.platforms['windows-x86_64-msi'].url, /\.msi$/);
    assert.ok(Object.values(feed.platforms).every((entry) => entry.url.startsWith('https://github.com/coolbirdzik/debrid-torrent-downloader/releases/download/v0.4.0/')));
  });
  it('refuses incomplete, unsigned, empty or ambiguous builds before publishing', () => {
    assert.throws(() => manifest(files.filter((name) => !name.endsWith('AppImage.sig'))), /Missing signature/);
    assert.throws(() => manifest(files.filter((name) => !name.includes('portable'))), /Expected one/);
    assert.throws(() => manifest(files, () => ''), /Empty signature/);
    assert.throws(() => manifest([...files, 'Other_x64-setup.exe']), /Expected one/);
  });
});
