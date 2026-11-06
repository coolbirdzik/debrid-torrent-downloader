import { describe, expect, it } from 'vitest';
import { createHash, generateKeyPairSync, randomBytes, sign } from 'node:crypto';
import { verifyManifestArtifacts, verifyUpdaterSignature } from './verify-updater.mjs';
import { createManifest } from './update-manifest.mjs';

function signer() {
  const { publicKey, privateKey } = generateKeyPairSync('ed25519');
  const keyId = randomBytes(8);
  const raw = publicKey.export({ format: 'der', type: 'spki' }).subarray(-32);
  const pubkey = Buffer.from(`untrusted comment: test public key\n${Buffer.concat([Buffer.from('Ed'), keyId, raw]).toString('base64')}\n`).toString('base64');
  return {
    pubkey,
    signature(data, version = '0.4.0', algorithm = 'ED') {
      const bytes = algorithm === 'ED' ? createHash('blake2b512').update(data).digest() : data;
      const signature = sign(null, bytes, privateKey);
      const comment = `timestamp:1\tfile:app\tversion:${version}`;
      const global = sign(null, Buffer.concat([signature, Buffer.from(comment)]), privateKey);
      return Buffer.from(`untrusted comment: test signature\n${Buffer.concat([Buffer.from(algorithm), keyId, signature]).toString('base64')}\ntrusted comment: ${comment}\n${global.toString('base64')}\n`).toString('base64');
    }
  };
}

describe('updater artifact verification before release', () => {
  it('accepts genuine version-bound signatures, including legacy non-prehashed data', () => {
    const key = signer(), data = Buffer.from('native application');
    for (const algorithm of ['ED', 'Ed']) {
      expect(() => verifyUpdaterSignature(data, key.signature(data, '0.4.0', algorithm), '0.4.0', key.pubkey)).not.toThrow();
    }
  });

  it('rejects a different signing key, modified payload, modified comment and mismatched version', () => {
    const key = signer(), other = signer(), data = Buffer.from('native application');
    const signature = key.signature(data);
    expect(() => verifyUpdaterSignature(data, signature, '0.4.0', other.pubkey)).toThrow(/signing key does not match/);
    expect(() => verifyUpdaterSignature(Buffer.from('modified application'), signature, '0.4.0', key.pubkey)).toThrow(/Invalid updater signature/);
    const tampered = Buffer.from(Buffer.from(signature, 'base64').toString().replace('version:0.4.0', 'version:9.9.9')).toString('base64');
    expect(() => verifyUpdaterSignature(data, tampered, '9.9.9', key.pubkey)).toThrow(/Invalid updater signature/);
    expect(() => verifyUpdaterSignature(data, signature, '0.4.1', key.pubkey)).toThrow(/signed version/);
  });

  it('verifies every installer and portable artifact in the generated CI manifest', () => {
    const key = signer();
    const names = ['DebridDock_0.4.0_x64-setup.exe', 'DebridDock_0.4.0_x64_en-US.msi', 'DebridDock-portable-win-x64.exe', 'DebridDock_0.4.0_amd64.AppImage', 'DebridDock_0.4.0_amd64.deb', 'DebridDock-0.4.0-1.x86_64.rpm', 'DebridDock.app.tar.gz'];
    const files = new Map();
    for (const name of names) {
      const data = Buffer.from(`payload ${name}`);
      files.set(name, data);
      files.set(`${name}.sig`, key.signature(data));
    }
    const manifest = createManifest([...files.keys()], (name) => files.get(name), '0.4.0');
    expect(verifyManifestArtifacts(manifest, (name) => files.get(name), key.pubkey)).toBe(7);
    files.set('DebridDock-portable-win-x64.exe', Buffer.from('broken portable download'));
    expect(() => verifyManifestArtifacts(manifest, (name) => files.get(name), key.pubkey)).toThrow(/portable-win-x64.exe: Invalid updater signature/);
  });
});
