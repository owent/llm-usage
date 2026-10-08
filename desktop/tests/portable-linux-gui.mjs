import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer } from 'node:net';
import { DatabaseSync } from 'node:sqlite';

const repository = fileURLToPath(new URL('../../', import.meta.url));
const executable = resolve(process.argv[2] ?? '');
assert.equal(process.platform, 'linux');
assert.notEqual(process.getuid(), 0);
const base = join(repository, 'build/portable-linux-gui');
await mkdir(base, { recursive: true });
const root = await mkdtemp(join(base, 'case-'));
const data = join(root, '含空格 data');
const home = join(root, 'source-home');
const runtime = join(root, 'runtime');
await Promise.all([mkdir(data), mkdir(join(home, '.codex/sessions'), { recursive: true }), mkdir(runtime, { mode: 0o700 })]);
const at = new Date().toISOString();
const rows = [
  { timestamp: at, type: 'session_meta', payload: { id: 'portable-gui', session_id: 'portable-gui', timestamp: at,
    originator: 'codex_vscode', cli_version: '0.999.0-synthetic', model_provider: 'openai', source: 'vscode' } },
  { timestamp: at, type: 'token_usage_record', payload: { thread_id: 'portable-gui', turn_id: 'portable-gui',
    session_id: 'portable-gui', response_id: 'portable-gui', usage: { input_tokens: 10, cached_input_tokens: 0,
      cache_write_input_tokens: 0, output_tokens: 5, reasoning_output_tokens: 0, total_tokens: 15 } } },
];
await writeFile(join(home, '.codex/sessions/rollout-portable.jsonl'), rows.map(row => JSON.stringify(row)).join('\n') + '\n');
const env = { PATH: process.env.PATH, HOME: home, CODEX_HOME: join(home, '.codex'),
  XDG_CONFIG_HOME: join(home, '.config'), XDG_DATA_HOME: join(home, '.local/share'), XDG_CACHE_HOME: join(home, '.cache'),
  XDG_RUNTIME_DIR: runtime, TEMP: join(root, 'temp'), TMP: join(root, 'temp'), TMPDIR: join(root, 'temp'),
  TAURI_WEBVIEW_AUTOMATION: 'true', GDK_SCALE: '1', GDK_DPI_SCALE: '1' };
await mkdir(env.TEMP);
const server = createServer(); server.listen(0, '127.0.0.1'); await once(server, 'listening');
const port = server.address().port; await new Promise(done => server.close(done));
const children = [];
let session;
let driverLog = '';
const sleep = ms => new Promise(done => setTimeout(done, ms));
async function request(method, route, payload) {
  const response = await fetch(`http://127.0.0.1:${port}${route}`, { method,
    headers: { 'Content-Type': 'application/json' }, body: payload ? JSON.stringify(payload) : undefined,
    signal: AbortSignal.timeout(30_000) });
  const result = await response.json();
  if (!response.ok || result.value?.error) throw new Error(JSON.stringify(result));
  return result.value;
}
async function script(text, args = [], asynchronous = false) {
  return request('POST', `/session/${session}/execute/${asynchronous ? 'async' : 'sync'}`, { script: text, args });
}
async function ipc(command) {
  const result = await script('const done=arguments[arguments.length-1];window.__TAURI_INTERNALS__.invoke(arguments[0]).then(value=>done({ok:true,value}),error=>done({ok:false,error:String(error)}));', [command], true);
  assert.equal(result.ok, true, JSON.stringify(result)); return result.value;
}
try {
  const display = spawn('Xvfb', ['-displayfd', '1', '-screen', '0', '1440x1000x24', '-nolisten', 'tcp'], { env, stdio: ['ignore', 'pipe', 'inherit'] });
  children.push(display);
  let displayOutput = '';
  const number = await new Promise((done, reject) => {
    const timeout = setTimeout(() => reject(new Error('Xvfb startup timed out')), 15_000);
    display.stdout.on('data', chunk => { displayOutput += chunk; if (/^\d+\n/.test(displayOutput)) { clearTimeout(timeout); done(displayOutput.trim()); } });
    display.once('error', reject); display.once('exit', code => { clearTimeout(timeout); reject(new Error(`Xvfb exited ${code}`)); });
  });
  env.DISPLAY = `:${number}`;
  const driver = spawn('dbus-run-session', ['--', 'WebKitWebDriver', '--host=127.0.0.1', `--port=${port}`], { env, stdio: ['ignore', 'pipe', 'pipe'], detached: true });
  children.push(driver);
  driver.stdout.on('data', chunk => driverLog += chunk); driver.stderr.on('data', chunk => driverLog += chunk);
  for (let attempt = 0; attempt < 100; attempt++) { try { await request('GET', '/status'); break; } catch { await sleep(100); } }
  const created = await request('POST', '/session', { capabilities: { alwaysMatch: {
    'webkitgtk:browserOptions': { binary: executable, args: ['--data-dir', data] },
  } } });
  session = created.sessionId;
  await request('POST', `/session/${session}/timeouts`, { script: 30_000, pageLoad: 30_000, implicit: 0 });
  for (let attempt = 0; attempt < 150; attempt++) {
    if (await script('return Boolean(window.__TAURI_INTERNALS__?.invoke && document.querySelector(".today-cards"));')) break;
    await sleep(100);
  }
  assert.equal(await script('return Boolean(window.__TAURI_INTERNALS__?.invoke && document.querySelector(".today-cards"));'), true);
  let sources;
  for (let attempt = 0; attempt < 100; attempt++) {
    sources = await ipc('list_sources'); if (sources.sources.length === 1) break;
    await sleep(100);
  }
  assert.deepEqual(sources.sources.map(source => source.agent), ['codex']);
  const previous = (await ipc('refresh_status')).last_finished_ms;
  await ipc('refresh_sources');
  for (let attempt = 0; attempt < 200; attempt++) {
    const state = await ipc('refresh_status'); if (!state.running && state.last_finished_ms !== previous) break;
    await sleep(100);
  }
  const database = new DatabaseSync(join(data, 'llm-usage.sqlite'), { readOnly: true });
  try {
    const observation = database.prepare('SELECT COUNT(*) AS events, SUM(CAST(total_tokens AS INTEGER)) AS tokens FROM usage_events').get();
    assert.equal(observation.events, 1); assert.equal(observation.tokens, 15);
  } finally { database.close(); }
  for (let index = 0; index < 5; index++) {
    await script('document.querySelectorAll("nav button")[arguments[0]].click();', [index]); await sleep(200);
    assert.ok(await script('return document.body.innerText.length;') > 100);
  }
  const result = { executable, native_gui: true, real_ipc: true, events: 1, synthetic_tokens: 15, pages: 5, fuse: false };
  await writeFile(join(root, 'result.json'), JSON.stringify(result, null, 2));
  console.log(JSON.stringify(result));
} finally {
  await writeFile(join(root, 'driver.log'), driverLog);
  if (session) await request('DELETE', `/session/${session}`).catch(() => {});
  for (const child of children.reverse()) {
    if (child.pid) { try { if (child.spawnargs[0] === 'dbus-run-session') process.kill(-child.pid, 'SIGTERM'); else child.kill(); } catch {} }
  }
}
