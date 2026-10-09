import assert from 'node:assert/strict';
import { copyFile, mkdir, writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { chromium } from 'playwright';
import { spawnIsolated } from './isolated-child.mjs';
import { freeHttpPort } from './http-test-port.mjs';
import { once } from 'node:events';

assert.equal(process.platform, 'win32');
const args = process.argv.slice(2);
const option = (name, fallback) => (args.includes(name) ? args[args.indexOf(name) + 1] : fallback);
const root = resolve('build/update-check/native', `${Date.now()}-${process.pid}`);
const executable = join(root, 'unpackaged', 'LLMUsage.exe');
const data = join(root, 'data');
const temp = join(root, 'temp');
const home = join(root, 'source');
await Promise.all(
  [join(root, 'unpackaged'), data, temp, home].map((path) => mkdir(path, { recursive: true }))
);
await copyFile(resolve(option('--exe', 'desktop/src-tauri/target/release/LLMUsage.exe')), executable);
const env = Object.fromEntries(
  ['PATH', 'SystemRoot', 'WINDIR', 'COMPUTERNAME', 'USERNAME', 'USERDOMAIN']
    .filter((key) => process.env[key])
    .map((key) => [key, process.env[key]])
);
const port = await freeHttpPort();
Object.assign(env, {
  TEMP: temp,
  TMP: temp,
  CODEX_HOME: join(home, '.codex'),
  APPDATA: join(home, 'AppData/Roaming'),
  LOCALAPPDATA: join(home, 'AppData/Local'),
  WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port}`,
});
const child = spawnIsolated(executable, ['--data-dir', data], { env, windowsHide: true, stdio: 'ignore' });
const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
let browser;
const report = { checks: [] };
try {
  for (let n = 0; n < 200; n++) {
    assert.equal(child.exitCode, null, 'owned application exited before IPC');
    try {
      browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
      break;
    } catch {
      await pause(100);
    }
  }
  assert.ok(browser, 'WebView2 must expose the owned debugging port');
  const page = browser.contexts()[0].pages()[0];
  await page.waitForFunction(() => typeof window.__TAURI_INTERNALS__?.invoke === 'function');
  const invoke = (command, args = {}) =>
    page.evaluate(({ command, args }) => window.__TAURI_INTERNALS__.invoke(command, args), { command, args });
  const settings = await invoke('get_settings');
  settings.refresh_interval_secs = 0;
  settings.manual_roots_only = true;
  settings.manual_roots = [];
  settings.updates = { schedule: 'manual', auto_download: true };
  await invoke('set_settings', { settings });
  for (let n = 0; n < 450; n++) {
    if ((await invoke('update_status')).phase !== 'checking') break;
    await pause(100);
  }
  await invoke('check_update');
  let status;
  for (let n = 0; n < 450; n++) {
    status = await invoke('update_status');
    if (!['checking', 'idle'].includes(status.phase)) break;
    await pause(100);
  }
  report.status = status;
  await writeFile(join(root, 'report.json'), JSON.stringify(report, null, 2));
  assert.ok(['up_to_date', 'available'].includes(status.phase), JSON.stringify(status));
  assert.equal(status.error, null);
  assert.equal(status.asset_name, null);
  assert.equal(status.package_kind, args.includes('--development') ? 'development' : 'unknown');
  report.checks.push('unpackaged executable checks real public version without selecting an asset');
  await assert.rejects(invoke('download_update'));
  assert.ok(['up_to_date', 'available'].includes((await invoke('update_status')).phase));
  report.checks.push('automatic download and direct download cannot act without an identity');
  await writeFile(join(root, 'report.json'), JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report));
} finally {
  await browser?.close();
  if (child.exitCode === null) {
    child.kill();
    await once(child, 'exit');
  }
}
