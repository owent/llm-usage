import { spawnSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const desktop = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const root = resolve(desktop, '..');
const mode = process.argv[2];
if (mode === '--help') {
  console.log('Usage: node desktop/scripts/assets.mjs --generate | --check\nRequires the locked desktop npm dependencies and Git LFS. No network access.\n--generate replaces derived assets; --check renders temporary files and compares bytes.');
  process.exit(0);
}
if (!['--generate', '--check'].includes(mode) || process.argv.length !== 3) {
  console.error('Expected --generate or --check; use --help for details.');
  process.exit(1);
}

function run(command, args, input) {
  const result = spawnSync(command, args, { cwd: root, input, encoding: 'utf8', timeout: 120_000, windowsHide: true });
  if (result.error || result.status !== 0) {
    throw new Error(`${command} failed: ${result.error?.message ?? result.stderr ?? result.stdout}`);
  }
  return result.stdout;
}

function files(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    return entry.isDirectory() ? files(path) : [path];
  }).sort();
}

function content(path) {
  const bytes = readFileSync(path);
  if (bytes.subarray(0, 80).toString().startsWith('version https://git-lfs.github.com/spec/v1')) {
    throw new Error(`LFS pointer has not been downloaded: ${relative(root, path)}; run git lfs pull`);
  }
  return bytes;
}

function checkPng(path) {
  const bytes = content(path);
  if (bytes.subarray(0, 8).toString('hex') !== '89504e470d0a1a0a' || bytes.toString('ascii', 12, 16) !== 'IHDR') {
    throw new Error(`Invalid PNG: ${path}`);
  }
  const width = bytes.readUInt32BE(16);
  const height = bytes.readUInt32BE(20);
  if (width !== height || width < 16 || bytes[24] !== 8 || bytes[25] !== 6) {
    throw new Error(`Expected square RGBA8 PNG: ${path}`);
  }
  return width;
}

function icnsBlocks(icns) {
  if (icns.length < 8 || icns.toString('ascii', 0, 4) !== 'icns' || icns.readUInt32BE(4) !== icns.length) throw new Error('Invalid ICNS header');
  const blocks = [];
  let cursor = 8;
  while (cursor < icns.length) {
    if (cursor + 8 > icns.length) throw new Error('Truncated ICNS block');
    const length = icns.readUInt32BE(cursor + 4);
    if (length < 8 || cursor + length > icns.length) throw new Error('Invalid ICNS block');
    blocks.push(icns.subarray(cursor, cursor + length));
    cursor += length;
  }
  return blocks;
}

function normalizeIcns(path) {
  // CLI 2.11.5 emits the same layer bytes in a varying order. Keep legacy
  // RGB/mask pairs together, then order the named PNG layers deterministically.
  const bytes = content(path);
  const order = ['is32', 's8mk', 'il32', 'l8mk', 'ih32', 'h8mk', 'it32', 't8mk', 'ic07', 'ic08', 'ic09', 'ic10', 'ic11', 'ic12', 'ic13', 'ic14'];
  const blocks = icnsBlocks(bytes);
  const rank = (block) => {
    const value = order.indexOf(block.toString('ascii', 0, 4));
    if (value < 0) throw new Error('Unexpected ICNS layer; review the pinned CLI before updating assets');
    return value;
  };
  blocks.sort((a, b) => rank(a) - rank(b));
  writeFileSync(path, Buffer.concat([bytes.subarray(0, 8), ...blocks]));
}

function checkContainers() {
  const ico = content(join(desktop, 'src-tauri/icons/icon.ico'));
  if (ico.readUInt32LE(0) !== 0x00010000) throw new Error('Invalid ICO header');
  const sizes = new Set();
  for (let i = 0; i < ico.readUInt16LE(4); i++) {
    const offset = 6 + i * 16;
    const width = ico[offset] || 256;
    if (width !== (ico[offset + 1] || 256) || ico.readUInt32LE(offset + 12) + ico.readUInt32LE(offset + 8) > ico.length) {
      throw new Error('Invalid ICO directory');
    }
    sizes.add(width);
  }
  for (const size of [16, 24, 32, 48, 64, 256]) {
    if (!sizes.has(size)) throw new Error(`Missing ${size}px ICO layer`);
  }
  const icns = content(join(desktop, 'src-tauri/icons/icon.icns'));
  const types = new Set(icnsBlocks(icns).map((block) => block.toString('ascii', 0, 4)));
  for (const type of ['is32', 's8mk', 'il32', 'l8mk', 'ic07', 'ic08', 'ic09', 'ic10', 'ic11', 'ic12', 'ic13', 'ic14']) {
    if (!types.has(type)) throw new Error(`Missing ICNS layer: ${type}`);
  }
}

const tempParent = join(root, 'build');
mkdirSync(tempParent, { recursive: true });
const temp = mkdtempSync(join(tempParent, 'assets-'));
try {
  const source = join(desktop, 'assets/app-icon.svg');
  const cli = join(desktop, 'node_modules/@tauri-apps/cli/tauri.js');
  const inputPaths = [source, ...['dark', 'light'].map((theme) => join(desktop, `assets/tray-${theme}.svg`))];
  for (const path of inputPaths) content(path);
  const render = (input, output, sizes = []) => run(process.execPath, [cli, 'icon', input, '--output', output, ...sizes.flatMap((size) => ['--png', String(size)])]);
  render(source, join(temp, 'icons'));
  normalizeIcns(join(temp, 'icons/icon.icns'));
  render(source, join(temp, 'brand'), [256, 512, 1024]);
  for (const theme of ['dark', 'light']) {
    render(join(desktop, `assets/tray-${theme}.svg`), join(temp, theme), [16, 20, 24, 32, 48]);
  }

  const outputs = files(join(temp, 'icons')).map((path) => [path, join(desktop, 'src-tauri/icons', relative(join(temp, 'icons'), path))]);
  for (const size of [256, 512, 1024]) outputs.push([join(temp, `brand/${size}x${size}.png`), join(desktop, `public/brand/app-icon-${size}.png`)]);
  for (const theme of ['dark', 'light']) {
    for (const size of [16, 20, 24, 32, 48]) outputs.push([join(temp, `${theme}/${size}x${size}.png`), join(desktop, `src-tauri/icons/tray/${theme}-${size}.png`)]);
  }
  outputs.push([source, join(desktop, 'public/brand/app-icon.svg')]);
  outputs.push([join(temp, 'icons/icon.ico'), join(desktop, 'public/favicon.ico')]);
  for (const [from, to] of outputs) {
    if (mode === '--generate') {
      mkdirSync(dirname(to), { recursive: true });
      copyFileSync(from, to);
    } else if (!existsSync(to) || !content(from).equals(content(to))) {
      throw new Error(`Missing or stale asset: ${relative(root, to)}; run npm run assets:generate`);
    }
  }

  const assets = [...inputPaths, ...files(join(desktop, 'public')), ...files(join(desktop, 'src-tauri/icons'))];
  for (const path of assets) {
    const bytes = content(path);
    if (path.endsWith('.png')) checkPng(path);
    if (path.endsWith('.svg') && (!bytes.toString().includes('<svg ') || /<script|<foreignObject|(?:href|src)\s*=\s*["'](?:https?:|\/\/)/i.test(bytes.toString()))) {
      throw new Error(`Expected self-contained SVG: ${relative(root, path)}`);
    }
  }
  for (const size of [256, 512, 1024]) {
    if (checkPng(join(desktop, `public/brand/app-icon-${size}.png`)) !== size) throw new Error('Incorrect brand PNG size');
  }
  for (const [name, size] of [['32x32.png', 32], ['128x128.png', 128], ['128x128@2x.png', 256]]) {
    if (checkPng(join(desktop, 'src-tauri/icons', name)) !== size) throw new Error(`Incorrect Tauri PNG size: ${name}`);
  }
  checkContainers();
  const config = JSON.parse(readFileSync(join(desktop, 'src-tauri/tauri.conf.json'), 'utf8'));
  for (const path of config.bundle.icon) content(join(desktop, 'src-tauri', path));
  const names = assets.map((path) => relative(root, path).split(sep).join('/'));
  const attrs = run('git', ['check-attr', '--stdin', '-z', 'filter'], names.join('\0') + '\0').split('\0');
  for (let i = 0; i < attrs.length - 1; i += 3) {
    if (attrs[i + 2] !== 'lfs') throw new Error(`Asset is not covered by LFS: ${attrs[i]}`);
  }
  console.log(`${mode === '--generate' ? 'Generated' : 'Verified'} ${outputs.length} derived assets; checked ${assets.length} files, formats and LFS attributes.`);
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
} finally {
  if (resolve(temp).startsWith(resolve(tempParent) + sep) && relative(tempParent, temp).startsWith('assets-')) {
    rmSync(temp, { recursive: true, force: true });
  }
}
