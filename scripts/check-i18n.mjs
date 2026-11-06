// 1. Lists every source string used by the UI and the Rust core that has no English translation.
// 2. Checks that every other locale file has exactly the keys of en.json, with the same {n} placeholders and line breaks.
// Vietnamese is the source language and the key of src/lib/locales/en.json.
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';

const catalog = JSON.parse(readFileSync('src/lib/locales/en.json', 'utf8'));
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

const localeDir = 'src/lib/locales';
const placeholders = (text) => [...text.matchAll(/\{(\d+)(?::[^}]*)?\}/g)].map((m) => m[1]).sort().join(',');
const lineBreaks = (text) => text.split('\n').length;
let localeProblems = 0;
const report = (locale, message) => { localeProblems++; console.log(`${locale}\t${message}`); };
for (const name of readdirSync(localeDir).filter((n) => n.endsWith('.json') && n !== 'en.json')) {
  const locale = JSON.parse(readFileSync(join(localeDir, name), 'utf8'));
  for (const key of Object.keys(catalog)) {
    if (!(key in locale)) report(name, `missing ${JSON.stringify(key)}`);
    else if (typeof locale[key] !== 'string' || !locale[key].trim()) report(name, `empty ${JSON.stringify(key)}`);
    else {
      if (placeholders(locale[key]) !== placeholders(key)) report(name, `placeholder mismatch ${JSON.stringify(key)} -> ${JSON.stringify(locale[key])}`);
      if (lineBreaks(locale[key]) !== lineBreaks(key)) report(name, `line break mismatch ${JSON.stringify(key)}`);
    }
  }
  for (const key of Object.keys(locale)) if (!(key in catalog)) report(name, `unknown key ${JSON.stringify(key)}`);
}
console.log(`${localeProblems} locale problems`);
process.exitCode = missing.size || localeProblems ? 1 : 0;
