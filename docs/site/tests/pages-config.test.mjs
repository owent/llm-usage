import test from 'node:test';
import assert from 'node:assert/strict';
import { pagesProblems, configurePublicationEnvironment } from '../scripts/pages-config.mjs';

test('Pages verification rejects another branch, directory, publishing mechanism or domain', () => {
  const configured = { source: { branch: 'gh-pages', path: '/' }, build_type: 'legacy', cname: 'llm-usage.atframe.work' };
  assert.deepEqual(pagesProblems(configured), []);
  assert.ok(pagesProblems({ ...configured, source: { branch: 'main', path: '/docs' } }).length);
  assert.ok(pagesProblems({ ...configured, build_type: 'workflow' }).length);
  assert.ok(pagesProblems({ ...configured, cname: null }).length);
});

test('administrator setup adds exactly the main branch while preserving branch and tag rules', () => {
  const rules = [{ id: 1, name: 'gh-pages', type: 'branch' }, { id: 2, name: 'main', type: 'tag' }];
  const writes = [];
  const api = (method, body, target) => {
    if (method === 'POST') {
      assert.equal(target, 'repos/owent/llm-usage/environments/github-pages/deployment-branch-policies');
      writes.push(body);
      rules.push({ id: 3, ...body });
      return rules.at(-1);
    }
    if (target.endsWith('/github-pages')) return { deployment_branch_policy: { custom_branch_policies: true } };
    return { total_count: rules.length, branch_policies: [...rules] };
  };
  configurePublicationEnvironment(api);
  configurePublicationEnvironment(api);
  assert.deepEqual(writes, [{ name: 'main', type: 'branch' }]);
  assert.deepEqual(rules.slice(0, 2), [{ id: 1, name: 'gh-pages', type: 'branch' }, { id: 2, name: 'main', type: 'tag' }]);
});

test('administrator setup preserves non-custom environment protection settings', () => {
  for (const policy of [null, { protected_branches: true, custom_branch_policies: false }]) {
    let reads = 0;
    configurePublicationEnvironment(method => {
      assert.equal(method, 'GET');
      reads++;
      return { deployment_branch_policy: policy };
    });
    assert.equal(reads, 1);
  }
});

test('administrator setup rejects incomplete branch listings before writing', () => {
  const api = (method, _body, target) => {
    assert.equal(method, 'GET');
    if (target.endsWith('/github-pages')) return { deployment_branch_policy: { custom_branch_policies: true } };
    return { total_count: 101, branch_policies: [] };
  };
  assert.throws(() => configurePublicationEnvironment(api), /Incomplete deployment branch rules/);
});

test('administrator setup requires read-back of main and every existing rule', () => {
  for (const actual of [[], [{ id: 2, name: 'main', type: 'branch' }], [{ id: 1, name: 'gh-pages', type: 'branch' }]]) {
    let written = false;
    const api = (method, _body, target) => {
      if (method === 'POST') { written = true; return {}; }
      if (target.endsWith('/github-pages')) return { deployment_branch_policy: { custom_branch_policies: true } };
      const rules = written ? actual : [{ id: 1, name: 'gh-pages', type: 'branch' }];
      return { total_count: rules.length, branch_policies: rules };
    };
    assert.throws(() => configurePublicationEnvironment(api), /read-back differs/);
  }
});
