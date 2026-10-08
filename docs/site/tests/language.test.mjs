import test from 'node:test';
import assert from 'node:assert/strict';
import { preferredLanguage, localizedPath } from '../src/lib/language.mjs';

test('browser negotiation and explicit preference use a deterministic English fallback', () => {
  assert.equal(preferredLanguage(['zh-TW', 'en']), 'zh-CN');
  assert.equal(preferredLanguage(['en-GB', 'zh-CN']), 'en');
  assert.equal(preferredLanguage(['fr-FR', 'zh-HK']), 'zh-CN');
  assert.equal(preferredLanguage(['ja-JP']), 'en');
  assert.equal(preferredLanguage([]), 'en');
  assert.equal(preferredLanguage(['zh-CN'], 'en'), 'en');
  assert.equal(preferredLanguage(['en'], 'zh-CN'), 'zh-CN');
  assert.equal(preferredLanguage(['en'], 'invalid'), 'en');
});

test('locale switching preserves deep paths and does not stack prefixes', () => {
  assert.equal(localizedPath('/guide/sources/', 'zh-CN'), '/zh-cn/guide/sources/');
  assert.equal(localizedPath('/zh-cn/guide/sources/', 'en'), '/guide/sources/');
  assert.equal(localizedPath('/zh-cn/guide/sources/', 'zh-CN'), '/zh-cn/guide/sources/');
  assert.equal(localizedPath('/zh-cn/', 'en'), '/');
  assert.equal(localizedPath('/zh-cn-other/', 'en'), '/zh-cn-other/');
});
