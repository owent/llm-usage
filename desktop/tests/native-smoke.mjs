// Windows desktop acceptance through the real WebView2 and Tauri IPC. The child
// receives isolated source/config/data directories; no IPC functions are mocked.
import { chromium } from 'playwright';
import { spawn } from 'node:child_process';
import { spawnIsolated } from './isolated-child.mjs';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { freeHttpPort } from './http-test-port.mjs';
import { once } from 'node:events';
import assert from 'node:assert/strict';
import { DatabaseSync } from 'node:sqlite';

assert.equal(process.platform, 'win32', 'native acceptance currently requires Windows');
const args = process.argv.slice(2);
const option = (name, fallback) => args.includes(name) ? args[args.indexOf(name) + 1] : fallback;
const exe = resolve(option('--exe', 'desktop/src-tauri/target/release/LLMUsage.exe'));
const runs = Number(option('--runs', '20'));
const dev = args.includes('--dev');
assert.ok(Number.isInteger(runs) && runs >= 1 && runs <= 100);
const root = resolve('build/plan-completion/native', String(Date.now()));
const data = join(root, '含空格 data');
const sourceHome = join(root, 'source-home');
const sessions = join(sourceHome, '.codex/sessions');
const temp = join(root, 'temp');
await Promise.all([mkdir(data, {recursive:true}), mkdir(sessions, {recursive:true}), mkdir(temp, {recursive:true})]);
const at = new Date().toISOString();
const day = new Intl.DateTimeFormat('en-CA', {timeZone:'Asia/Shanghai', year:'numeric', month:'2-digit', day:'2-digit'}).format(new Date());
const record = (id, input, timestamp=at) => [
  {timestamp,type:'session_meta',payload:{id,session_id:id,timestamp,originator:'codex_vscode',cli_version:'0.999.0-synthetic',model_provider:'openai',source:'vscode'}},
  {timestamp,type:'turn_context',payload:{model:'gpt-6-sol'}},
  {timestamp,type:'token_usage_record',payload:{thread_id:id,turn_id:`${id}-turn`,session_id:id,response_id:`${id}-response`,usage:{input_tokens:input,cached_input_tokens:0,cache_write_input_tokens:0,output_tokens:5,reasoning_output_tokens:0,total_tokens:input+5}}},
].map(row => JSON.stringify(row)).join('\n')+'\n';
await writeFile(join(sessions, 'rollout-native-one.jsonl'), record('native-one', 10));
const childEnv = Object.fromEntries(['PATH','SystemRoot','WINDIR','COMPUTERNAME','USERNAME','USERDOMAIN','ProgramFiles','ProgramFiles(x86)'].filter(key => process.env[key]).map(key => [key, process.env[key]]));
// No home context prevents observed absolute candidates; isolate TEMP-based traces.
Object.assign(childEnv, {CODEX_HOME:join(sourceHome,'.codex'), TEMP:temp, TMP:temp, APPDATA:join(sourceHome,'AppData/Roaming'), LOCALAPPDATA:join(sourceHome,'AppData/Local')});
const logs = [], errors = [], outbound = [], timings = [];
let child, browser, page, server;
async function invoke(cmd, args={}) {
  let timeout;
  try {
    return await Promise.race([
      page.evaluate(({cmd,args}) => window.__TAURI_INTERNALS__.invoke(cmd,args), {cmd,args}),
      new Promise((_,reject) => {timeout=setTimeout(()=>reject(new Error(`native IPC timed out: ${cmd}`)),30000);}),
    ]);
  } finally {clearTimeout(timeout);}
}
const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
const query = {first_day:day,last_day:day,granularity:'day',agents:[],providers:[],models:[]};
async function port() {
  return freeHttpPort();
}
async function launch() {
  const cdpPort = await port();
  const started = performance.now();
  child = spawnIsolated(exe, ['--data-dir',data], {env:{...childEnv,WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-port=${cdpPort} --force-renderer-accessibility --enable-logging --log-file="${join(root,'webview.log')}"`,WEBVIEW2_USER_DATA_FOLDER:join(root,'webview-profile')}, stdio:['ignore','pipe','pipe'], windowsHide:true});
  logs.push(`native pid=${child.pid}; requested CDP port=${cdpPort}`);
  child.stdout.on('data', chunk => logs.push(String(chunk)));
  child.stderr.on('data', chunk => logs.push(String(chunk)));
  let endpoint;
  for (let n=0;n<300;n++) {
    if (child.exitCode !== null) throw new Error('native application exited before WebView2 startup');
    try { endpoint = await (await fetch(`http://127.0.0.1:${cdpPort}/json/version`)).json(); } catch {}
    if (endpoint?.webSocketDebuggerUrl) break;
    await pause(100);
  }
  assert.ok(endpoint?.webSocketDebuggerUrl, 'WebView2 debugging endpoint is available');
  browser = await chromium.connectOverCDP(endpoint.webSocketDebuggerUrl);
  for (let n=0;n<100;n++) {
    page = browser.contexts().flatMap(context => context.pages()).find(page => /tauri\.localhost|localhost:1421/.test(page.url()));
    if (page) break;
    await pause(100);
  }
  assert.ok(page, 'real Tauri window loaded');
  page.on('pageerror', error => errors.push(error.message));
  page.on('request', request => { if (/^https?:/.test(request.url()) && !/\/\/(tauri\.localhost|ipc\.localhost|localhost|127\.0\.0\.1)([:/]|$)/.test(request.url())) outbound.push(request.url()); });
  await page.waitForFunction(() => Boolean(window.__TAURI_INTERNALS__?.invoke));
  await page.locator('nav button').first().waitFor({state:'visible'});
  await page.locator('.today-cards').waitFor({state:'visible',timeout:15000});
  timings.push(performance.now()-started);
}
async function close() {
  if (browser) { await browser.close(); browser=undefined; }
  if (child && child.exitCode === null) { child.kill(); await once(child,'exit'); }
  child=undefined;
  page=undefined;
}
async function headless(mode='--headless') {
  const proc = spawnIsolated(exe,[mode,'--data-dir',data],{env:childEnv,stdio:['ignore','pipe','pipe'],windowsHide:true});
  let output=''; proc.stdout.on('data',chunk=>output+=chunk); proc.stderr.on('data',chunk=>output+=chunk);
  const timeout=setTimeout(()=>proc.kill(),30000);
  try { const [code]=await once(proc,'exit'); assert.equal(code,0,output); return output; }
  finally { clearTimeout(timeout); }
}
async function systemTrigger() {
  const script=await readFile(resolve('desktop/tests/system-task-trigger.ps1'),'utf8');
  const proc=spawnIsolated(join(childEnv.SystemRoot,'System32/WindowsPowerShell/v1.0/powershell.exe'),
    ['-NoProfile','-NonInteractive','-Command',script],
    {env:{...childEnv,LLM_USAGE_ACCEPTANCE_DATA:data,LLM_USAGE_ACCEPTANCE_EXE:exe},stdio:['ignore','pipe','pipe'],windowsHide:true});
  let output='';proc.stdout.on('data',chunk=>output+=chunk);proc.stderr.on('data',chunk=>output+=chunk);
  const timeout=setTimeout(()=>proc.kill(),90000);
  try {const [code]=await once(proc,'exit');assert.equal(code,0,output);return JSON.parse(output);}
  finally {clearTimeout(timeout);}
}
async function nativeWindow(action='State') {
  const script=await readFile(resolve('desktop/tests/native-window.ps1'),'utf8');
  const proc=spawnIsolated(join(childEnv.SystemRoot,'System32/WindowsPowerShell/v1.0/powershell.exe'),
    ['-NoProfile','-NonInteractive','-Command',`& {\n${script}\n} -RootPid ${child.pid} -Action ${action}`],
    {env:childEnv,stdio:['ignore','pipe','pipe'],windowsHide:true});
  let output='',error='';proc.stdout.on('data',chunk=>output+=chunk);proc.stderr.on('data',chunk=>error+=chunk);
  const timeout=setTimeout(()=>proc.kill(),30000);
  try {const [code]=await once(proc,'exit');assert.equal(code,0,error);return JSON.parse(output);}
  finally {clearTimeout(timeout);}
}
async function settled(previous) {
  for (let n=0;n<300;n++) { const status=await invoke('refresh_status'); if (!status.running && status.last_finished_ms && status.last_finished_ms !== previous) return status; await pause(100); }
  throw new Error('scan did not complete');
}
async function refresh() {
  const previous=(await invoke('refresh_status')).last_finished_ms;
  await invoke('refresh_sources'); return settled(previous);
}
const checks = [];
try {
  if (dev) {
    server = spawn(process.execPath,[resolve('desktop/node_modules/vite/bin/vite.js'),'--host','127.0.0.1','--port','1421','--strictPort'],{cwd:resolve('desktop'),stdio:['ignore','pipe','pipe'],windowsHide:true});
    server.stdout.on('data',chunk=>logs.push(String(chunk))); server.stderr.on('data',chunk=>logs.push(String(chunk)));
    for (let n=0;n<300;n++) {
      if (server.exitCode!==null) throw new Error('native test dev server exited');
      try { if ((await fetch('http://127.0.0.1:1421')).ok) break; } catch {}
      await pause(100);
    }
  }
  await launch();
  await settled();
  const initialSettings = await invoke('get_settings');
  const initialDay = new Intl.DateTimeFormat('en-CA',{timeZone:initialSettings.timezone,year:'numeric',month:'2-digit',day:'2-digit'}).format(new Date(at));
  const original = await invoke('summary',{q:{...query,first_day:initialDay,last_day:initialDay}});
  assert.equal(original.totals.call_count,1); assert.equal(original.totals.total_tokens_known,'15');
  checks.push('real discovery → IPC summary (1 call, 15 tokens)');
  let settings=await invoke('get_settings');
  settings.timezone='Asia/Shanghai'; settings.refresh_interval_secs=0; settings.language='en'; settings.theme='dark';
  await invoke('set_settings',{settings});
  let sources=(await invoke('list_sources')).sources;
  assert.deepEqual([...new Set(sources.map(row=>row.agent))], ['codex']);
  const source=sources.find(row=>row.agent==='codex'); assert.ok(source);
  await invoke('set_source_schedule',{instanceId:source.instance_id,rule:{kind:'daily',timeOfDay:'09:00',timezone:'Asia/Shanghai'}});
  settings.timezone='UTC'; await invoke('set_settings',{settings});
  sources=(await invoke('list_sources')).sources;
  assert.equal(sources.find(row=>row.instance_id===source.instance_id).schedule.timezone,'Asia/Shanghai');
  assert.equal(sources.find(row=>row.instance_id===source.instance_id).schedule.previewMs.length,3);
  settings.timezone='Asia/Shanghai'; await invoke('set_settings',{settings});
  const currentHour=Number(new Intl.DateTimeFormat('en-GB',{timeZone:'Asia/Shanghai',hour:'2-digit',hourCycle:'h23'}).format(new Date(at)));
  const secondHour=currentHour===8?'09':'08';
  await writeFile(join(sessions,'rollout-native-two.jsonl'),record('native-two',20,`${day}T${secondHour}:15:00+08:00`));
  await refresh();
  assert.equal((await invoke('summary',{q:query})).totals.total_tokens_known,'40');
  checks.push('manual refresh includes custom schedules while automatic collection is paused; schedule timezone stays fixed');
  await page.waitForFunction(()=>document.querySelector('.today-cards .card .value')?.textContent==='2');
  const selectionQuery=async()=>{
    const caption=await page.locator('.today-range-caption').textContent();
    const labels=caption.match(/\d{4}-\d{2}-\d{2} \d{2}:00/g);
    assert.ok(labels?.length);
    return {...query,granularity:'hour',first_period:labels[0],last_period:labels.at(-1)};
  };
  const hourly=page.locator('[data-panel-group="overviewToday"] .hourly');
  await hourly.scrollIntoViewIfNeeded();
  const box=await hourly.boundingBox();
  await page.mouse.move(box.x+box.width*.35,box.y+box.height*.5);await page.mouse.down();
  await page.mouse.move(box.x+box.width*.65,box.y+box.height*.5,{steps:12});
  assert.equal(await page.locator('.today-range-caption').count(),0,'native drag waits for release before applying the selection');
  await page.mouse.up();await page.locator('.today-range-caption').waitFor({state:'visible'});
  await page.waitForFunction(()=>document.querySelector('.today-cards .card .value')?.textContent==='2');
  const selected=await selectionQuery();
  assert.equal(selected.granularity,'hour');assert.notEqual(selected.first_period,selected.last_period);
  const scoped=await invoke('summary',{q:selected});
  assert.equal(scoped.totals.call_count,2);assert.equal(scoped.totals.total_tokens_known,'40');
  await hourly.scrollIntoViewIfNeeded();const clicked=await hourly.boundingBox();
  await page.mouse.click(clicked.x+clicked.width*.35,clicked.y+clicked.height*.5);
  await page.waitForFunction(()=>document.querySelector('.today-cards .card .value')?.textContent==='1');
  const point=await selectionQuery();
  assert.equal(point.first_period,point.last_period);
  assert.equal((await invoke('summary',{q:point})).totals.call_count,1,'native point selection restricts the actual data');
  await page.locator('.today-range-reset').click();
  await page.waitForFunction(()=>document.querySelector('.today-cards .card .value')?.textContent==='2');
  checks.push('real WebView2 mouse drag/point/reset → scoped IPC → SQLite usage across two hours');
  await hourly.focus();await page.keyboard.press('Home');await page.keyboard.press('Shift+End');
  assert.equal(await page.locator('.today-range-caption').count(),0,'keyboard movement waits for Enter');
  await page.keyboard.press('Enter');await page.locator('.today-range-caption').waitFor({state:'visible'});
  assert.equal((await invoke('summary',{q:await selectionQuery()})).totals.call_count,2);
  await page.locator('.today-range-reset').click();
  checks.push('real keyboard range selection uses the same scoped IPC and applies on Enter');
  const details=await invoke('event_details',{q:query,page:0,pageSize:20});
  assert.equal(details.total_count ?? details.total,2);
  const exported=await invoke('export_data',{kind:'exchange',targetDir:join(root,'exports'),q:query,userFilter:null,hostFilter:null});
  const exchange=JSON.parse(await readFile(exported.path,'utf8'));
  assert.ok(exchange.daily_partitions.length>0,'exchange has nonempty daily partitions');
  await invoke('import_exchange',{path:exported.path});
  await invoke('import_exchange',{path:exported.path});
  const detailExport=await invoke('export_data',{kind:'details',targetDir:join(root,'exports'),q:query,userFilter:null,hostFilter:null});
  const detailPackage=JSON.parse(await readFile(detailExport.path,'utf8'));
  assert.equal(detailPackage.format_version,'llm-usage-details-1');
  assert.ok(detailPackage.details.length>0,'native details export must be nonempty');
  assert.equal(detailPackage.archive.daily_partitions.length,0,'live days have one detail contribution');
  const preview=await invoke('preview_exchange',{path:detailExport.path});
  assert.equal(preview.details,detailPackage.details.length);
  const detailRevision=(await invoke('summary',{q:query})).data_revision;
  for(let replay=0;replay<2;replay++) {
    const merged=await invoke('import_exchange',{path:detailExport.path});
    assert.equal(merged.details_unchanged,detailPackage.details.length);
    assert.equal((await invoke('summary',{q:query})).data_revision,detailRevision);
  }
  checks.push('native nonempty detail export, preview and repeated Merge preserve statistics and revision');
  settings=await invoke('get_settings');
  assert.equal(settings.budget.enabled,false);
  settings.budget={enabled:true,metric:'total_tokens',period:'day',threshold:'1',currency:'USD'};
  await invoke('set_settings',{settings});
  const budget=await invoke('budget_status',{claim:true});
  assert.equal(budget.exceeded,true);
  assert.equal(budget.newly_triggered,true);
  assert.equal((await invoke('budget_status',{claim:true})).newly_triggered,false);
  settings.budget.enabled=false;
  await invoke('set_settings',{settings});
  checks.push('native opt-in budget threshold and persistent reminder deduplication');
  assert.equal((await invoke('summary',{q:query})).totals.total_tokens_known,'40');
  checks.push('nonempty exchange export and repeated same-source import retain totals');
  const manualBefore=(await invoke('refresh_status')).last_finished_ms;
  await headless('--scan-once'); await settled(manualBefore);
  assert.equal((await invoke('summary',{q:query})).totals.total_tokens_known,'40');
  checks.push('competing manual CLI queues for the GUI owner even while automatic collection is paused');
  try {
    await invoke('set_refresh_task',{install:true});
    const task=await invoke('system_task_status');
    assert.equal(task.refresh_task_desired,true); assert.equal(task.refresh_task,true);
    assert.equal(task.refresh_task_interval,'minute'); assert.equal(task.refresh_task_error,null);
    const pausedBefore=(await invoke('refresh_status')).last_finished_ms;
    await headless(); await pause(700);
    assert.equal((await invoke('refresh_status')).last_finished_ms,pausedBefore);
    assert.equal((await systemTrigger()).completed,true);
    await pause(700);
    assert.equal((await invoke('refresh_status')).last_finished_ms,pausedBefore);
  } finally { await invoke('set_refresh_task',{install:false}); }
  const removed=await invoke('system_task_status');
  assert.equal(removed.refresh_task_desired,false); assert.equal(removed.refresh_task_exists,false);
  checks.push('real minute OS trigger, task enable/status/remove IPC and consent cannot bypass paused GUI collection');
  const before=(await invoke('refresh_status')).last_finished_ms;
  await headless(); await pause(700);
  assert.equal((await invoke('refresh_status')).last_finished_ms,before);
  checks.push('leftover background trigger while disabled leaves the GUI and data untouched');
  await browser.contexts()[0].setOffline(true);
  await refresh();
  assert.equal((await invoke('summary',{q:query})).totals.total_tokens_known,'40');
  checks.push('offline real IPC refresh, summary and details');
  await browser.contexts()[0].setOffline(false);
  await close();
  await headless();
  await launch();
  const restored=await invoke('get_settings');
  assert.equal(restored.refresh_interval_secs,0); assert.equal(restored.theme,'dark'); assert.equal(restored.language,'en');
  assert.equal((await invoke('refresh_status')).last_finished_ms,0,'paused restart does not scan');
  assert.equal((await invoke('summary',{q:query})).totals.total_tokens_known,'40');
  checks.push('paused native restart restores settings and history without automatic scan');
  await page.getByRole('button',{name:'Sources',exact:true}).click();
  await page.locator('[data-testid="schedule-preview"]').waitFor({state:'visible'});
  await page.getByRole('button',{name:'Settings',exact:true}).click();
  await page.locator('[data-testid="language-select"]').selectOption('zh-CN');
  await page.getByRole('button',{name:'Save',exact:true}).click();
  await page.getByRole('button',{name:'数据源',exact:true}).waitFor({state:'visible'});
  checks.push('real DOM schedule preview and language save through IPC');
  const locales=['zh-CN','zh-TW','en','ja','ko','es','fr','de','pt-BR','ru'];
  for (const locale of locales) {
    await page.locator('[data-testid="language-select"]').selectOption(locale);
    await page.locator('button[type="submit"]').click();
    await page.waitForFunction(locale=>document.documentElement.lang===locale,locale);
    assert.equal((await invoke('get_settings')).language,locale);
    const unnamed=await page.locator('input:not([type="hidden"]),select,textarea').evaluateAll(els=>els.filter(el=>el.offsetWidth && !el.getAttribute('aria-label') && !el.labels?.length).map(el=>el.tagName+':'+el.className));
    assert.deepEqual(unnamed,[],`visible settings controls have accessible names in ${locale}`);
    for(const zoom of [1,1.25,1.5,2]) {
      await page.evaluate(zoom=>{document.documentElement.style.zoom=String(zoom);},zoom);
      await pause(50);
      const width=await page.evaluate(()=>({client:document.documentElement.clientWidth,scroll:document.documentElement.scrollWidth}));
      assert.ok(width.scroll<=width.client+2,`${locale} zoom ${zoom}: no document overflow`);
      assert.ok(await page.locator('button[type="submit"]').isVisible());
    }
    await page.evaluate(()=>{document.documentElement.style.zoom='';});
    assert.equal((await invoke('summary',{q:query})).totals.total_tokens_known,'40');
  }
  checks.push('ten languages saved through real IPC, accessible settings names, 100–200% page zoom and stable totals');
  settings=await invoke('get_settings');settings.refresh_interval_secs=15;
  await invoke('set_source_schedule',{instanceId:source.instance_id,rule:null});
  settings.manual_roots=[join(sourceHome,'.codex')];settings.manual_roots_only=true;
  await invoke('set_settings',{settings});
  await refresh();
  await invoke('set_refresh_task',{install:true});
  await close();
  try {
    await writeFile(join(sessions,'rollout-native-background.jsonl'),record('native-background',30));
    assert.equal((await systemTrigger()).completed,true);
    const scanDb=new DatabaseSync(join(data,'llm-usage.sqlite'),{readOnly:true});
    try {
      assert.equal(scanDb.prepare('SELECT COUNT(*) AS count FROM usage_events').get().count,3,'OS task imported before GUI startup');
      assert.equal(scanDb.prepare('SELECT SUM(total_tokens) AS tokens FROM usage_events').get().tokens,75);
      assert.deepEqual(scanDb.prepare('SELECT DISTINCT agent FROM source_instances').all().map(row=>row.agent),['codex'],'OS source scope stays isolated');
    } finally {scanDb.close();}
    await launch();
    assert.equal((await invoke('summary',{q:query})).totals.total_tokens_known,'75');
  } finally {
    if(!page) await launch();
    await invoke('set_refresh_task',{install:false});
  }
  settings=await invoke('get_settings');settings.refresh_interval_secs=0;
  await invoke('set_settings',{settings});
  assert.equal((await invoke('system_task_status')).refresh_task_exists,false);
  checks.push('real minute OS trigger with GUI exited imports a new local event; restart retains it and removes the owned task');
  settings=await invoke('get_settings');
  Object.assign(settings,{language:'en',refresh_interval_secs:86400,file_watch_enabled:true,close_to_tray:true,manual_roots_only:true,manual_roots:[join(sourceHome,'.codex')]});
  await invoke('set_settings',{settings});await refresh();await pause(1500);
  const previous=(await invoke('refresh_status')).last_finished_ms;
  await writeFile(join(sessions,'rollout-native-watch.jsonl'),record('native-watch',30));
  await settled(previous);
  assert.equal((await invoke('summary',{q:query})).totals.total_tokens_known,'110');
  checks.push('native directory notification imports a new event with global interval one day');
  settings.refresh_interval_secs=0;await invoke('set_settings',{settings});await pause(1000);
  await writeFile(join(sessions,'rollout-native-paused.jsonl'),record('native-paused',35));await pause(3000);
  assert.equal((await invoke('summary',{q:query})).totals.total_tokens_known,'110','automatic pause covers directory notifications');
  await refresh();assert.equal((await invoke('summary',{q:query})).totals.total_tokens_known,'150');
  checks.push('paused watcher reads no new usage while manual collection still works');
  await page.reload();await page.getByRole('button',{name:'Settings',exact:true}).waitFor();
  const accessibility=await nativeWindow('Accessibility');
  await writeFile(join(root,'accessibility.json'),JSON.stringify(accessibility,null,2));
  assert.ok(accessibility.dpi>=96);assert.ok(accessibility.button_names.includes('Settings'),'Windows UI Automation exposes navigation button names');
  checks.push(`Windows UI Automation with forced accessibility exposes navigation names; actual OS DPI ${accessibility.dpi}`);
  await nativeWindow('Close');await pause(500);assert.equal(child.exitCode,null);
  assert.equal((await nativeWindow()).visible,false,'close request hides an opted-in window');
  settings.close_to_tray=false;await invoke('set_settings',{settings});await pause(500);
  assert.equal((await nativeWindow()).visible,true,'disabling tray restores the hidden window');
  checks.push('real WM_CLOSE hides to tray and disabling tray restores the native window');
  await close();
  for (let i=timings.length;i<runs;i++) { await launch(); await close(); }
  timings.sort((a,b)=>a-b);
  const report={checks,mode:dev?'debug':'release',event_count:5,total_tokens:150,os_dpi:accessibility.dpi,accessibility,first_screen_ms:timings,p95_ms:timings[Math.ceil(timings.length*.95)-1],sample_count:timings.length,errors,outbound};
  assert.deepEqual(errors,[]); assert.deepEqual(outbound,[]);
  await writeFile(join(root,'result.json'),JSON.stringify(report,null,2));
  console.log(JSON.stringify({root,checks:checks.length,samples:timings.length,p95_ms:report.p95_ms}));
} catch (error) {
  if(page) await page.screenshot({path:join(root,'failure.png')}).catch(()=>{});
  await writeFile(join(root,'failure.json'),JSON.stringify({message:String(error),errors,logs},null,2));
  throw error;
} finally { await close(); if(server && server.exitCode===null) {server.kill();await once(server,'exit');} }
