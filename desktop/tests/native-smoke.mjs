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
const tokenFields=['input_total_known','uncached_known','cache_read_known','cache_write_known','output_total_known','total_tokens_known'];
const countFields=['input_known_count','input_unknown_count','output_known_count','output_unknown_count','total_known_count','total_unknown_count','event_count','call_count','attempt_count','observation_count','conflict_count'];
const ipcCountFields=countFields.filter(field=>field!=='observation_count');
const metricFields=[...tokenFields,...countFields];
const normalizedPath=path=>path.replaceAll('\\','/').replace(/\/+$/,'').toLowerCase();
function combinedMetrics(rows) {
  const result={};
  for(const field of tokenFields) {
    const values=rows.map(row=>row[field]).filter(value=>value!==null);
    result[field]=values.length?values.reduce((sum,value)=>sum+value,0):null;
  }
  for(const field of countFields) result[field]=rows.reduce((sum,row)=>sum+row[field],0);
  return result;
}
function assertMetrics(actual,expected,label) {
  for(const field of tokenFields) assert.equal(actual[field],expected[field]===null?null:String(expected[field]),`${label}: ${field}`);
  for(const field of ipcCountFields) assert.equal(actual[field],expected[field],`${label}: ${field}`);
}
function assertSummaryMatches(snapshot,summary) {
  const agents=Object.keys(snapshot.agents).sort();
  assert.deepEqual(summary.agents.map(row=>row.agent).sort(),agents,'summary agents match independent SQLite rows');
  for(const agent of agents) assertMetrics(summary.agents.find(row=>row.agent===agent).sums,snapshot.agents[agent],agent);
  assertMetrics(summary.totals,combinedMetrics(Object.values(snapshot.agents)),'all agents');
  assert.equal(summary.excluded_event_count,0);
}
async function createMultiSourceData() {
  const fixtures=resolve('desktop/src-tauri/crates/core/tests/fixtures');
  const projection=JSON.parse(await readFile(join(fixtures,'opencode/real-2.0.32-windows/projection.json'),'utf8'));
  assert.equal(projection.messages.length,7);
  const openCodeManual=join(sourceHome,'multi-opencode');
  const openCodeData=join(openCodeManual,'opencode');
  await mkdir(openCodeData,{recursive:true});
  const openCodeDb=join(openCodeData,'opencode.db');
  const db=new DatabaseSync(openCodeDb);
  try {
    db.exec(`CREATE TABLE session_v2(id TEXT PRIMARY KEY,parent_id TEXT,version TEXT,fork_session_id TEXT,fork_boundary TEXT);
      CREATE TABLE session_message(id TEXT PRIMARY KEY,session_id TEXT,type TEXT,seq INTEGER,time_created INTEGER,time_updated INTEGER,data TEXT);`);
    const session=db.prepare('INSERT INTO session_v2(id,parent_id,version,fork_session_id,fork_boundary) VALUES (?,?,?,?,?)');
    const message=db.prepare('INSERT INTO session_message VALUES (?,?,?,?,?,?,?)');
    db.exec('BEGIN');
    try {
      for(const row of projection.sessions) session.run(row.id,null,row.version,null,null);
      for(const row of projection.messages) message.run(row.id,row.session_id,row.type,row.seq,row.time_created,row.time_updated,JSON.stringify(row.data));
      const created=projection.messages[0].time_created+1000;
      const compaction={time:{created},status:'completed',model:null,tokens:{input:100,output:20,reasoning:5,cache:{read:30,write:2}},cost:0};
      message.run('synthetic-compaction','session-2','compaction',7,created,created,JSON.stringify(compaction));
      db.exec('COMMIT');
    } catch(error) {db.exec('ROLLBACK');throw error;}
  } finally {db.close();}
  const qoderManual=join(sourceHome,'multi-qoder');
  const qoderFixture=await readFile(join(fixtures,'qoder/source-cli-1.1.76/session.jsonl'),'utf8');
  const qoderFiles=[];
  for(const suffix of ['a','b']) {
    const rows=qoderFixture.trim().split('\n').map(line=>{
      const row=JSON.parse(line);
      if(typeof row.sessionId==='string') row.sessionId=`${row.sessionId}-${suffix}`;
      if(typeof row.forkedFrom?.sessionId==='string') row.forkedFrom.sessionId=`${row.forkedFrom.sessionId}-${suffix}`;
      return JSON.stringify(row);
    });
    const project=join(qoderManual,'projects',`project-${suffix}`);
    await mkdir(project,{recursive:true});
    const file=join(project,'session.jsonl');
    await writeFile(file,rows.join('\n')+'\n');
    qoderFiles.push(file);
  }
  return {openCodeManual,qoderManual,openCodeDb,qoderFiles};
}
function readMultiSnapshot() {
  const db=new DatabaseSync(join(data,'llm-usage.sqlite'),{readOnly:true});
  try {
    const rows=db.prepare(`WITH normalized AS (
      SELECT agent,record_kind,quality_bucket,conflict,
        CASE WHEN json_extract(quality_json,'$.input_total') IN ('reported','derived') THEN input_total END AS known_input,
        CASE WHEN json_extract(quality_json,'$.input_uncached') IN ('reported','derived') THEN input_uncached END AS known_uncached,
        CASE WHEN json_extract(quality_json,'$.input_cache_read') IN ('reported','derived') THEN input_cache_read END AS known_read,
        CASE WHEN json_extract(quality_json,'$.input_cache_write') IN ('reported','derived') THEN input_cache_write END AS known_write,
        CASE WHEN json_extract(quality_json,'$.output_total') IN ('reported','derived') THEN output_total END AS known_output,
        CASE WHEN json_extract(quality_json,'$.total_tokens') IN ('reported','derived') THEN total_tokens END AS known_total
      FROM usage_events WHERE attribution_status='verified'
        AND record_kind IN ('model_call','transport_attempt','usage_observation'))
      SELECT agent,COUNT(*) AS event_count,
        SUM(CASE WHEN record_kind='model_call' THEN 1 ELSE 0 END) AS call_count,
        SUM(CASE WHEN record_kind='transport_attempt' THEN 1 ELSE 0 END) AS attempt_count,
        SUM(CASE WHEN record_kind='usage_observation' THEN 1 ELSE 0 END) AS observation_count,
        SUM(CASE WHEN record_kind!='transport_attempt' THEN known_input END) AS input_total_known,
        SUM(CASE WHEN record_kind!='transport_attempt' AND known_input IS NOT NULL THEN 1 ELSE 0 END) AS input_known_count,
        SUM(CASE WHEN record_kind!='transport_attempt' AND quality_bucket!='unknown' AND known_input IS NULL THEN 1 ELSE 0 END) AS input_unknown_count,
        SUM(CASE WHEN record_kind!='transport_attempt' THEN known_uncached END) AS uncached_known,
        SUM(CASE WHEN record_kind!='transport_attempt' THEN known_read END) AS cache_read_known,
        SUM(CASE WHEN record_kind!='transport_attempt' THEN known_write END) AS cache_write_known,
        SUM(CASE WHEN record_kind!='transport_attempt' THEN known_output END) AS output_total_known,
        SUM(CASE WHEN record_kind!='transport_attempt' AND known_output IS NOT NULL THEN 1 ELSE 0 END) AS output_known_count,
        SUM(CASE WHEN record_kind!='transport_attempt' AND quality_bucket!='unknown' AND known_output IS NULL THEN 1 ELSE 0 END) AS output_unknown_count,
        SUM(CASE WHEN record_kind!='transport_attempt' THEN known_total END) AS total_tokens_known,
        SUM(CASE WHEN record_kind!='transport_attempt' AND known_total IS NOT NULL THEN 1 ELSE 0 END) AS total_known_count,
        SUM(CASE WHEN record_kind!='transport_attempt' AND quality_bucket!='unknown' AND known_total IS NULL THEN 1 ELSE 0 END) AS total_unknown_count,
        SUM(conflict) AS conflict_count
      FROM normalized GROUP BY agent ORDER BY agent`).all();
    const agents=Object.fromEntries(rows.map(row=>[row.agent,Object.fromEntries(metricFields.map(field=>[field,row[field]]))]));
    const files=db.prepare(`SELECT f.file_id,s.agent,f.status FROM source_files f
      JOIN source_instances s ON s.instance_id=f.instance_id ORDER BY f.file_id`).all().map(row=>({file_id:row.file_id,agent:row.agent,status:row.status}));
    const compaction=db.prepare(`SELECT COUNT(*) AS events,
      SUM(CASE WHEN record_kind='usage_observation' THEN 1 ELSE 0 END) AS observations,
      SUM(CASE WHEN record_kind='model_call' THEN 1 ELSE 0 END) AS calls,
      SUM(total_tokens) AS total_tokens FROM usage_events WHERE origin_call_id='synthetic-compaction'`).get();
    const qoderUnknownDefaults=db.prepare(`SELECT COUNT(*) AS count FROM usage_events
      WHERE agent='qoder' AND origin_call_id='official' AND input_total IS NULL
        AND output_total IS NULL AND total_tokens IS NULL AND quality_bucket='unknown'`).get().count;
    const cumulativeCount=db.prepare('SELECT COUNT(*) AS count FROM source_aggregates').get().count;
    return {agents,files,compaction,qoderUnknownDefaults,cumulativeCount};
  } finally {db.close();}
}
function addedByAgent(status) {
  return Object.fromEntries(['codex','opencode','qoder'].map(agent=>[agent,status.instances.filter(row=>row.agent===agent).reduce((sum,row)=>sum+row.added,0)]));
}
const formatNumber=value=>value===null?'—':new Intl.NumberFormat('en').format(BigInt(value));
const formatSmart=value=>new Intl.NumberFormat('en',{notation:'compact',maximumFractionDigits:1}).format(Number(value));
const shiftDay=(day,delta)=>new Date(Date.parse(day+'T00:00:00Z')+delta*86400000).toISOString().slice(0,10);
const checks = [], pauseStates = [];
let multiSource;
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
  assert.equal(initialSettings.updates.auto_download,true,'automatic downloads default on');
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
  const pauseSnapshot=async phase=>{
    const status=await invoke('refresh_status');
    pauseStates.push({phase,running:status.running,trigger:status.trigger,last_finished_ms:status.last_finished_ms});
  };
  try {
    await invoke('set_refresh_task',{install:true});
    const task=await invoke('system_task_status');
    assert.equal(task.refresh_task_desired,true); assert.equal(task.refresh_task,true);
    assert.equal(task.refresh_task_interval,'minute'); assert.equal(task.refresh_task_error,null);
    const pausedBefore=(await invoke('refresh_status')).last_finished_ms;
    await pauseSnapshot('task-installed');
    await headless(); await pause(700);
    await pauseSnapshot('headless');
    assert.equal((await invoke('refresh_status')).last_finished_ms,pausedBefore);
    assert.equal((await systemTrigger()).completed,true);
    await pause(700);
    await pauseSnapshot('minute-trigger');
    assert.equal((await invoke('refresh_status')).last_finished_ms,pausedBefore);
  } finally {
    try { await invoke('set_refresh_task',{install:false}); }
    catch(error) { throw new Error(`${error}; pauseStates=${JSON.stringify(pauseStates)}`); }
  }
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
  await page.getByRole('button',{name:'数据源',exact:true}).waitFor({state:'visible'});
  checks.push('real DOM schedule preview and language save through IPC');
  const locales=['zh-CN','zh-TW','en','ja','ko','es','fr','de','pt-BR','ru'];
  for (const locale of locales) {
    await page.locator('[data-testid="language-select"]').selectOption(locale);
    await page.waitForFunction(locale=>document.documentElement.lang===locale,locale);
    assert.equal((await invoke('get_settings')).language,locale);
    const unnamed=await page.locator('input:not([type="hidden"]),select,textarea').evaluateAll(els=>els.filter(el=>el.offsetWidth && !el.getAttribute('aria-label') && !el.labels?.length).map(el=>el.tagName+':'+el.className));
    assert.deepEqual(unnamed,[],`visible settings controls have accessible names in ${locale}`);
    for(const zoom of [1,1.25,1.5,2]) {
      await page.evaluate(zoom=>{document.documentElement.style.zoom=String(zoom);},zoom);
      await pause(50);
      const width=await page.evaluate(()=>({client:document.documentElement.clientWidth,scroll:document.documentElement.scrollWidth}));
      assert.ok(width.scroll<=width.client+2,`${locale} zoom ${zoom}: no document overflow`);
      assert.ok(await page.getByTestId('settings-feedback').isVisible());
    }
    await page.evaluate(()=>{document.documentElement.style.zoom='';});
    assert.equal((await invoke('summary',{q:query})).totals.total_tokens_known,'40');
  }
  checks.push('ten languages saved through real IPC, accessible settings names, 100–200% page zoom and stable totals');
  await page.locator('[data-testid="language-select"]').selectOption('zh-CN');
  await page.waitForFunction(()=>document.documentElement.lang==='zh-CN');
  await page.locator('[data-settings-section="identity"]').click();
  await page.getByLabel('显示别名',{exact:true}).fill('Autosaved native alias');
  await page.getByRole('navigation').first().getByRole('button',{name:'总览',exact:true}).click();
  await page.waitForFunction(async () => (await window.__TAURI_INTERNALS__.invoke('get_settings')).hostname_alias === 'Autosaved native alias');
  await close(); await launch();
  assert.equal((await invoke('get_settings')).hostname_alias,'Autosaved native alias');
  checks.push('pending text autosaves before navigation and survives a native restart');
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

  const multi=await createMultiSourceData();
  settings=await invoke('get_settings');
  Object.assign(settings,{language:'en',timezone:'Asia/Shanghai',refresh_interval_secs:0,file_watch_enabled:false,close_to_tray:false,manual_roots_only:true,
    manual_roots:[join(sourceHome,'.codex'),multi.openCodeManual,multi.qoderManual]});
  await invoke('set_settings',{settings});
  const firstMultiRefresh=await refresh();
  assert.deepEqual(addedByAgent(firstMultiRefresh),{codex:0,opencode:8,qoder:8});
  assert.ok(firstMultiRefresh.instances.filter(row=>['codex','opencode','qoder'].includes(row.agent)).every(row=>row.error===null));
  const firstMultiSnapshot=readMultiSnapshot();
  for(const [agent,eventCount,callCount] of [['codex',5,5],['opencode',8,7],['qoder',8,8]]) {
    assert.equal(firstMultiSnapshot.agents[agent].event_count,eventCount,`${agent} events`);
    assert.equal(firstMultiSnapshot.agents[agent].call_count,callCount,`${agent} model calls`);
  }
  assert.deepEqual(firstMultiSnapshot.agents.opencode,{
    input_total_known:132,uncached_known:78911,cache_read_known:93662,cache_write_known:2,output_total_known:7913,total_tokens_known:157,
    input_known_count:1,input_unknown_count:7,output_known_count:8,output_unknown_count:0,total_known_count:1,total_unknown_count:7,
    event_count:8,call_count:7,attempt_count:0,observation_count:1,conflict_count:0,
  });
  assert.deepEqual(firstMultiSnapshot.agents.qoder,{
    input_total_known:340,uncached_known:160,cache_read_known:60,cache_write_known:20,output_total_known:72,total_tokens_known:394,
    input_known_count:4,input_unknown_count:2,output_known_count:6,output_unknown_count:0,total_known_count:4,total_unknown_count:2,
    event_count:8,call_count:8,attempt_count:0,observation_count:0,conflict_count:0,
  });
  assert.equal(firstMultiSnapshot.compaction.events,1);
  assert.equal(firstMultiSnapshot.compaction.observations,1);
  assert.equal(firstMultiSnapshot.compaction.calls,0,'compaction is not a model call');
  assert.equal(firstMultiSnapshot.compaction.total_tokens,157);
  assert.equal(firstMultiSnapshot.qoderUnknownDefaults,2,'default-zero Qoder usage stays unknown');
  assert.equal(firstMultiSnapshot.cumulativeCount,0,'token-stats/cumulative rows create no calls or usage events');
  const ownedPaths=new Set();
  for(const file of firstMultiSnapshot.files) {
    const id=normalizedPath(file.file_id);
    assert.ok(!ownedPaths.has(id),`physical file has multiple registrations: ${id}`);
    ownedPaths.add(id);
  }
  for(const [file,agent] of [[multi.openCodeDb,'opencode'],...multi.qoderFiles.map(file=>[file,'qoder'])]) {
    const matches=firstMultiSnapshot.files.filter(row=>normalizedPath(row.file_id)===normalizedPath(file));
    assert.equal(matches.length,1,`${agent} has exactly one registered physical file`);
    assert.equal(matches[0].agent,agent);
    assert.ok(['active','active_compat'].includes(matches[0].status));
  }
  const multiQuery={...query,first_day:'2026-10-19',last_day:day};
  const firstMultiSummary=await invoke('summary',{q:multiQuery});
  assertSummaryMatches(firstMultiSnapshot,firstMultiSummary);
  checks.push('full-registry refresh uniquely owns Codex/OpenCode/Qoder files; IPC matches independent SQLite known tokens and unknown coverage');

  const secondMultiRefresh=await refresh();
  assert.ok(secondMultiRefresh.instances.every(row=>row.added===0),'consecutive refresh adds no events');
  assert.deepEqual(readMultiSnapshot(),firstMultiSnapshot);
  const secondMultiSummary=await invoke('summary',{q:multiQuery});
  assert.deepEqual(secondMultiSummary.totals,firstMultiSummary.totals);
  assert.deepEqual(secondMultiSummary.agents,firstMultiSummary.agents);
  checks.push('second multi-source refresh has added=0 and unchanged calls, event counts and token components');

  await close();
  await launch();
  const restoredMultiSettings=await invoke('get_settings');
  assert.equal(restoredMultiSettings.manual_roots_only,true);
  assert.deepEqual(restoredMultiSettings.manual_roots,settings.manual_roots);
  assertSummaryMatches(firstMultiSnapshot,await invoke('summary',{q:multiQuery}));
  assert.deepEqual(readMultiSnapshot(),firstMultiSnapshot,'native restart retains exactly one copy of every multi-source event');
  const restartMultiRefresh=await refresh();
  assert.ok(restartMultiRefresh.instances.every(row=>row.added===0),'post-restart refresh adds no events');
  assert.deepEqual((await invoke('summary',{q:multiQuery})).totals,firstMultiSummary.totals);
  checks.push('close/reopen preserves multi-source history and a further real refresh remains idempotent');

  const mainNav=name=>page.locator('.app-shell > aside nav').getByRole('button',{name,exact:true});
  await mainNav('Overview').click();
  await page.locator('.today-cards').waitFor({state:'visible'});
  const multiToday=await invoke('summary',{q:query});
  assert.equal(multiToday.totals.call_count,5);
  assert.equal(multiToday.totals.total_tokens_known,'150');
  await page.waitForFunction(expected=>document.querySelector('.today-cards .card .value')?.textContent===expected,formatSmart(5));
  assert.ok((await page.locator('main').textContent()).trim().length>0);
  const todayAgent=page.locator('[data-panel-group="overviewToday"] .tables.single tbody tr').filter({hasText:'codex'});
  assert.equal(await todayAgent.count(),1);
  assert.equal((await todayAgent.locator('td').nth(1).textContent()).trim(),formatNumber(5));
  await page.locator('[data-panel-group="overviewHistory"] .calls-chart svg').waitFor({state:'visible'});
  assert.ok(await page.locator('[data-panel-group="overviewHistory"] .calls-chart svg path').count()>0);

  const trendQuery={...multiQuery,first_day:shiftDay(day,-29)};
  const trendSummary=await invoke('summary',{q:trendQuery});
  assertSummaryMatches(firstMultiSnapshot,trendSummary);
  await mainNav('Trends').click();
  await page.locator('.range-summary .summary-cards').waitFor({state:'visible'});
  await page.waitForFunction(expected=>document.querySelector('.range-summary .scard .svalue')?.textContent===expected,formatSmart(trendSummary.totals.call_count));
  await page.locator('[data-panel-group="trendMain"] .calls-chart svg').waitFor({state:'visible'});
  await page.locator('[data-panel-group="trendMain"] .token-chart svg').waitFor({state:'visible'});
  const trendModelRows=page.locator('[data-panel-group="trendMain"] .tables.single tbody tr');
  assert.equal(await trendModelRows.count(),trendSummary.models.length);
  assert.ok(await trendModelRows.count()>0);
  assert.equal((await page.locator('[data-panel-group="trendMain"] tfoot td').first().textContent()).trim(),formatNumber(trendSummary.totals.call_count));
  checks.push('real Overview/Trends DOM renders nonempty charts and tables whose calls match IPC');

  const multiSources=(await invoke('list_sources')).sources;
  assert.deepEqual(multiSources.map(row=>row.agent).sort(),['codex','opencode','qoder']);
  await mainNav('Sources').click();
  await page.locator('.source-grid').waitFor({state:'visible'});
  assert.equal(await page.locator('.source-card').count(),3);
  for(const agent of ['codex','opencode','qoder']) {
    const source=multiSources.find(row=>row.agent===agent);
    assert.equal(source.health,'ok');
    assert.equal(source.available,true);
    assert.equal(source.enabled,true);
    assert.ok(source.last_success_ms>0);
    for(const field of ['degraded_files','unsupported_files','incompatible_files','missing_files']) assert.equal(source[field],0,`${agent}: ${field}`);
    const card=page.locator('.source-card').filter({has:page.getByRole('heading',{name:agent,exact:true})});
    assert.equal(await card.count(),1,`${agent} has one visible owner card`);
    assert.equal(await card.locator('.health.attention').count(),0);
    assert.equal(await card.locator('.tag.bad').count(),0);
  }
  checks.push('real Sources DOM shows three healthy unique owners without degraded, unsupported or missing files');

  const firstDetails=await invoke('event_details',{q:trendQuery,page:0,pageSize:20});
  const secondDetails=await invoke('event_details',{q:trendQuery,page:1,pageSize:20});
  assert.equal(firstDetails.total,21);
  assert.equal(firstDetails.rows.length,20);
  assert.equal(secondDetails.rows.length,1);
  assert.equal(firstDetails.rows.filter(row=>row.agent==='qoder'&&row.input===null&&row.output===null&&row.total===null).length,2);
  const compactionDetail=[...firstDetails.rows,...secondDetails.rows].find(row=>row.agent==='opencode'&&row.model===null);
  assert.ok(compactionDetail);
  assert.equal(compactionDetail.total,'157');
  const assertDetailRows=async expected=>{
    const actual=await page.locator('.details tbody tr').evaluateAll(rows=>rows.map(row=>{
      const cells=row.querySelectorAll('td');
      return JSON.stringify([cells[1].textContent.trim(),cells[2].textContent.trim(),cells[3].textContent.trim(),cells[4].textContent.trim(),cells[5].textContent.trim(),cells[6].textContent.trim(),cells[7].textContent.trim()]);
    })).then(rows=>rows.sort());
    const wanted=expected.rows.map(row=>JSON.stringify([row.agent,row.model??'unknown',row.category??'unknown',formatNumber(row.input),formatNumber(row.cache_read),formatNumber(row.output),formatNumber(row.total)])).sort();
    assert.deepEqual(actual,wanted,'DOM detail rows preserve IPC values and unknown dashes');
  };
  await mainNav('Details').click();
  await page.locator('.details .size-select select').selectOption('20');
  await page.waitForFunction(()=>document.querySelector('.page-info')?.textContent.includes('Page 1 of 2')&&document.querySelectorAll('.details tbody tr').length===20);
  await assertDetailRows(firstDetails);
  await page.getByRole('button',{name:'Next',exact:true}).click();
  await page.waitForFunction(()=>document.querySelector('.page-info')?.textContent.includes('Page 2 of 2')&&document.querySelectorAll('.details tbody tr').length===1);
  await assertDetailRows(secondDetails);
  await page.getByRole('button',{name:'Previous',exact:true}).click();
  await page.waitForFunction(()=>document.querySelector('.page-info')?.textContent.includes('Page 1 of 2')&&document.querySelectorAll('.details tbody tr').length===20);
  await assertDetailRows(firstDetails);
  checks.push('real Details DOM paginates 20+1 rows and preserves unknown token dashes instead of zero');

  await mainNav('Settings').click();
  const sectionPanels=[
    ['general','[data-testid="language-select"]'],
    ['updates','[data-testid="update-settings"]'],
    ['telemetry','#telemetry-settings'],
    ['retention','[data-settings-editor] .content .tier'],
    ['costs','#cost-enabled'],
    ['system','[data-settings-editor] .content .sysrow'],
    ['identity','[data-settings-editor] .content .meta'],
    ['export','[data-settings-editor] .content .export-filters'],
    ['logs','[data-settings-editor] .content .logs-toolbar'],
  ];
  const visitedSettings=[];
  for(const [section,selector] of sectionPanels) {
    await page.locator(`[data-settings-section="${section}"]`).click();
    assert.equal(await page.locator(`[data-settings-section="${section}"].active`).count(),1);
    await page.locator(selector).first().waitFor({state:'visible'});
    assert.ok((await page.locator('[data-settings-editor] .content').textContent()).trim().length>0,`${section} panel is nonempty`);
    visitedSettings.push(section);
  }
  assert.deepEqual(visitedSettings,['general','updates','telemetry','retention','costs','system','identity','export','logs']);
  checks.push('real Settings DOM visits all nine sections and exposes each corresponding panel');
  const multiTotals=combinedMetrics(Object.values(firstMultiSnapshot.agents));
  multiSource={event_count:multiTotals.event_count,call_count:multiTotals.call_count,observation_count:multiTotals.observation_count,
    agents:firstMultiSnapshot.agents,first_refresh_added:addedByAgent(firstMultiRefresh),second_refresh_added:0,restart_refresh_added:0,
    qoder_unknown_defaults:firstMultiSnapshot.qoderUnknownDefaults,cumulative_count:firstMultiSnapshot.cumulativeCount,detail_pages:2,settings_sections:visitedSettings};
  await close();
  for (let i=timings.length;i<runs;i++) { await launch(); await close(); }
  timings.sort((a,b)=>a-b);
  const report={checks,mode:dev?'debug':'release',event_count:5,total_tokens:150,multi_source:multiSource,os_dpi:accessibility.dpi,accessibility,first_screen_ms:timings,p95_ms:timings[Math.ceil(timings.length*.95)-1],sample_count:timings.length,errors,outbound};
  assert.deepEqual(errors,[]); assert.deepEqual(outbound,[]);
  await writeFile(join(root,'result.json'),JSON.stringify(report,null,2));
  console.log(JSON.stringify({root,checks:checks.length,samples:timings.length,p95_ms:report.p95_ms}));
} catch (error) {
  if(page) await page.screenshot({path:join(root,'failure.png')}).catch(()=>{});
  await writeFile(join(root,'failure.json'),JSON.stringify({message:String(error),pauseStates,errors,logs},null,2));
  throw error;
} finally { await close(); if(server && server.exitCode===null) {server.kill();await once(server,'exit');} }
