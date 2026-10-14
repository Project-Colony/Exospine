// Checks the interface strings: en and fr have the same keys, no empty value
// and the same {placeholders}, and every t('key') / tn('key', n) / data-i18n
// attribute in the UI names a key that exists. With --verbose it also
// lists the French strings much longer than their English one, so the layout
// can be checked against them.
//
// Run from the repository root: node scripts/check-i18n.mjs [--verbose]

import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';

const UI = 'assets/ui';
const { default: en } = await import(pathToFileURL(`${UI}/js/i18n/en.js`));
const { default: fr } = await import(pathToFileURL(`${UI}/js/i18n/fr.js`));

const errors = [];
const warnings = [];
const placeholders = (s) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort().join(',');

for (const [lang, table, other] of [['en', en, fr], ['fr', fr, en]]) {
  for (const [key, value] of Object.entries(table)) {
    if (!(key in other)) errors.push(`${key} is in ${lang} only`);
    if (typeof value !== 'string' || !value.trim()) errors.push(`${lang}.${key} is empty`);
  }
}
for (const key of Object.keys(en)) {
  if (!(key in fr)) continue;
  if (placeholders(en[key]) !== placeholders(fr[key])) {
    errors.push(`${key}: en has {${placeholders(en[key])}} but fr has {${placeholders(fr[key])}}`);
  }
  // French runs 15-20% longer than English; well past that, check the layout.
  if (en[key].length >= 16 && fr[key].length > en[key].length * 1.4) {
    warnings.push(`${key}: French is ${Math.round((fr[key].length / en[key].length - 1) * 100)}% longer`);
  }
}

const files = [];
(function walk(dir) {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) {
      if (name !== 'vendor') walk(path);
    } else if (/\.(js|html)$/.test(name)) {
      files.push(path);
    }
  }
})(UI);

for (const file of files) {
  const src = readFileSync(file, 'utf8');
  for (const m of src.matchAll(/\bt\(\s*'([\w.]+)'/g)) {
    if (!(m[1] in en)) errors.push(`${file}: t('${m[1]}') has no string`);
  }
  for (const m of src.matchAll(/\btn\(\s*'([\w.]+)'/g)) {
    for (const form of ['one', 'other']) {
      if (!(`${m[1]}_${form}` in en)) errors.push(`${file}: tn('${m[1]}') has no ${m[1]}_${form}`);
    }
  }
  for (const m of src.matchAll(/data-i18n(?:-[\w-]+)?="([\w.]+)"/g)) {
    if (!(m[1] in en)) errors.push(`${file}: data-i18n "${m[1]}" has no string`);
  }
}

if (process.argv.includes('--verbose')) for (const w of warnings) console.warn(`long: ${w}`);
for (const e of errors) console.error(`error: ${e}`);
console.log(`${Object.keys(en).length} keys, ${errors.length} error(s), ${warnings.length} French string(s) over 40% longer`);
process.exit(errors.length ? 1 : 0);
