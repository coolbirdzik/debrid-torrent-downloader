import { describe, it } from 'vitest';
import assert from 'node:assert/strict';
import { createManifest } from './update-manifest.mjs';

const artifacts = ['DebridDock_0.4.0_x64-setup.exe', 'DebridDock_0.4.0_x64_en-US.msi', 'DebridDock-portable-win-x64.exe', 'DebridDock_0.4.0_amd64.AppImage', 'DebridDock_0.4.0_amd64.deb', 'DebridDock-0.4.0-1.x86_64.rpm', 'DebridDock.app.tar.gz'];
const files = artifacts.flatMap((name) => [name, `${name}.sig`]);
const manifest = (names = files, read = () => 'signed\n') => createManifest(names, read, '0.4.0', 'coolbirdzik/realdebrid');

describe('release updater manifest', () => {
  it('keeps portable separate from the installer and maps both Macs to the universal bundle', () => {
    const feed = manifest();
    assert.match(feed.platforms['windows-x86_64'].url, /_x64-setup\.exe$/);
    assert.match(feed.platforms['windows-x86_64-portable'].url, /portable-win-x64\.exe$/);
    assert.deepEqual(feed.platforms['darwin-aarch64'], feed.platforms['darwin-x86_64']);
    assert.equal(feed.platforms['linux-x86_64'].signature, 'signed');
    assert.match(feed.platforms['linux-x86_64-deb'].url, /\.deb$/);
    assert.match(feed.platforms['linux-x86_64-rpm'].url, /\.rpm$/);
    assert.match(feed.platforms['windows-x86_64-msi'].url, /\.msi$/);
    assert.ok(Object.values(feed.platforms).every((entry) => entry.url.startsWith('https://github.com/coolbirdzik/realdebrid/releases/download/v0.4.0/')));
  });
  it('refuses incomplete, unsigned, empty or ambiguous builds before publishing', () => {
    assert.throws(() => manifest(files.filter((name) => !name.endsWith('AppImage.sig'))), /Missing signature/);
    assert.throws(() => manifest(files.filter((name) => !name.includes('portable'))), /Expected one/);
    assert.throws(() => manifest(files, () => ''), /Empty signature/);
    assert.throws(() => manifest([...files, 'Other_x64-setup.exe']), /Expected one/);
  });
});
