import assert from 'node:assert/strict';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { once } from 'node:events';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { DatabaseSync } from 'node:sqlite';
import { chromium } from '../../../desktop/node_modules/playwright/index.mjs';
import { spawnIsolated } from '../../../desktop/tests/isolated-child.mjs';
import { freeHttpPort } from '../../../desktop/tests/http-test-port.mjs';

const args = process.argv.slice(2);
if (args.includes('--help')) {
  console.log('Capture the actual Windows release UI with isolated synthetic sources.\nUsage: npm run docs:screenshots [-- --exe <absolute-executable>]\nWrites approved PNGs to docs/site/public/screenshots/ and temporary data/logs to build/documentation-site/captures/.\nRequires Windows, a built application and an available WebView2 CDP endpoint. No provider requests or IPC mocks.');
  process.exit(0);
}
assert.equal(process.platform, 'win32', 'Native documentation captures require Windows');
const exeFlag = args.indexOf('--exe');
const exe = resolve(exeFlag < 0 ? 'desktop/src-tauri/target/release/LLMUsage.exe' : args[exeFlag + 1]);
const version = execFileSync('pwsh', ['-NoLogo', '-NoProfile', '-NonInteractive', '-File',
  fileURLToPath(new URL('./executable-version.ps1', import.meta.url)), '-Executable', exe], { encoding: 'utf8' }).trim();
const task = resolve('build/documentation-site/captures', String(Date.now()));
const data = join(task, 'data');
const sourceHome = join(task, 'source-home');
const sessions = join(sourceHome, '.codex/sessions');
const temp = join(task, 'temp');
await Promise.all([data, sessions, temp].map(path => mkdir(path, { recursive: true })));
const today = new Intl.DateTimeFormat('en-CA', { timeZone: 'Asia/Shanghai', year: 'numeric', month: '2-digit', day: '2-digit' }).format(new Date());
let expectedCalls = 0;
for (let offset = 0; offset < 30; offset++) {
  const day = new Date(`${today}T00:00:00Z`);
  day.setUTCDate(day.getUTCDate() - offset);
  const date = day.toISOString().slice(0, 10);
  for (let session = 0; session < 2; session++) {
    const id = `documentation-${date}-${session}`;
    const timestamp = `${date}T08:00:00+08:00`;
    const rows = [{ timestamp, type: 'session_meta', payload: { id, session_id: id, timestamp, originator: 'codex_cli_rs', cli_version: '0.153.0', model_provider: 'openai', source: 'cli' } }];
    for (let call = 0; call < 4; call++) {
      const at = `${date}T${String(8 + call).padStart(2, '0')}:15:00+08:00`;
      const input = 4200 + ((offset * 17 + call * 13 + session * 5) % 23) * 320;
      const output = 200 + ((offset + call + session) % 7) * 90;
      const cache = Math.floor(input * (session ? 0.72 : 0.38));
      rows.push({ timestamp: at, type: 'turn_context', payload: { model: session ? 'gpt-6.1-sol' : 'gpt-6-sol' } });
      rows.push({ timestamp: at, type: 'token_usage_record', payload: { thread_id: id, session_id: id, turn_id: `${id}-turn-${call}`, response_id: `${id}-response-${call}`, usage: { input_tokens: input, cached_input_tokens: cache, cache_write_input_tokens: 32, output_tokens: output, reasoning_output_tokens: 80, total_tokens: input + output } } });
      expectedCalls++;
    }
    await writeFile(join(sessions, `rollout-${date}-${session}.jsonl`), rows.map(row => JSON.stringify(row)).join('\n') + '\n');
  }
}
const env = Object.fromEntries(['PATH', 'SystemRoot', 'WINDIR', 'COMPUTERNAME', 'USERNAME', 'USERDOMAIN', 'ProgramFiles', 'ProgramFiles(x86)'].filter(key => process.env[key]).map(key => [key, process.env[key]]));
Object.assign(env, { USERNAME: 'Demo', USER: 'demo', COMPUTERNAME: 'DEMO-PC', HOME: sourceHome, CODEX_HOME: join(sourceHome, '.codex'), TEMP: temp, TMP: temp, APPDATA: join(sourceHome, 'AppData/Roaming'), LOCALAPPDATA: join(sourceHome, 'AppData/Local') });
// Initialize storage without collection, then restrict discovery before the GUI starts.
const initializer = spawnIsolated(exe, ['--headless', '--data-dir', data], { env, stdio: ['ignore', 'pipe', 'pipe'], windowsHide: true });
let initialization = '';
initializer.stdout.on('data', chunk => initialization += chunk);
initializer.stderr.on('data', chunk => initialization += chunk);
const [initializationCode] = await once(initializer, 'exit');
assert.equal(initializationCode, 0, initialization);
assert.match(initialization, /executed=false/, 'Storage initialization does not collect sources');
const database = new DatabaseSync(join(data, 'llm-usage.sqlite'));
try {
  database.prepare('INSERT OR REPLACE INTO settings(key,value,schema_version,updated_at_ms) VALUES(?,?,1,?)').run('app_settings', JSON.stringify({ timezone: 'Asia/Shanghai', week_start: null, language: 'en', theme: 'light', refresh_interval_secs: 0, manual_roots_only: true, manual_roots: [join(sourceHome, '.codex')], hostname_alias: 'Documentation demo' }), Date.now());
} finally { database.close(); }
const cdpPort = await freeHttpPort();
const child = spawnIsolated(exe, ['--data-dir', data], { env: { ...env, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${cdpPort}`, WEBVIEW2_USER_DATA_FOLDER: join(task, 'webview-profile') }, stdio: ['ignore', 'pipe', 'pipe'], windowsHide: true });
const logs = [];
child.stdout.on('data', chunk => logs.push(String(chunk)));
child.stderr.on('data', chunk => logs.push(String(chunk)));
const pause = ms => new Promise(done => setTimeout(done, ms));
let browser;
const report = { version, executableSha256: createHash('sha256').update(await readFile(exe)).digest('hex'), provenance: 'actual Windows WebView2/Tauri IPC with isolated synthetic Codex records; rendered task paths replaced with Demo paths before capture; numbers and controls unchanged; no real-provider certification', capturedAt: new Date().toISOString(), timezone: 'Asia/Shanghai', viewport: { width: 1440, height: 1000 }, deviceScaleFactor: 2, expectedCalls, screenshots: [] };
try {
  let endpoint;
  for (let attempt = 0; attempt < 300; attempt++) {
    assert.equal(child.exitCode, null, 'Application exited before CDP startup');
    try { endpoint = await (await fetch(`http://127.0.0.1:${cdpPort}/json/version`)).json(); } catch { /* Wait for the owned child. */ }
    if (endpoint?.webSocketDebuggerUrl) break;
    await pause(100);
  }
  assert.ok(endpoint?.webSocketDebuggerUrl, 'WebView2 CDP endpoint is available');
  browser = await chromium.connectOverCDP(endpoint.webSocketDebuggerUrl);
  let page;
  for (let attempt = 0; attempt < 100; attempt++) {
    page = browser.contexts().flatMap(context => context.pages()).find(page => /tauri\.localhost/.test(page.url()));
    if (page) break;
    await pause(100);
  }
  assert.ok(page, 'Actual Tauri page is available');
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.setViewportSize(report.viewport);
  const emulation = await page.context().newCDPSession(page);
  await emulation.send('Emulation.setDeviceMetricsOverride', { ...report.viewport, deviceScaleFactor: report.deviceScaleFactor, mobile: false });
  await page.waitForFunction(() => Boolean(window.__TAURI_INTERNALS__?.invoke));
  const invoke = (cmd, args = {}) => page.evaluate(({ cmd, args }) => window.__TAURI_INTERNALS__.invoke(cmd, args), { cmd, args });
  await invoke('refresh_sources');
  for (let attempt = 0; attempt < 300; attempt++) {
    const status = await invoke('refresh_status');
    if (!status.running && status.last_finished_ms) break;
    await pause(100);
  }
  const settings = await invoke('get_settings');
  Object.assign(settings, { timezone: 'Asia/Shanghai', refresh_interval_secs: 0, manual_roots_only: true, manual_roots: [join(sourceHome, '.codex')], hostname_alias: 'Documentation demo' });
  const first = new Date(`${today}T00:00:00Z`);
  first.setUTCDate(first.getUTCDate() - 29);
  const summary = await invoke('summary', { q: { first_day: first.toISOString().slice(0, 10), last_day: today, granularity: 'day', agents: [], providers: [], models: [] } });
  assert.equal(summary.totals.call_count, expectedCalls, 'Actual backend imported every synthetic call');
  const sources = (await invoke('list_sources')).sources;
  assert.deepEqual([...new Set(sources.map(source => source.agent))], ['codex'], 'Host agent data is not discovered');
  for (const language of ['en', 'zh-CN']) {
    const directory = resolve('docs/site/public/screenshots', language);
    await mkdir(directory, { recursive: true });
    for (const theme of ['light', 'dark']) {
      Object.assign(settings, { language, theme });
      await invoke('set_settings', { settings });
      await page.reload();
      await page.waitForFunction(({ language, theme }) => document.documentElement.lang === language && document.documentElement.dataset.theme === theme, { language, theme });
      await page.locator('.today-cards').waitFor();
      await page.evaluate(() => document.fonts.ready);
      for (const [index, name] of ['overview', 'trend', 'sources', 'details', 'settings'].entries()) {
        await page.locator('nav button').nth(index).click();
        await page.waitForTimeout(1000);
        // Anonymize only owned task paths in the rendered UI before taking original PNG pixels.
        const replacements = [
          [sourceHome.replaceAll('\\', '/'), 'C:/Users/Demo'],
          [sourceHome.replaceAll('/', '\\'), 'C:\\Users\\Demo'],
          [task.replaceAll('\\', '/'), 'C:/Users/Demo/AppData/Local/LLMUsage'],
          [task.replaceAll('/', '\\'), 'C:\\Users\\Demo\\AppData\\Local\\LLMUsage'],
        ];
        await page.evaluate(replacements => {
          const clean = value => replacements.reduce((text, [actual, demo]) => text.replaceAll(actual, demo), value);
          const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
          let text;
          while ((text = walker.nextNode())) {
            if (['SCRIPT', 'STYLE'].includes(text.parentElement?.tagName ?? '')) continue;
            const value = clean(text.nodeValue ?? '');
            if (value !== text.nodeValue) text.nodeValue = value;
          }
          for (const element of document.querySelectorAll('input,textarea')) {
            const value = clean(element.value);
            if (value !== element.value) element.value = value;
          }
          for (const element of document.querySelectorAll('[title]')) element.title = clean(element.title);
        }, replacements);
        const visible = await page.evaluate(() => document.body.innerText + [...document.querySelectorAll('input,textarea')].map(element => element.value).join('\n'));
        assert.ok(!visible.includes(task) && !visible.includes(task.replaceAll('\\', '/')), 'Rendered screenshot has no actual task directory');
        const filename = `${name}-${theme}.png`;
        const path = join(directory, filename);
        await emulation.send('Emulation.setDeviceMetricsOverride', { ...report.viewport, deviceScaleFactor: report.deviceScaleFactor, mobile: false });
        await page.evaluate(() => document.fonts.ready);
        const capture = await emulation.send('Page.captureScreenshot', { format: 'png', fromSurface: true,
          clip: { x: 0, y: 0, ...report.viewport, scale: 1 } });
        const pixels = Buffer.from(capture.data, 'base64');
        assert.equal(pixels.readUInt32BE(16), report.viewport.width * report.deviceScaleFactor, 'Native capture has the declared pixel width');
        assert.equal(pixels.readUInt32BE(20), report.viewport.height * report.deviceScaleFactor, 'Native capture has the declared pixel height');
        await writeFile(path, pixels);
        report.screenshots.push({ path: `screenshots/${language}/${filename}`, language, theme, page: name, sha256: createHash('sha256').update(await readFile(path)).digest('hex') });
      }
    }
  }
  assert.deepEqual(errors, [], 'Screenshot pages have no JavaScript errors');
  await writeFile(resolve('docs/site/public/screenshots/provenance.json'), JSON.stringify(report, null, 2) + '\n');
  console.log(`Captured ${report.screenshots.length} real UI screenshots in both languages and themes.`);
} finally {
  if (browser) await browser.close();
  if (child.exitCode === null) { child.kill(); await once(child, 'exit'); }
  await writeFile(join(task, 'capture.log'), logs.join(''));
  await writeFile(join(task, 'report.json'), JSON.stringify(report, null, 2));
}
