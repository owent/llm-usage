import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { once } from 'node:events';
import { copyFile, mkdir, readFile, unlink, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { spawn } from 'node:child_process';
import { DatabaseSync } from 'node:sqlite';
import { chromium } from 'playwright';
import { archivePortable, binaryArchitecture } from '../scripts/portable.mjs';
import { spawnIsolated } from './isolated-child.mjs';
import { freeHttpPort } from './http-test-port.mjs';

const args = process.argv.slice(2);
if (args.includes('--help')) {
  console.log('Usage: npm run test:update:windows -- [--exe PATH]\nTests real Windows update IPC, helper replacement and restart with an isolated synthetic portable archive in build/application-updates. Does not change installed applications or publish a release.');
  process.exit(0);
}
assert.equal(process.platform, 'win32');
const option = (key, fallback) => args.includes(key) ? args[args.indexOf(key) + 1] : fallback;
const original = resolve(option('--exe', 'desktop/src-tauri/target/release/LLMUsage.exe'));
const version = JSON.parse(await readFile(resolve('desktop/package.json'), 'utf8')).version;
const nextVersion = version.split('.').map((v, i) => i === 2 ? Number(v) + 1 : v).join('.');
const arch = binaryArchitecture(await readFile(original), 'windows');
const root = resolve('build/application-updates/native', `${Date.now()}-${process.pid}`);
const packageRoot = join(root, '含空格 portable');
const data = join(root, '含空格 data');
const source = join(root, 'source');
const temp = join(root, 'temp');
const exe = join(packageRoot, 'LLMUsage.exe');
await Promise.all([packageRoot, data, source, temp].map(path => mkdir(path, { recursive: true })));
await copyFile(original, exe);
const marker = { schema: 1, kind: 'portable', platform: 'windows', arch, version,
  executable: 'LLMUsage.exe', entrypoint: 'LLMUsage.exe', managed: ['LLMUsage.exe', 'README.txt', 'llmusage-package.json'] };
await writeFile(join(packageRoot, 'llmusage-package.json'), JSON.stringify(marker));
await writeFile(join(packageRoot, 'README.txt'), 'old package instructions');
await writeFile(join(packageRoot, 'personal.txt'), 'keep this unrelated file');
const env = Object.fromEntries(['PATH', 'SystemRoot', 'WINDIR', 'COMPUTERNAME', 'USERNAME', 'USERDOMAIN', 'ProgramFiles', 'ProgramFiles(x86)']
  .filter(key => process.env[key]).map(key => [key, process.env[key]]));
Object.assign(env, { CODEX_HOME: join(source, '.codex'), TEMP: temp, TMP: temp,
  APPDATA: join(source, 'AppData/Roaming'), LOCALAPPDATA: join(source, 'AppData/Local') });
const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const operations = [];
let child, browser, page, restoredApplication = false;

async function run(binary, arguments_, options = {}) {
  const process_ = spawnIsolated(binary, arguments_, { env, windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'], ...options });
  let output = '';
  process_.stdout.on('data', chunk => output += String(chunk));
  process_.stderr.on('data', chunk => output += String(chunk));
  const timer = setTimeout(() => process_.kill(), 60_000);
  try {
    const [code] = await once(process_, 'exit');
    assert.equal(code, 0, output);
    return output;
  } finally { clearTimeout(timer); }
}

async function launch() {
  const port = await freeHttpPort();
  child = spawnIsolated(exe, ['--data-dir', data], { env: { ...env,
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port}`,
    WEBVIEW2_USER_DATA_FOLDER: join(root, 'webview') }, windowsHide: true, stdio: 'ignore' });
  await attach(port);
}

async function attach(port) {
  let endpoint;
  for (let i = 0; i < 300; i++) {
    try { endpoint = await (await fetch(`http://127.0.0.1:${port}/json/version`)).json(); } catch {}
    if (endpoint?.webSocketDebuggerUrl) break;
    await pause(100);
  }
  assert.ok(endpoint?.webSocketDebuggerUrl, 'native WebView2 endpoint');
  browser = await chromium.connectOverCDP(endpoint.webSocketDebuggerUrl);
  for (let i = 0; i < 100; i++) {
    page = browser.contexts().flatMap(context => context.pages()).find(page => /tauri\.localhost/.test(page.url()));
    if (page) break;
    await pause(100);
  }
  assert.ok(page);
  await page.waitForFunction(() => Boolean(window.__TAURI_INTERNALS__?.invoke));
}

async function ipc(cmd, args = {}) {
  return page.evaluate(({ cmd, args }) => window.__TAURI_INTERNALS__.invoke(cmd, args), { cmd, args });
}

async function waitStatus(phase) {
  for (let i = 0; i < 300; i++) {
    const value = await ipc('update_status');
    if (value.phase === 'error') throw new Error(value.error);
    if (value.phase === phase) return value;
    await pause(100);
  }
  throw new Error(`Update status did not reach ${phase}`);
}

async function close() {
  await browser?.close(); browser = undefined;
  if (child?.exitCode === null) { child.kill(); await once(child, 'exit'); }
  child = undefined; page = undefined;
  if (restoredApplication) {
    await run('pwsh.exe', ['-NoProfile', '-NonInteractive', '-Command', '$ownedPath=$env:LLM_USAGE_UPDATER_TEST_EXE; Get-CimInstance Win32_Process -Filter "Name=\'LLMUsage.exe\'" | Where-Object { $_.ExecutablePath -eq $ownedPath } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction Stop }'], { env: { ...env, LLM_USAGE_UPDATER_TEST_EXE: exe } });
    restoredApplication = false;
  }
}

try {
  await run(exe, ['--scan-once', '--data-dir', data]);
  const connection = new DatabaseSync(join(data, 'llm-usage.sqlite'));
  const settings = JSON.parse(connection.prepare("SELECT value FROM settings WHERE key='app_settings'").get()?.value ?? '{}');
  Object.assign(settings, { timezone: 'Asia/Shanghai', week_start: null, language: 'en', theme: 'light', manual_roots: [],
    manual_roots_only: true, hostname_alias: null, refresh_interval_secs: 0, close_to_tray: false,
    updates: { schedule: 'manual', auto_download: false } });
  connection.prepare("INSERT OR REPLACE INTO settings(key,value,schema_version,updated_at_ms) VALUES('app_settings',?,1,0)").run(JSON.stringify(settings));
  connection.exec("CREATE TABLE update_acceptance(value TEXT); INSERT INTO update_acceptance VALUES('retained usage sentinel')");
  connection.close();
  await launch();
  assert.equal((await ipc('update_status')).package_kind, 'portable');
  await ipc('check_update');
  const checked = await waitStatus('up_to_date');
  assert.ok(checked.last_checked_ms);
  assert.equal(checked.current_version, version);
  const saved = await ipc('get_settings');
  assert.deepEqual(saved.updates, { schedule: 'manual', auto_download: false });
  saved.updates = { schedule: 'weekly', auto_download: true };
  await ipc('set_settings', { settings: saved });
  assert.deepEqual((await ipc('get_settings')).updates, saved.updates);
  saved.updates = { schedule: 'manual', auto_download: false };
  await ipc('set_settings', { settings: saved });
  operations.push({ check: 'real public release check and native settings', status: checked });
  await close();

  const fixtureSource = join(root, 'updated.rs');
  const fixtureBinary = join(root, 'updated.exe');
  await writeFile(fixtureSource, `fn main() { let args: Vec<String> = std::env::args().collect(); let index = args.iter().position(|v| v == "--data-dir").unwrap(); std::fs::write(std::path::Path::new(&args[index + 1]).join("updated-receipt.txt"), "synthetic updated native executable").unwrap(); }\n`);
  await run('rustc', [fixtureSource, '-o', fixtureBinary], { env: process.env });
  const packaged = await archivePortable({ platform: 'windows', arch, version: nextVersion,
    input: fixtureBinary, binary: fixtureBinary, output: join(root, 'release') });
  const bytes = await readFile(packaged.archive);
  const name = `LLMUsage-${nextVersion}-windows-${arch}-portable.tar.zst`;
  const cache = join(data, 'update-cache', digest(Buffer.from(exe.toLowerCase())).slice(0, 24));
  await mkdir(cache, { recursive: true });
  await copyFile(packaged.archive, join(cache, name));
  await writeFile(join(cache, 'download.json'), JSON.stringify({ version: nextVersion, name, size: bytes.length,
    sha256: digest(bytes), url: `https://github.com/owent/llm-usage/releases/download/v${nextVersion}/${name}` }));
  const oldHash = digest(await readFile(exe));
  await launch();
  const ready = await waitStatus('ready');
  assert.equal(ready.package_kind, 'portable');
  assert.equal(ready.version, nextVersion);
  assert.ok(ready.last_checked_ms, 'check deadline survives restart');
  const exited = once(child, 'exit');
  const install = ipc('install_update').catch(error => {
    if (!/closed|destroyed|Target/i.test(String(error))) throw error;
  });
  let exitTimer;
  try {
    await Promise.race([exited, new Promise((_, reject) => { exitTimer = setTimeout(() => reject(new Error('original application did not exit')), 30_000); })]);
  } finally { clearTimeout(exitTimer); }
  await install;
  for (let i = 0; i < 300; i++) {
    try { if ((await readFile(join(data, 'updated-receipt.txt'), 'utf8')).includes('synthetic updated')) break; } catch {}
    await pause(100);
  }
  assert.equal(await readFile(join(data, 'updated-receipt.txt'), 'utf8'), 'synthetic updated native executable');
  const work = join(packageRoot, '.llmusage-update');
  const journal = JSON.parse(await readFile(join(work, 'apply.json'), 'utf8'));
  assert.equal(journal.phase, 'applied');
  assert.equal(digest(await readFile(exe)), digest(await readFile(fixtureBinary)));
  assert.equal(digest(await readFile(join(work, 'backup/LLMUsage.exe'))), oldHash);
  assert.equal(await readFile(join(packageRoot, 'personal.txt'), 'utf8'), 'keep this unrelated file');
  assert.equal(JSON.parse(await readFile(join(packageRoot, 'llmusage-package.json'), 'utf8')).version, nextVersion);
  const retained = new DatabaseSync(join(data, 'llm-usage.sqlite'), { readOnly: true });
  assert.equal(retained.prepare('SELECT value FROM update_acceptance').get().value, 'retained usage sentinel');
  assert.deepEqual(JSON.parse(retained.prepare("SELECT value FROM settings WHERE key='app_settings'").get().value).updates, { schedule: 'manual', auto_download: false });
  retained.close();
  operations.push({ check: 'real update IPC, helper exit wait, in-place replacement, backup, restart, unrelated file and SQLite preservation',
    artifact: 'synthetic native executable, injected completed portable archive', ready, phase: journal.phase });
  await close();
  journal.phase = 'applying';
  journal.error = null;
  await writeFile(join(work, 'apply.json'), JSON.stringify(journal));
  await unlink(exe);
  const recoveryPort = await freeHttpPort();
  restoredApplication = true;
  await run(join(work, 'helper.exe'), [], { env: { ...env,
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${recoveryPort}`,
    WEBVIEW2_USER_DATA_FOLDER: join(root, 'recovered-webview') } });
  await attach(recoveryPort);
  const recovered = await waitStatus('ready');
  assert.equal(recovered.previous_error, 'update_interrupted_and_restored');
  assert.equal(digest(await readFile(exe)), oldHash);
  assert.equal(JSON.parse(await readFile(join(packageRoot, 'llmusage-package.json'), 'utf8')).version, version);
  assert.equal(await readFile(join(packageRoot, 'personal.txt'), 'utf8'), 'keep this unrelated file');
  assert.equal(JSON.parse(await readFile(join(work, 'apply.json'), 'utf8')).phase, 'rolled_back');
  operations.push({ check: 'standalone helper restores missing original executable and relaunches real application with saved failure notice', phase: 'rolled_back', previous_error: recovered.previous_error });
  await writeFile(join(root, 'report.json'), JSON.stringify({ passed: true, platform: process.platform, version, operations,
    limits: ['No newer public application release was installed.', 'This test does not establish NSIS or Linux/macOS GUI update acceptance.'] }, null, 2));
  console.log(`Windows update checks passed: ${root}`);
} catch (error) {
  await writeFile(join(root, 'report.json'), JSON.stringify({ passed: false, error: String(error), operations }, null, 2));
  throw error;
} finally { await close(); }
