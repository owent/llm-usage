import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import test from 'node:test';
import { reportBundles } from './bundle-report.mjs';

async function workspace(t) {
  const directory = await mkdtemp(join(tmpdir(), 'llm-usage-bundle-test-'));
  t.after(() => rm(directory, { recursive: true, force: true }));
  return directory;
}

test('missing and empty bundle directories fail', async (t) => {
  const root = await workspace(t);
  await assert.rejects(reportBundles(join(root, 'missing'), 'rev'), /ENOENT/);
  await assert.rejects(reportBundles(root, 'rev'), /No release bundles/);
});

test('report records revision, bytes and SHA-256 for each installer', async (t) => {
  const root = await workspace(t);
  await mkdir(join(root, 'nsis'));
  await writeFile(join(root, 'nsis', 'test setup.exe'), 'installer');
  const report = await reportBundles(root, 'test-revision');
  assert.deepEqual(report, {
    revision: 'test-revision',
    bundles: [{ path: 'nsis/test setup.exe', bytes: 9, sha256: createHash('sha256').update('installer').digest('hex') }],
  });
  assert.deepEqual(JSON.parse(await readFile(join(root, 'bundle-report.json'))), report);
});

test('macOS app directory becomes a nonempty archive and repeated reports do not duplicate it', async (t) => {
  const root = await workspace(t);
  const app = join(root, 'macos', 'Test App.app');
  await mkdir(join(app, 'Contents', 'MacOS'), { recursive: true });
  await writeFile(join(app, 'Contents', 'MacOS', 'test'), 'executable');
  await reportBundles(root, 'first');
  const report = await reportBundles(root, 'second');
  assert.equal(report.bundles.length, 1);
  assert.equal(report.bundles[0].path, 'macos/Test App.app.tar.gz');
  assert.ok(report.bundles[0].bytes > 0);
  const contents = spawnSync('tar', ['-tzf', report.bundles[0].path], { cwd: root, encoding: 'utf8', windowsHide: true });
  assert.equal(contents.status, 0, contents.stderr);
  assert.match(contents.stdout, /Test App\.app\/Contents\/MacOS\/test/);
});
