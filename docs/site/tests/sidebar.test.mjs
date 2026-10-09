import test from 'node:test';
import assert from 'node:assert/strict';
import { documentationSidebar, sidebarGroups } from '../scripts/sidebar.mjs';

const pages = [
  'start/install', 'guide/sources', 'development/setup', 'reference/design/architecture',
  'reference/evidence/current-acceptance', 'reference/repository/readme',
  'reference/fixtures/opencode/real-1.18.34/expectations',
].map(id => ({ id }));
const slugs = items => items.flatMap(item => item.slug ? [item.slug] : slugs(item.items));

test('sidebar uses localized Starlight page slugs for guides and nested versioned references', () => {
  const groups = sidebarGroups(pages);
  assert.equal(groups.length, 7);
  assert.deepEqual(slugs(groups).sort(), pages.map(page => page.id).sort());
  assert.equal(groups[0].translations['zh-CN'], '开始使用');
  assert.equal(groups[6].items[0].items[0].items[0].slug, 'reference/fixtures/opencode/real-1.18.34/expectations');
  assert.ok(!JSON.stringify(groups).includes('autogenerate'));
});

test('sidebar honors page order and rejects duplicate pages or empty categories', () => {
  const ordered = sidebarGroups([...pages, { id: 'start/quick-start', order: 1 }, { id: 'start/about', order: 2 }]);
  assert.deepEqual(slugs(ordered[0].items), ['start/quick-start', 'start/about', 'start/install']);
  assert.throws(() => sidebarGroups([...pages, pages[0]]), /Duplicate sidebar page/);
  assert.throws(() => sidebarGroups(pages.slice(1)), /Empty sidebar category: start/);
});

test('real source inventory produces every required menu page without generated content paths', async () => {
  const groups = await documentationSidebar();
  const ids = slugs(groups);
  for (const id of ['start/installation', 'guide/dashboard', 'guide/sources', 'development/documentation',
    'reference/design/documentation-site', 'reference/evidence/documentation-site', 'reference/repository/readme',
    'reference/design/application-updates', 'reference/evidence/application-updates', 'reference/evidence/release-030',
    'reference/evidence/update-check-repair']) {
    assert.ok(ids.includes(id), id);
  }
  assert.equal(ids.length, 223);
  assert.ok(!ids.some(id => id.includes('build/') || id.includes('zh-cn/') || id.endsWith('/plan')));
});
