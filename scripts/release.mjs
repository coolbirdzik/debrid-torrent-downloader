// Cut a release: bump the version everywhere, commit, tag vX.Y.Z and push.
//
//   node scripts/release.mjs <X.Y.Z | patch | minor | major> [--dry-run] [--no-push] [--skip-checks] [--yes]
//
// Pushing the tag starts the "Build and release" workflow, which builds the installers and drafts a GitHub
// release. The draft still has to be published by hand before the in-app update check can see it.
import { spawnSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { createInterface } from 'node:readline/promises';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
process.chdir(root);

const args = process.argv.slice(2);
const flags = new Set(args.filter((a) => a.startsWith('--')));
const target = args.find((a) => !a.startsWith('--'));
const dryRun = flags.has('--dry-run');
const push = !flags.has('--no-push');

const fail = (message) => {
  console.error(`\n✖ ${message}`);
  process.exit(1);
};
const say = (message) => console.log(message);

function run(command, commandArgs, { allowFail = false } = {}) {
  // pnpm is a .cmd shim on Windows, which needs a shell.
  const result = spawnSync(command, commandArgs, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], shell: process.platform === 'win32' && command === 'pnpm' });
  if (result.status !== 0 && !allowFail) fail(`${command} ${commandArgs.join(' ')}\n${(result.stderr || result.stdout || '').trim()}`);
  return (result.stdout ?? '').trim();
}

function runVisible(command, commandArgs) {
  const result = spawnSync(command, commandArgs, { stdio: 'inherit', shell: process.platform === 'win32' && command === 'pnpm' });
  if (result.status !== 0) fail(`${command} ${commandArgs.join(' ')} failed`);
}

// Files that carry the version. Each edit replaces only the version text, so line endings are preserved.
const targets = [
  { file: 'package.json', pattern: /("version":\s*")([^"]+)(")/ },
  { file: 'src-tauri/tauri.conf.json', pattern: /("version":\s*")([^"]+)(")/ },
  { file: 'src-tauri/Cargo.toml', pattern: /(^version\s*=\s*")([^"]+)(")/m },
  { file: 'crates/core/Cargo.toml', pattern: /(^version\s*=\s*")([^"]+)(")/m },
  // Cargo.lock lists the workspace crates too; `cargo check --locked` in CI fails if it is stale.
  { file: 'Cargo.lock', pattern: /(name = "rdbatch-core"\r?\nversion = ")([^"]+)(")/ },
  { file: 'Cargo.lock', pattern: /(name = "rdbatch-desktop"\r?\nversion = ")([^"]+)(")/ }
];

const semver = /^(\d+)\.(\d+)\.(\d+)(-[0-9A-Za-z.-]+)?$/;
const current = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8')).version;
const parts = semver.exec(current);
if (!parts) fail(`Current version "${current}" in tauri.conf.json is not MAJOR.MINOR.PATCH.`);
const [major, minor, patch] = parts.slice(1, 4).map(Number);

let next;
if (target === 'patch') next = `${major}.${minor}.${patch + 1}`;
else if (target === 'minor') next = `${major}.${minor + 1}.0`;
else if (target === 'major') next = `${major + 1}.0.0`;
else if (target && semver.test(target.replace(/^v/, ''))) next = target.replace(/^v/, '');
else fail('Usage: node scripts/release.mjs <X.Y.Z | patch | minor | major> [--dry-run] [--no-push] [--skip-checks] [--yes]');

const tag = `v${next}`;
const prerelease = next.includes('-');

// Every file must already agree on the current version, otherwise a bump would hide a mistake.
for (const { file, pattern } of targets) {
  const match = pattern.exec(readFileSync(file, 'utf8'));
  if (!match) fail(`Could not find the version in ${file}.`);
  if (match[2] !== current) fail(`${file} is at ${match[2]} but tauri.conf.json is at ${current}. Make them agree first.`);
}

const compare = (a, b) => {
  const [x, y] = [a, b].map((v) => v.split('-')[0].split('.').map(Number));
  for (let i = 0; i < 3; i++) if (x[i] !== y[i]) return x[i] - y[i];
  return (a.includes('-') ? 0 : 1) - (b.includes('-') ? 0 : 1);
};
if (compare(next, current) <= 0) fail(`${next} is not newer than the current version ${current}.`);

const branch = run('git', ['rev-parse', '--abbrev-ref', 'HEAD']);
if (branch !== 'main') fail(`Releases are cut from main, but the current branch is "${branch}".`);
const dirty = run('git', ['status', '--porcelain']);
if (dirty) fail(`The working tree has uncommitted changes. Commit or stash them first:\n${dirty}`);
run('git', ['fetch', 'origin', 'main', '--tags']);
const behind = Number(run('git', ['rev-list', '--count', 'HEAD..origin/main']));
if (behind > 0) fail(`main is ${behind} commit(s) behind origin/main. Pull first.`);
if (run('git', ['tag', '--list', tag])) fail(`Tag ${tag} already exists locally.`);
if (run('git', ['ls-remote', '--tags', 'origin', `refs/tags/${tag}`])) fail(`Tag ${tag} already exists on origin.`);

say(`Release ${current} → ${next}  (tag ${tag}${prerelease ? ', will be a GitHub prerelease' : ''})`);
for (const file of new Set(targets.map((t) => t.file))) say(`  • ${file}`);
say(push ? '  then: commit, tag, push main and the tag' : '  then: commit and tag (no push)');

if (prerelease && !/-\d+$/.test(next)) {
  say("\n⚠ Tauri's Windows .msi installer only accepts a numeric prerelease such as 0.4.0-1. A suffix like -beta.1 will likely fail the Windows bundle.");
}

if (dryRun) {
  say('\nDry run: nothing was changed.');
  process.exit(0);
}

if (!flags.has('--skip-checks')) {
  say('\nRunning the same checks as CI…');
  runVisible('cargo', ['test', '-p', 'rdbatch-core', '--locked']);
  runVisible('pnpm', ['check']);
  runVisible('pnpm', ['test']);
  runVisible('node', ['scripts/check-i18n.mjs']);
}

if (!flags.has('--yes')) {
  const rl = createInterface({ input: process.stdin, output: process.stdout });
  const answer = (await rl.question(`\n${push ? 'Commit, tag and PUSH' : 'Commit and tag'} ${tag}? (y/N) `)).trim().toLowerCase();
  rl.close();
  if (answer !== 'y' && answer !== 'yes') fail('Cancelled. Nothing was changed.');
}

for (const { file, pattern } of targets) {
  const text = readFileSync(file, 'utf8');
  writeFileSync(file, text.replace(pattern, `$1${next}$3`));
}
run('cargo', ['metadata', '--locked', '--offline', '--format-version', '1', '--no-deps']); // proves Cargo.lock agrees

run('git', ['add', ...new Set(targets.map((t) => t.file))]);
run('git', ['commit', '-m', `Release ${tag}`]);
run('git', ['tag', '-a', tag, '-m', `DebridDock ${next}`]);
say(`\n✔ Committed and tagged ${tag}.`);

if (push) {
  run('git', ['push', '--atomic', 'origin', 'main', tag]);
  say(`✔ Pushed main and ${tag}. The "Build and release" workflow is now drafting the release.`);
  say('  Publish the draft on GitHub (Releases) when the build is green.');
} else {
  say(`  Push when ready:  git push --atomic origin main ${tag}`);
}
