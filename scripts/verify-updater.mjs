// Verify exactly what the in-app updater trusts, before publishing any release assets.
import { createHash, createPublicKey, verify } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const config = JSON.parse(readFileSync(new URL('../src-tauri/tauri.conf.json', import.meta.url), 'utf8'));
export const UPDATER_PUBLIC_KEY = config.plugins.updater.pubkey;

export function verifyUpdaterSignature(data, encodedSignature, version, encodedPublicKey = UPDATER_PUBLIC_KEY) {
  const keyLines = Buffer.from(encodedPublicKey, 'base64').toString('utf8').trim().split(/\r?\n/);
  const key = Buffer.from(keyLines[1] ?? '', 'base64');
  const lines = Buffer.from(encodedSignature.trim(), 'base64').toString('utf8').trim().split(/\r?\n/);
  const signature = Buffer.from(lines[1] ?? '', 'base64');
  const globalSignature = Buffer.from(lines[3] ?? '', 'base64');
  if (key.length !== 42 || key.subarray(0, 2).toString() !== 'Ed' || signature.length !== 74 || globalSignature.length !== 64 || !lines[2]?.startsWith('trusted comment: ')) {
    throw new Error('Invalid updater public key or signature encoding');
  }
  if (!key.subarray(2, 10).equals(signature.subarray(2, 10))) {
    throw new Error('Updater signing key does not match the public key pinned in the app');
  }
  const algorithm = signature.subarray(0, 2).toString();
  if (algorithm !== 'ED' && algorithm !== 'Ed') throw new Error('Unsupported updater signature algorithm');
  const publicKey = createPublicKey({ key: Buffer.concat([Buffer.from('302a300506032b6570032100', 'hex'), key.subarray(10)]), format: 'der', type: 'spki' });
  const signedBytes = algorithm === 'ED' ? createHash('blake2b512').update(data).digest() : data;
  const comment = lines[2].slice('trusted comment: '.length);
  if (!verify(null, signedBytes, publicKey, signature.subarray(10)) || !verify(null, Buffer.concat([signature.subarray(10), Buffer.from(comment)]), publicKey, globalSignature)) {
    throw new Error('Invalid updater signature: artifact or trusted comment was changed');
  }
  const signedVersion = comment.split('\t').find((field) => field.startsWith('version:'))?.slice('version:'.length);
  if (signedVersion?.replace(/^v/, '') !== version.replace(/^v/, '')) {
    throw new Error(`Updater signed version ${signedVersion ?? '(missing)'} does not match ${version}`);
  }
}

export function verifyManifestArtifacts(manifest, read, publicKey = UPDATER_PUBLIC_KEY) {
  const verified = new Set();
  for (const entry of Object.values(manifest.platforms)) {
    if (verified.has(entry.url)) continue;
    const name = decodeURIComponent(new URL(entry.url).pathname.split('/').pop());
    try {
      verifyUpdaterSignature(read(name), entry.signature, manifest.version, publicKey);
    } catch (error) {
      throw new Error(`${name}: ${error.message}`);
    }
    verified.add(entry.url);
  }
  return verified.size;
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const [file, version] = process.argv.slice(2);
  if (!file || !version) throw new Error('Usage: node scripts/verify-updater.mjs <artifact> <version>');
  verifyUpdaterSignature(readFileSync(file), readFileSync(`${file}.sig`, 'utf8'), version);
  console.log('Updater signature and version match the public key pinned in the app.');
}
