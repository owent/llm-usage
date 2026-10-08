import assert from 'node:assert/strict';
import { chmod, mkdir, mkdtemp, readFile, readdir, rm, symlink, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import test from 'node:test';
import { archivePortable, binaryArchitecture, releaseVersion } from './portable.mjs';

async function workspace(t) {
  const base = fileURLToPath(new URL('../../build/portable-tests/', import.meta.url));
  await mkdir(base, { recursive: true });
  const root = await mkdtemp(join(base, 'case-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  return root;
}

function binary(platform, arch) {
  const bytes = Buffer.alloc(256);
  if (platform === 'windows') {
    bytes.write('MZ'); bytes.writeUInt32LE(128, 0x3c); bytes.write('PE\0\0', 128);
    bytes.writeUInt16LE(arch === 'x64' ? 0x8664 : 0xaa64, 132);
  } else if (platform === 'linux') {
    bytes.set([0x7f, 0x45, 0x4c, 0x46, 2, 1]); bytes.writeUInt16LE(arch === 'x64' ? 62 : 183, 18);
  } else {
    bytes.writeUInt32LE(0xfeedfacf); bytes.writeUInt32LE(arch === 'x64' ? 0x01000007 : 0x0100000c, 4);
  }
  return bytes;
}

async function fixture(root, platform, arch) {
  const input = join(root, platform === 'windows' ? 'LLMUsage.exe' : 'input');
  const executable = platform === 'windows' ? input : join(input, platform === 'linux' ? 'usr/bin/LLMUsage' : 'Contents/MacOS/LLMUsage');
  await mkdir(join(executable, '..'), { recursive: true });
  await writeFile(executable, binary(platform, arch));
  await chmod(executable, 0o755);
  if (platform === 'linux') {
    await writeFile(join(input, 'AppRun'), '#!/bin/sh\nexec "$(dirname "$0")/usr/bin/LLMUsage" "$@"\n');
    await chmod(join(input, 'AppRun'), 0o775);
    await writeFile(join(input, '.hidden-resource'), 'retain-hidden-files');
    await chmod(join(input, '.hidden-resource'), 0o664);
    await chmod(join(input, 'usr'), 0o775);
    if (process.platform !== 'win32') await symlink('usr/bin/LLMUsage', join(input, 'native-link'));
  }
  return { input, binary: executable };
}

for (const platform of ['windows', 'linux', 'macos']) {
  for (const arch of ['x64', 'arm64']) {
    test(`${platform}/${arch} real zstd archive round trip preserves layout and overwrite`, async t => {
      const root = await workspace(t);
      const source = await fixture(root, platform, arch);
      const options = { platform, arch, version: '0.2.2', output: join(root, 'output'), ...source };
      const first = await archivePortable(options);
      assert.equal(first.portable.extracted_verified, true);
      assert.equal(first.portable.headless_verified, false);
      const initial = await readFile(first.archive);
      await writeFile(source.binary, Buffer.concat([binary(platform, arch), Buffer.from('updated executable')]));
      const repeated = await archivePortable(options);
      assert.notDeepEqual(await readFile(repeated.archive), initial);
      assert.deepEqual(await readdir(options.output), [`LLMUsage-0.2.2-${platform}-${arch}-portable.tar.zst`]);
      const check = spawnSync('zstd', ['-t', repeated.archive], { encoding: 'utf8', windowsHide: true });
      assert.equal(check.status, 0, check.stderr);
    });
  }
}

test('wrong architecture and incomplete inputs fail before replacing an archive', async t => {
  const root = await workspace(t);
  const source = await fixture(root, 'windows', 'x64');
  const options = { platform: 'windows', arch: 'x64', version: '0.2.2', output: join(root, 'output'), ...source };
  const result = await archivePortable(options);
  const original = await readFile(result.archive);
  await assert.rejects(archivePortable({ ...options, arch: 'arm64' }), /architecture mismatch/);
  await assert.rejects(archivePortable({ ...options, input: join(root, 'missing') }), /ENOENT/);
  assert.deepEqual(await readFile(result.archive), original);
  assert.throws(() => binaryArchitecture(Buffer.alloc(256), 'linux'), /Invalid linux executable/);
});

test('version check reads both manifests and locks and rejects a stale Rust core version', async t => {
  const root = await workspace(t);
  const sourceRoot = fileURLToPath(new URL('../../', import.meta.url));
  const paths = ['package.json', 'desktop/package.json', 'desktop/src-tauri/tauri.conf.json', 'package-lock.json',
    'desktop/package-lock.json', 'desktop/src-tauri/Cargo.toml', 'desktop/src-tauri/crates/core/Cargo.toml', 'desktop/src-tauri/Cargo.lock'];
  for (const path of paths) {
    await mkdir(join(root, path, '..'), { recursive: true });
    await writeFile(join(root, path), await readFile(join(sourceRoot, path)));
  }
  const version = await releaseVersion(root);
  assert.match(version, /^\d+\.\d+\.\d+$/);
  const core = join(root, 'desktop/src-tauri/crates/core/Cargo.toml');
  await writeFile(core, (await readFile(core, 'utf8')).replace(`version = "${version}"`, 'version = "0.0.0"'));
  await assert.rejects(releaseVersion(root), /Release versions differ/);
});
