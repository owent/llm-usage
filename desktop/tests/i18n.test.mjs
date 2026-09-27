import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { translatedKeys, translatedCatalogs } from '../src/lib/locales.ts';

function baseCatalog(name) {
  const source = readFileSync(new URL('../src/lib/i18n.svelte.ts', import.meta.url), 'utf8');
  const body = source.split('const ' + name + ': Record<string, string> = {')[1]?.split('\n};')[0];
  assert.ok(body, name + ' catalog exists');
  return Object.fromEntries(
    [...body.matchAll(/^\s*'([^']+)':\s*'((?:\\.|[^'])*)',?$/gm)].map(([, key, value]) => [key, value])
  );
}

const placeholders = (text) => [...text.matchAll(/\{[a-zA-Z][a-zA-Z0-9]*\}/g)].map((m) => m[0]).sort();

test('base catalogs have matching keys and placeholders', () => {
  const zh = baseCatalog('zhCN');
  const en = baseCatalog('en');
  assert.deepEqual(Object.keys(zh).sort(), Object.keys(en).sort());
  for (const key of Object.keys(en)) {
    assert.deepEqual(placeholders(zh[key]), placeholders(en[key]), key);
  }
});

test('every added locale covers the common interface without broken placeholders', () => {
  const en = baseCatalog('en');
  assert.equal(new Set(translatedKeys).size, translatedKeys.length);
  assert.deepEqual([...translatedKeys].sort(), Object.keys(en).sort(), 'added locales cover every interface key');
  assert.deepEqual(Object.keys(translatedCatalogs).sort(), ['zh-TW', 'ja', 'ko', 'es', 'fr', 'de', 'pt-BR', 'ru'].sort());
  for (const [locale, messages] of Object.entries(translatedCatalogs)) {
    assert.deepEqual(Object.keys(messages).sort(), [...translatedKeys].sort(), locale);
    for (const [key, value] of Object.entries(messages)) {
      assert.ok(en[key], locale + ' unknown key: ' + key);
      assert.ok(value.trim(), locale + ' empty value: ' + key);
      assert.deepEqual(placeholders(value), placeholders(en[key]), locale + ' ' + key);
    }
  }
});
