// Lists every source string used by the UI and the Rust core that has no English translation.
// Vietnamese is the source language and the key of src/lib/en.json.
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';

const catalog = JSON.parse(readFileSync('src/lib/en.json', 'utf8'));
const files = [];
const walk = (dir) => {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) walk(path);
    else if (/\.(svelte|ts|rs)$/.test(name) && !name.endsWith('.d.ts')) files.push(path);
  }
};
walk('src');
walk('crates/core/src');
walk('src-tauri/src');

const rust = [/(?:Msg::new|Msg::plain|Error::operation)\(\s*"((?:[^"\\]|\\.)*)"/g];
const web = [/\bt\(\s*'((?:[^'\\]|\\.)*)'/g, /key:\s*'((?:[^'\\]|\\.)*)'/g];
const missing = new Map();
for (const file of files) {
  const text = readFileSync(file, 'utf8');
  for (const pattern of file.endsWith('.rs') ? rust : web) {
    for (const match of text.matchAll(pattern)) {
      const key = match[1].replace(/\\n/g, '\n').replace(/\\'/g, "'").replace(/\\"/g, '"');
      // Only user-facing Vietnamese sentences need a translation (technical English strings stay as they are).
      const vietnamese = /[À-ỹ]/.test(key);
      if (vietnamese && !(key in catalog)) missing.set(key, file);
    }
  }
}
for (const [key, file] of missing) console.log(`${file}\t${JSON.stringify(key)}`);
console.log(`${missing.size} untranslated Vietnamese source strings`);
process.exitCode = missing.size ? 1 : 0;
