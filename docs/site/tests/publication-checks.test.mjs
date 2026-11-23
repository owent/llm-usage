import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { randomUUID } from 'node:crypto';
import { root } from '../scripts/content.mjs';
import { outputDigests, verifyPublication, validationFile, requiredChecks, waitForPagesBuild } from '../scripts/publication-checks.mjs';

test('publication rejects unvalidated, stale, changed or incomplete artifacts', async () => {
  const directory = resolve(root, 'build/documentation-site/unit-publication', randomUUID());
  await mkdir(directory, { recursive: true });
  await writeFile(resolve(directory, 'index.html'), '<html lang="en">Example</html>');
  const revision = 'a'.repeat(40);
  await assert.rejects(verifyPublication(directory, revision), { code: 'ENOENT' });
  const evidence = { schema: 1, sourceRevision: revision, checks: requiredChecks, files: await outputDigests(directory) };
  const stamp = async value => writeFile(resolve(directory, validationFile), JSON.stringify(value));
  await stamp(evidence);
  await verifyPublication(directory, revision);
  await assert.rejects(verifyPublication(directory, 'b'.repeat(40)), /matching successful-check evidence/);
  await stamp({ ...evidence, checks: ['content', 'unit', 'build'] });
  await assert.rejects(verifyPublication(directory, revision), /matching successful-check evidence/);
  await stamp(evidence);
  await writeFile(resolve(directory, 'index.html'), '<html lang="en">Changed</html>');
  await assert.rejects(verifyPublication(directory, revision), /changed after validation/);
  await writeFile(resolve(directory, 'index.html'), '<html lang="en">Example</html>');
  await writeFile(resolve(directory, 'unexpected.js'), 'console.log("unexpected")');
  await assert.rejects(verifyPublication(directory, revision), /changed after validation/);
});

test('publication waits for a successful Pages build of the same revision after a cancelled build', async () => {
  const commit = 'a'.repeat(40);
  const failed = { commit, status: 'errored', error: { message: 'Page build failed.' } };
  const other = { commit: 'b'.repeat(40), status: 'built' };
  const snapshots = [
    [failed, other],
    [failed, { commit, status: 'building' }, other],
    [failed, { commit, status: 'built' }, other],
  ];
  await waitForPagesBuild(commit, () => {
    assert.ok(snapshots.length);
    return snapshots.shift();
  }, { timeoutMs: 1_000, pollIntervalMs: 1 });
  assert.equal(snapshots.length, 0);
});

test('publication reports the Pages error when no build of that revision completes', async () => {
  const commit = 'a'.repeat(40);
  const builds = [
    { commit: 'b'.repeat(40), status: 'built' },
    { commit, status: 'errored', error: { message: 'asset build failed' } },
  ];
  await assert.rejects(waitForPagesBuild(commit, () => builds, {
    timeoutMs: 30, pollIntervalMs: 5,
  }), /Pages build failed for a{40}: asset build failed/);
});
