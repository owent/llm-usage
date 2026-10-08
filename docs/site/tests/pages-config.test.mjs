import test from 'node:test';
import assert from 'node:assert/strict';
import { pagesProblems } from '../scripts/pages-config.mjs';

test('Pages verification rejects another branch, directory, publishing mechanism or domain', () => {
  const configured = { source: { branch: 'gh-pages', path: '/' }, build_type: 'legacy', cname: 'llm-usage.atframe.work' };
  assert.deepEqual(pagesProblems(configured), []);
  assert.ok(pagesProblems({ ...configured, source: { branch: 'main', path: '/docs' } }).length);
  assert.ok(pagesProblems({ ...configured, build_type: 'workflow' }).length);
  assert.ok(pagesProblems({ ...configured, cname: null }).length);
});
