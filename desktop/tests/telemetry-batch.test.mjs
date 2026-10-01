import test from 'node:test';
import assert from 'node:assert/strict';
import { configureTelemetryBatch } from '../src/lib/telemetry-batch.ts';

const target = (id, status = 'missing', configurable = status === 'missing') => ({
  id, name: id, status, configurable, reason: '', kind: 'jsonc',
  config_path: '/local/settings.json', output_path: '/local/events.jsonl', docs_url: '',
});
const preview = (row, token) => ({ token, target: row, changes: [], keys: [], receiver: false });

test('batch prepares each target after prior shared settings changes and preserves existing outputs', async () => {
  const rows = [target('profile-a'), target('existing', 'configured'), target('profile-b'), target('policy', 'blocked', false)];
  let revision = 0;
  const calls = [];
  const snapshots = [];
  const results = await configureTelemetryBatch({
    check: async () => rows,
    preview: async (id) => {
      calls.push('preview:' + id);
      return preview(rows.find((r) => r.id === id), String(revision));
    },
    apply: async (token) => {
      calls.push('apply:' + token);
      assert.equal(Number(token), revision, 'next plan must observe previous writes');
      revision++;
    },
  }, (progress) => snapshots.push(progress));
  assert.deepEqual(calls, ['preview:profile-a', 'apply:0', 'preview:profile-b', 'apply:1']);
  assert.deepEqual(results.map((r) => [r.target.id, r.status]), [
    ['policy', 'manual'], ['profile-a', 'applied'], ['profile-b', 'applied'],
  ]);
  assert.equal(snapshots[0].results.length, 1, 'progress snapshots do not mutate later');
  assert.equal(snapshots.at(-1).completed, 2);
  assert.equal(snapshots.at(-1).total, 2);
});

test('a failed preview or apply does not prevent later targets and never exposes raw errors', async () => {
  const rows = [target('failed-preview'), target('failed-apply'), target('success')];
  const applied = [];
  const results = await configureTelemetryBatch({
    check: async () => rows,
    preview: async (id) => {
      if (id === 'failed-preview') throw 'config_changed';
      return preview(rows.find((r) => r.id === id), id);
    },
    apply: async (token) => {
      if (token === 'failed-apply') throw new Error('opaque OS/config text');
      applied.push(token);
    },
  });
  assert.deepEqual(applied, ['success']);
  assert.equal(results[0].errorKey, 'telemetry.reason.config_changed');
  assert.equal(results[1].errorKey, 'telemetry.writeFailed');
  assert.equal(results[2].status, 'applied');
  assert.ok(!JSON.stringify(results).includes('opaque'));
});

test('batch uses the fresh check and makes no writes when nothing is eligible or discovery fails', async () => {
  let writes = 0;
  const operations = {
    check: async () => [target('already-configured', 'configured'), target('manual', 'blocked', false)],
    preview: async () => { writes++; throw new Error('unexpected preview'); },
    apply: async () => { writes++; },
  };
  const results = await configureTelemetryBatch(operations);
  assert.deepEqual(results.map((r) => r.status), ['manual']);
  assert.equal(writes, 0);
  operations.check = async () => { throw new Error('discovery failed'); };
  await assert.rejects(configureTelemetryBatch(operations), /discovery failed/);
  assert.equal(writes, 0);
});
