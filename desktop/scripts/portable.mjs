import { createHash } from 'node:crypto';
import { createReadStream } from 'node:fs';
import { chmod, copyFile, cp, lstat, mkdir, mkdtemp, readFile, readdir, readlink, realpath, rename, rm, writeFile } from 'node:fs/promises';
import { join, relative, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { reportBundles } from './bundle-report.mjs';

const repository = fileURLToPath(new URL('../../', import.meta.url));
const platforms = { win32: 'windows', linux: 'linux', darwin: 'macos' };
const compression = ['-19', '-T2', '--long=27'];

function run(command, args, cwd) {
  const result = spawnSync(command, args, { cwd, encoding: 'utf8', timeout: 600_000, windowsHide: true });
  if (result.error || result.status !== 0) throw new Error(`${command} failed: ${result.error?.message ?? result.stderr}`);
  return result.stdout;
}

export async function releaseVersion(root = repository) {
  const manifests = ['package.json', 'desktop/package.json', 'desktop/src-tauri/tauri.conf.json'];
  const versions = await Promise.all(manifests.map(async file => JSON.parse(await readFile(join(root, file))).version));
  for (const file of ['package-lock.json', 'desktop/package-lock.json']) {
    const lock = JSON.parse(await readFile(join(root, file)));
    versions.push(lock.version, lock.packages[''].version);
  }
  for (const file of ['desktop/src-tauri/Cargo.toml', 'desktop/src-tauri/crates/core/Cargo.toml']) {
    versions.push((await readFile(join(root, file), 'utf8')).match(/^version = "([^"]+)"/m)?.[1]);
  }
  const lock = await readFile(join(root, 'desktop/src-tauri/Cargo.lock'), 'utf8');
  for (const name of ['llm-usage-core', 'llm-usage-desktop']) {
    versions.push(lock.match(new RegExp(`name = "${name}"\\r?\\nversion = "([^"]+)"`))?.[1]);
  }
  if (!/^\d+\.\d+\.\d+$/.test(versions[0]) || versions.some(version => version !== versions[0])) {
    throw new Error(`Release versions differ: ${versions.join(', ')}`);
  }
  return versions[0];
}

export function binaryArchitecture(bytes, platform) {
  if (platform === 'windows' && bytes.subarray(0, 2).toString() === 'MZ') {
    const offset = bytes.readUInt32LE(0x3c);
    if (bytes.subarray(offset, offset + 4).toString() !== 'PE\0\0') throw new Error('Invalid PE signature');
    return { 0x8664: 'x64', 0xaa64: 'arm64' }[bytes.readUInt16LE(offset + 4)];
  }
  if (platform === 'linux' && bytes.subarray(0, 4).equals(Buffer.from([0x7f, 0x45, 0x4c, 0x46])) && bytes[4] === 2 && bytes[5] === 1) {
    return { 62: 'x64', 183: 'arm64' }[bytes.readUInt16LE(18)];
  }
  if (platform === 'macos' && bytes.readUInt32LE(0) === 0xfeedfacf) {
    return { 0x01000007: 'x64', 0x0100000c: 'arm64' }[bytes.readUInt32LE(4)];
  }
  throw new Error(`Invalid ${platform} executable`);
}

async function digest(file) {
  const hash = createHash('sha256');
  for await (const chunk of createReadStream(file)) hash.update(chunk);
  return hash.digest('hex');
}

async function tree(root) {
  const entries = [];
  async function visit(directory) {
    for (const name of (await readdir(directory)).sort()) {
      const path = join(directory, name);
      const info = await lstat(path);
      const entry = { path: relative(root, path).split('\\').join('/') };
      if (process.platform !== 'win32') entry.mode = info.mode & 0o777;
      if (info.isSymbolicLink()) {
        entry.link = await readlink(path);
        const target = resolve(directory, entry.link);
        if (relative(root, target).startsWith('..')) throw new Error(`Archive symlink escapes its directory: ${entry.path}`);
        if (relative(root, await realpath(path)).startsWith('..')) throw new Error(`Resolved archive symlink escapes its directory: ${entry.path}`);
      } else if (info.isDirectory()) {
        entry.directory = true;
      } else if (info.isFile()) {
        entry.bytes = info.size;
        entry.sha256 = await digest(path);
      } else throw new Error(`Unsupported archive member: ${entry.path}`);
      entries.push(entry);
      if (info.isDirectory()) await visit(path);
    }
  }
  await visit(root);
  return entries;
}

export async function archivePortable({ platform, arch, version, input, binary, output, smoke = false }) {
  if (!['windows', 'linux', 'macos'].includes(platform) || !['x64', 'arm64'].includes(arch) || !/^\d+\.\d+\.\d+$/.test(version)) {
    throw new Error('Invalid portable platform, architecture or version');
  }
  if (binaryArchitecture(await readFile(binary), platform) !== arch) throw new Error('Executable architecture mismatch');
  const binaryHash = await digest(binary);
  const base = join(repository, 'build', 'portable-packaging');
  await mkdir(base, { recursive: true });
  const work = await mkdtemp(join(base, `${platform}-${arch}-`));
  const name = `LLMUsage-${version}-${platform}-${arch}-portable`;
  const stage = join(work, 'stage', name);
  const unpack = join(work, 'unpacked');
  let entrypoint;
  try {
    await mkdir(stage, { recursive: true });
    if (platform === 'windows') {
      await copyFile(input, join(stage, 'LLMUsage.exe'));
      entrypoint = 'LLMUsage.exe';
    } else if (platform === 'macos') {
      await cp(input, join(stage, 'LLMUsage.app'), { recursive: true, verbatimSymlinks: true });
      entrypoint = 'LLMUsage.app/Contents/MacOS/LLMUsage';
    } else {
      await cp(input, stage, { recursive: true, verbatimSymlinks: true });
      entrypoint = 'AppRun';
    }
    await lstat(join(stage, entrypoint));
    const instructions = await readFile(join(repository, 'desktop', 'portable', `${platform}.txt`));
    await writeFile(join(stage, 'README.txt'), instructions);
    const native = (await tree(stage)).find(entry => entry.sha256 === binaryHash);
    if (!native) throw new Error('Staging omitted the native executable');
    const managed = [...await readdir(stage), 'llmusage-package.json'].sort();
    await writeFile(join(stage, 'llmusage-package.json'), JSON.stringify({
      schema: 1, kind: 'portable', platform, arch, version,
      executable: native.path, entrypoint, managed,
    }, null, 2) + '\n');
    const expected = await tree(stage);
    if (!expected.some(entry => entry.sha256 === binaryHash)) throw new Error('Staging omitted the native executable');
    // Materialize tar before compression; an external tar/zstd pipe can stall on large Windows archives.
    run('tar', ['-cf', '../portable.tar', name], join(work, 'stage'));
    run('zstd', [...compression, '-f', 'portable.tar', '-o', 'portable.tar.zst'], work);
    run('zstd', ['-t', 'portable.tar.zst'], work);
    run('zstd', ['-d', '-f', 'portable.tar.zst', '-o', 'verified.tar'], work);
    await mkdir(unpack);
    // Restore Unix modes explicitly; tar otherwise applies the extracting user's umask.
    run('tar', [process.platform === 'win32' ? '-xf' : '-xpf', '../verified.tar'], unpack);
    const actual = await tree(join(unpack, name));
    if (JSON.stringify(actual) !== JSON.stringify(expected)) {
      const difference = expected.find((entry, index) => JSON.stringify(entry) !== JSON.stringify(actual[index]));
      const extracted = actual.find(entry => entry.path === difference?.path);
      throw new Error(`Extracted archive contents or modes differ: ${JSON.stringify({ expected: difference, actual: extracted, counts: [expected.length, actual.length] })}`);
    }
    if (smoke) {
      // Run from the extraction: Linux AppRun supplies bundled library paths without FUSE.
      run(process.execPath, [join(repository, 'desktop/tests/headless-smoke.mjs'), '--exe', join(unpack, name, entrypoint)], repository);
    }
    const gui = smoke && platform === 'linux' && process.env.PORTABLE_GUI_CHECK === '1';
    if (gui) run(process.execPath, [join(repository, 'desktop/tests/portable-linux-gui.mjs'), join(unpack, name, entrypoint)], repository);
    await mkdir(output, { recursive: true });
    const archive = join(resolve(output), `${name}.tar.zst`);
    await copyFile(join(work, 'portable.tar.zst'), `${archive}.tmp`);
    await rename(`${archive}.tmp`, archive);
    return { archive, portable: { platform, arch, version, entrypoint, compression,
      verified_files: actual.filter(entry => entry.sha256).length, extracted_verified: true, headless_verified: smoke, gui_verified: gui } };
  } finally {
    await rm(work, { recursive: true, force: true });
  }
}

async function main() {
  const [releaseDirectory, output, expectedPlatform, expectedArch] = process.argv.slice(2);
  const platform = platforms[process.platform];
  const arch = process.arch;
  if (!output || platform !== expectedPlatform || arch !== expectedArch) {
    throw new Error('Usage: node desktop/scripts/portable.mjs <release-directory> <output-directory> <native-platform> <native-arch>');
  }
  const version = await releaseVersion();
  if (process.env.GITHUB_REF_TYPE === 'tag' && process.env.GITHUB_REF_NAME !== `v${version}`) throw new Error('Release tag does not match package version');
  const release = resolve(releaseDirectory);
  let binary = platform === 'macos' ? join(release, 'bundle/macos/LLMUsage.app/Contents/MacOS/LLMUsage')
    : join(release, `LLMUsage${platform === 'windows' ? '.exe' : ''}`);
  let input = platform === 'macos' ? join(release, 'bundle/macos/LLMUsage.app') : binary;
  let extraction;
  try {
    if (platform === 'linux') {
      const images = (await readdir(join(release, 'bundle/appimage'))).filter(name => name.endsWith('.AppImage'));
      if (images.length !== 1) throw new Error('Expected exactly one native AppImage');
      const base = join(repository, 'build/portable-packaging');
      await mkdir(base, { recursive: true });
      extraction = await mkdtemp(join(base, 'appimage-'));
      const image = join(release, 'bundle/appimage', images[0]);
      await chmod(image, 0o755);
      run(image, ['--appimage-extract'], extraction);
      input = join(extraction, 'squashfs-root');
      // linuxdeploy may adjust ELF library paths; validate the bundled executable itself.
      binary = join(input, 'usr/bin/LLMUsage');
    }
    const result = await archivePortable({ platform, arch, version, input, binary, output, smoke: true });
    if (platform === 'windows' && arch === 'x64') {
      const directory = join(release, 'bundle/nsis');
      const installers = (await readdir(directory)).filter(name => name === `LLMUsage_${version}_x64-setup.exe`);
      if (installers.length !== 1) throw new Error('Expected the Windows x64 NSIS installer');
      await copyFile(join(directory, installers[0]), join(output, installers[0]));
    }
    console.log(JSON.stringify(await reportBundles(output, process.env.GITHUB_SHA ?? 'local', { portable: result.portable }), null, 2));
  } finally {
    if (extraction) await rm(extraction, { recursive: true, force: true });
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  main().catch(error => { console.error(error.message); process.exitCode = 1; });
}
