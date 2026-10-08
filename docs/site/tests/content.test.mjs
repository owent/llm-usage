import test from 'node:test';
import assert from 'node:assert/strict';
import { resolve } from 'node:path';
import { root, route, rewriteLinks, pairedAnchors, referenceBody, bilingualDocument } from '../scripts/content.mjs';

test('bilingual publication includes guides and designs without translating agent skills or execution plans', () => {
  for (const key of ['README.md', 'desktop/assets/README.md', 'docs/design/documentation-site.md', 'docs/design/desktop-usage/architecture.md', 'docs/design/desktop-usage/validation.md', 'docs/validation/desktop-usage/current-acceptance.md', 'desktop/src-tauri/crates/core/tests/fixtures/codex/README.md', 'previous-draft/README.md']) assert.equal(bilingualDocument(key), true, key);
  for (const key of ['AGENTS.md', 'Plan.md', '.agents/skills/ai-maintenance/SKILL.md', 'docs/design/desktop-usage/execution.md', 'docs/zh-CN/README.md']) assert.equal(bilingualDocument(key), false, key);
});

test('links to untranslated execution records use the original repository file', () => {
  const source = resolve(root, 'docs/zh-CN/README.md');
  const architecture = resolve(root, 'docs/design/desktop-usage/architecture.md');
  const rendered = rewriteLinks('[plan](Plan.md#current-implementation)\n\n[design](docs/design/desktop-usage/architecture.md)', source, new Set([architecture]));
  assert.ok(rendered.includes('https://github.com/owent/llm-usage/blob/main/Plan.md#current-implementation'));
  assert.ok(rendered.includes('/zh-cn/reference/design/architecture/'));
});

test('repository routes preserve version punctuation and map evidence correctly', () => {
  assert.equal(route(resolve(root, 'docs/validation/desktop-usage/current-acceptance.md')), 'reference/evidence/current-acceptance');
  assert.equal(route(resolve(root, 'desktop/src-tauri/crates/core/tests/fixtures/opencode/real-1.18.34-local-default/_expectations.md')), 'reference/fixtures/opencode/real-1.18.34-local-default/expectations');
});

test('language anchors leave YAML metadata intact and Skill metadata remains visible as source', () => {
  const en = '---\ntitle: Guide\nsidebar:\n  order: 1\n---\n\n## Cache\n';
  const zh = '---\ntitle: 指南\nsidebar:\n  order: 1\n---\n\n## 缓存\n';
  const [english, chinese] = pairedAnchors(en, zh);
  assert.ok(english.startsWith(en.slice(0, en.indexOf('##'))));
  assert.ok(chinese.startsWith(zh.slice(0, zh.indexOf('##'))));
  assert.ok(english.includes('<a id="缓存"></a>\n\n## Cache'));
  const skill = referenceBody('---\nname: example\n---\n\n# Example\n\n## Steps\n');
  assert.ok(skill.startsWith('```yaml\nname: example\n```'));
  assert.ok(!skill.includes('# Example'));
  assert.ok(skill.includes('## Steps'));
});

test('Markdown links, nested image links and code examples retain their distinct semantics', () => {
  const source = resolve(root, 'docs/zh-CN/README.md');
  const plan = resolve(root, 'Plan.md');
  const body = '[![status](https://example.org/badge.svg)](Plan.md)\n\n[plan](Plan.md#progress)\n\n```md\n[example](Plan.md)\n```\n';
  const rendered = rewriteLinks(body, source, new Set([plan]));
  assert.ok(rendered.startsWith('[status](/zh-cn/reference/repository/plan/)'));
  assert.ok(rendered.includes('[plan](/zh-cn/reference/repository/plan/#progress)'));
  assert.ok(rendered.includes('```md\n[example](Plan.md)\n```'));
  assert.ok(!rendered.includes('https://example.org'));
});

test('paired heading aliases preserve translated deep-link targets without duplicate traversal', () => {
  const [english, chinese] = pairedAnchors('## Cache\n\nOne.\n\nTwo.\n\n## History\n\nThree.\n', '## 缓存\n\n一。\n\n## 历史\n\n二。\n');
  assert.ok(english.includes('<a id="缓存"></a>'));
  assert.ok(chinese.includes('<a id="cache"></a>'));
  assert.equal((english.match(/id="缓存"/g) ?? []).length, 1);
  assert.throws(() => pairedAnchors('## One\n\n## Two\n', '## 一\n'), /heading structure differs/);
  const [title] = pairedAnchors('# Title\n\nText.\n', '# 标题\n\n正文。\n');
  assert.ok(title.startsWith('# Title\n\n<a id="标题"></a>'));
  assert.ok(referenceBody(title).includes('<a id="title"></a>'));
});

test('renamed headings keep historical aliases beside the matching section in both languages', () => {
  const en = '# Guide\n\n<a id="old-guide"></a>\n\nIntroduction.\n\n## Requirements\n\nDetails.\n';
  const zh = '# 指南\n\n介绍。\n\n<a id="old-contract"></a>\n\n## 要求\n\n详情。\n';
  const [english, chinese] = pairedAnchors(en, zh);
  assert.ok(english.includes('<a id="old-contract"></a>\n\n## Requirements'));
  assert.ok(chinese.includes('# 指南\n\n<a id="guide"></a>\n\n<a id="old-guide"></a>'));
  assert.deepEqual(pairedAnchors(english, chinese), [english, chinese]);
  const literal = '## Example\n\n```html\n<a id="literal-example"></a>\n```\n';
  assert.ok(!pairedAnchors(literal, '## 示例\n\n正文。\n')[1].includes('literal-example'));
});
