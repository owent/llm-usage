import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { reportBundles } from './bundle-report.mjs';

async function workspace(t) {
  const base = fileURLToPath(new URL('../../build/bundle-report-tests/', import.meta.url));
  await mkdir(base, { recursive: true });
  const directory = await mkdtemp(join(base, 'case-'));
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
  await writeFile(join(root, 'nsis', 'test-setup.exe'), 'installer');
  const report = await reportBundles(root, 'test-revision');
  assert.deepEqual(report, {
    revision: 'test-revision',
    bundles: [{ path: 'nsis/test-setup.exe', bytes: 9, sha256: createHash('sha256').update('installer').digest('hex') }],
  });
  assert.deepEqual(JSON.parse(await readFile(join(root, 'bundle-report.json'))), report);
});

test('validated portable archives are reported without duplicating or archiving app staging', async (t) => {
  const root = await workspace(t);
  const app = join(root, 'macos', 'Test App.app');
  await mkdir(join(app, 'Contents', 'MacOS'), { recursive: true });
  await writeFile(join(app, 'Contents', 'MacOS', 'test'), 'executable');
  await writeFile(join(root, 'Test-macos-arm64-portable.tar.zst'), 'validated-archive');
  await reportBundles(root, 'first');
  const report = await reportBundles(root, 'second');
  assert.equal(report.bundles.length, 1);
  assert.equal(report.bundles[0].path, 'Test-macos-arm64-portable.tar.zst');
  assert.ok(report.bundles[0].bytes > 0);
});

test('release report excludes Debian, raw AppImage, old gzip archives and temporary files', async (t) => {
  const root = await workspace(t);
  const staging = join(root, 'deb', 'Test_0.2.1_amd64');
  await mkdir(staging, { recursive: true });
  await mkdir(join(root, 'appimage'));
  await writeFile(join(root, 'deb', 'Test_0.2.1_amd64.deb'), 'deb-installer');
  await writeFile(join(root, 'appimage', 'Test_0.2.1_amd64.AppImage'), 'appimage-installer');
  for (const file of ['control.tar.gz', 'data.tar.gz']) await writeFile(join(staging, file), 'staging');
  await writeFile(join(root, 'Test-linux-x64-portable.tar.zst'), 'validated-portable');
  await writeFile(join(root, 'Test-linux-x64-portable.tar.zst.tmp'), 'incomplete');
  await writeFile(join(root, 'Test.app.tar.gz'), 'old-format');
  const report = await reportBundles(root, 'linux-revision');
  assert.deepEqual(report.bundles.map(item => item.path), ['Test-linux-x64-portable.tar.zst']);
});
