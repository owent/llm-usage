// Real executable + real files/SQLite; isolated environment, no Agent requests.
import { DatabaseSync } from 'node:sqlite';
import { spawnIsolated } from './isolated-child.mjs';
import { once } from 'node:events';
import { mkdir, writeFile, readFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import assert from 'node:assert/strict';

const option = (name, fallback) => process.argv.includes(name) ? process.argv[process.argv.indexOf(name)+1] : fallback;
const exe = resolve(option('--exe', `desktop/src-tauri/target/release/LLMUsage${process.platform==='win32'?'.exe':''}`));
const root = resolve('build/plan-completion/headless',String(Date.now()));
const data = join(root,'含空格 data');
const sourceHome = join(root,'source-home');
const sessions = join(sourceHome,'.codex/sessions');
const temp = join(root,'temp');
await Promise.all([mkdir(data,{recursive:true}),mkdir(sessions,{recursive:true}),mkdir(temp,{recursive:true})]);
const env = Object.fromEntries(['PATH','SystemRoot','WINDIR','COMPUTERNAME','USERNAME','USERDOMAIN','ProgramFiles','ProgramFiles(x86)'].filter(key=>process.env[key]).map(key=>[key,process.env[key]]));
// No HOME/USERPROFILE: observed absolute roots (Kimi Work) require a home context.
// TEMP is also a source root for Visual Studio Copilot, so it must be isolated.
Object.assign(env,{CODEX_HOME:join(sourceHome,'.codex'),TEMP:temp,TMP:temp,APPDATA:join(sourceHome,'AppData/Roaming'),LOCALAPPDATA:join(sourceHome,'AppData/Local')});
const dbPath=join(data,'llm-usage.sqlite');
const checks=[],commands=[];
async function run(mode,code=0,extra=['--data-dir',data]) {
  const proc=spawnIsolated(exe,[mode,...extra],{env,stdio:['ignore','pipe','pipe'],windowsHide:true});
  let output='';proc.stdout.on('data',chunk=>output+=chunk);proc.stderr.on('data',chunk=>output+=chunk);
  const timeout=setTimeout(()=>proc.kill(),30000);
  try { const [result]=await once(proc,'exit');commands.push({mode,code:result,output});assert.equal(result,code,output);return output; }
  finally {clearTimeout(timeout);}
}
function db(f,readOnly=false) {const conn=new DatabaseSync(dbPath,{readOnly});try{return f(conn);}finally{conn.close();}}
function count() {return db(conn=>conn.prepare('SELECT COUNT(*) AS n FROM usage_events').get().n,true);}
function tokens() {return db(conn=>Number(conn.prepare('SELECT SUM(CAST(total_tokens AS INTEGER)) AS n FROM usage_events').get().n),true);}
function intent(enabled) {db(conn=>conn.prepare("INSERT INTO settings(key,value,schema_version,updated_at_ms) VALUES('background_task',?,1,0) ON CONFLICT(key) DO UPDATE SET value=excluded.value").run(JSON.stringify({enabled})));}
function interval(seconds) {db(conn=>{const row=conn.prepare("SELECT value FROM settings WHERE key='app_settings'").get();const settings=row?JSON.parse(row.value):{timezone:'UTC',week_start:0,language:'en',manual_roots:[]};settings.refresh_interval_secs=seconds;conn.prepare("INSERT INTO settings(key,value,schema_version,updated_at_ms) VALUES('app_settings',?,1,0) ON CONFLICT(key) DO UPDATE SET value=excluded.value").run(JSON.stringify(settings));});}
async function record(id,input) {
  const at=new Date().toISOString();
  const rows=[{timestamp:at,type:'session_meta',payload:{id,session_id:id,timestamp:at,originator:'codex_vscode',cli_version:'0.999.0-synthetic',model_provider:'openai',source:'vscode'}},
    {timestamp:at,type:'token_usage_record',payload:{thread_id:id,turn_id:id,session_id:id,response_id:id,usage:{input_tokens:input,cached_input_tokens:0,cache_write_input_tokens:0,output_tokens:5,reasoning_output_tokens:0,total_tokens:input+5}}}];
  await writeFile(join(sessions,`rollout-${id}.jsonl`),rows.map(row=>JSON.stringify(row)).join('\n')+'\n');
}
await record('one',10);
await run('--headless');assert.equal(count(),0);checks.push('unconfigured leftover trigger does not scan');
await run('--scan-once');assert.equal(count(),1);assert.equal(tokens(),15);
assert.deepEqual(db(conn=>conn.prepare('SELECT DISTINCT agent FROM source_instances').all().map(row=>row.agent),true),['codex']);
await run('--scan-once');assert.equal(count(),1);checks.push('explicit manual CLI scans once and repeat is idempotent');
const source=db(conn=>conn.prepare("SELECT instance_id FROM source_instances WHERE agent='codex'").get().instance_id,true);
const sourceBytes=await readFile(join(sessions,'rollout-one.jsonl'));
await record('two',20);
intent(true);await run('--headless');assert.equal(count(),1);
checks.push('manual full scan postpones the shared system deadline');
interval(0);intent(true);await run('--headless');assert.equal(count(),1);checks.push('enabled OS intent cannot bypass global pause');
interval(3600);intent(false);await run('--headless');assert.equal(count(),1);checks.push('disabled OS intent cannot bypass enabled automatic interval');
intent(true);
const future=Date.now()+86400000;
db(conn=>conn.prepare("INSERT INTO extraction_schedules(schedule_id,scope,instance_id,rule_kind,interval_seconds,tz,enabled,config_version,next_due_at_ms,created_at_ms,updated_at_ms) VALUES(?,'source',?,'interval',86400,'UTC',1,1,?,0,0)").run(`source:${source}`,source,future));
await run('--headless');assert.equal(count(),1);checks.push('system global tick excludes not-yet-due custom source');
db(conn=>conn.prepare('UPDATE extraction_schedules SET next_due_at_ms=0').run());
await run('--headless');assert.equal(count(),2);assert.equal(tokens(),40);
assert.ok(db(conn=>conn.prepare('SELECT next_due_at_ms AS due FROM extraction_schedules').get().due,true)>Date.now());
const jobs=db(conn=>conn.prepare('SELECT COUNT(*) AS n FROM ingest_runs').get().n,true);
await run('--headless');assert.equal(count(),2);assert.equal(db(conn=>conn.prepare('SELECT COUNT(*) AS n FROM ingest_runs').get().n,true),jobs);
checks.push('due custom source scans and advances; frequent OS ticks do not rescan');
db(conn=>{conn.prepare('UPDATE source_instances SET enabled=0').run();conn.prepare('UPDATE extraction_schedules SET next_due_at_ms=0').run();});
await record('three',30);await run('--headless');await run('--scan-once');assert.equal(count(),2);
db(conn=>conn.prepare('UPDATE source_instances SET enabled=1').run());
await run('--scan-once');assert.equal(count(),3);assert.equal(tokens(),75);checks.push('disabled source skips both triggers; manual re-enable recovers without double count');
intent(false);await run('--headless');assert.equal(count(),3);
assert.deepEqual(await readFile(join(sessions,'rollout-one.jsonl')),sourceBytes);checks.push('source bytes unchanged throughout process scans');
const version=db(conn=>conn.prepare('PRAGMA user_version').get().user_version,true);
db(conn=>conn.exec('PRAGMA user_version=999'));
await run('--scan-once',1);assert.equal(count(),3);
db(conn=>conn.exec(`PRAGMA user_version=${version}`));checks.push('headless refuses newer schema without GUI prompt or data loss');
await run('--scan-once',2,['--data-dir']);await run('--scan-once',2,['--data-dir','relative']);checks.push('malformed data directory exits with argument error');
await writeFile(join(root,'result.json'),JSON.stringify({checks,commands,event_count:count(),total_tokens:tokens()},null,2));
console.log(JSON.stringify({root,checks:checks.length,event_count:count(),total_tokens:tokens()}));
