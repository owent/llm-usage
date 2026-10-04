// Existing synthetic million-event DB -> copied snapshot -> real source/IPC/UI.
// Node 22.16+ backup API; never write to the supplied baseline or real Agent files.
import { chromium } from 'playwright';
import { DatabaseSync, backup } from 'node:sqlite';
import { mkdir, appendFile, writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { createServer } from 'node:net';
import { once } from 'node:events';
import assert from 'node:assert/strict';
import { spawnIsolated } from './isolated-child.mjs';

const args=process.argv.slice(2);
const option=(key,fallback)=>args.includes(key)?args[args.indexOf(key)+1]:fallback;
if(args.includes('--help')) {
  console.log('node desktop/tests/native-incremental.mjs [--base-data <bench_v20 directory>] [--exe <release.exe>] [--runs 20] [--enforce-budget]');
  process.exit(0);
}
assert.equal(process.platform,'win32');
const runs=Number(option('--runs','20'));
assert.ok(Number.isInteger(runs)&&runs>=1&&runs<=100);
const root=resolve('build/query-next/native-incremental',String(Date.now()));
const data=join(root,'data'),sessions=join(root,'source/.codex/sessions'),temp=join(root,'temp');
await Promise.all([mkdir(data,{recursive:true}),mkdir(sessions,{recursive:true}),mkdir(temp,{recursive:true})]);
const sourceDb=new DatabaseSync(join(resolve(option('--base-data','build/plan-finalization/million-1')),'llm-usage.sqlite'),{readOnly:true});
try {
  assert.equal(sourceDb.prepare('SELECT COUNT(*) AS n FROM usage_events').get().n,1000000);
  assert.equal(sourceDb.prepare('SELECT COUNT(*) AS n FROM source_instances').get().n,20);
  assert.equal(sourceDb.prepare("SELECT COUNT(*) AS n FROM source_instances WHERE format='synthetic' AND parser_version='bench'").get().n,20);
  await backup(sourceDb,join(data,'llm-usage.sqlite'),{rate:1000});
} finally {sourceDb.close();}
const db=new DatabaseSync(join(data,'llm-usage.sqlite'));
try {
  const settings=JSON.parse(db.prepare("SELECT value FROM settings WHERE key='app_settings'").get().value);
  Object.assign(settings,{manual_roots:[join(root,'source/.codex')],manual_roots_only:true,refresh_interval_secs:0,language:'en',theme:'dark'});
  // The fixed 366-day baseline may have been created on an earlier local day.
  // This test measures appends; retention has its own boundary regression suite.
  Object.assign(settings.retention,{events_days:0,hourly_days:0,daily_days:0});
  db.prepare("UPDATE settings SET value=? WHERE key='app_settings'").run(JSON.stringify(settings));
} finally {db.close();}
const timestamp=new Date().toISOString(),day=timestamp.slice(0,10),file=join(sessions,'rollout-incremental.jsonl');
const row=value=>JSON.stringify({timestamp,...value})+'\n';
const event=id=>row({type:'token_usage_record',payload:{thread_id:'incremental',session_id:'incremental',turn_id:`turn-${id}`,response_id:`response-${id}`,usage:{input_tokens:10,cached_input_tokens:0,cache_write_input_tokens:0,output_tokens:5,reasoning_output_tokens:0,total_tokens:15}}});
await writeFile(file,row({type:'session_meta',payload:{id:'incremental',session_id:'incremental',timestamp,originator:'codex_vscode',cli_version:'0.999.0-synthetic',model_provider:'openai',source:'vscode'}})+row({type:'turn_context',payload:{model:'gpt-6-sol'}})+event('initial'));
const env=Object.fromEntries(['PATH','SystemRoot','WINDIR','COMPUTERNAME','USERNAME','USERDOMAIN','ProgramFiles','ProgramFiles(x86)'].filter(key=>process.env[key]).map(key=>[key,process.env[key]]));
Object.assign(env,{TEMP:temp,TMP:temp,APPDATA:join(root,'config'),LOCALAPPDATA:join(root,'local'),CODEX_HOME:join(root,'source/.codex')});
const errors=[],outbound=[],logs=[],samples=[],query={first_day:day,last_day:day,granularity:'day',agents:[],providers:[],models:[]};
let child,browser,page;
const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));
async function invoke(cmd,args={}) {return page.evaluate(({cmd,args})=>window.__TAURI_INTERNALS__.invoke(cmd,args),{cmd,args});}
async function refreshAndWait() {
  const previous=(await invoke('refresh_status')).last_finished_ms;
  await invoke('refresh_sources');
  for(let i=0;i<300;i++) {
    const status=await invoke('refresh_status');
    if(!status.running&&status.last_finished_ms>previous)return;
    await pause(100);
  }
  throw new Error('refresh did not finish');
}
try {
  const server=createServer();server.listen(0,'127.0.0.1');await once(server,'listening');const port=server.address().port;await new Promise(resolve=>server.close(resolve));
  child=spawnIsolated(resolve(option('--exe','desktop/src-tauri/target/release/LLMUsage.exe')),['--data-dir',data],{env:{...env,WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-port=${port}`,WEBVIEW2_USER_DATA_FOLDER:join(root,'webview')},stdio:['ignore','pipe','pipe'],windowsHide:true});
  child.stdout.on('data',chunk=>logs.push(String(chunk)));child.stderr.on('data',chunk=>logs.push(String(chunk)));
  let endpoint;
  for(let i=0;i<300;i++){assert.equal(child.exitCode,null);try{endpoint=await(await fetch(`http://127.0.0.1:${port}/json/version`)).json();}catch{}if(endpoint?.webSocketDebuggerUrl)break;await pause(100);}
  assert.ok(endpoint?.webSocketDebuggerUrl);
  browser=await chromium.connectOverCDP(endpoint.webSocketDebuggerUrl);
  for(let i=0;i<100;i++){page=browser.contexts().flatMap(context=>context.pages()).find(page=>/tauri\.localhost/.test(page.url()));if(page)break;await pause(100);}
  assert.ok(page);page.on('pageerror',error=>errors.push(error.message));
  page.on('request',request=>{if(/^https?:/.test(request.url())&&!/\/\/(tauri\.localhost|ipc\.localhost|localhost|127\.0\.0\.1)([:/]|$)/.test(request.url()))outbound.push(request.url());});
  await page.locator('.today-cards .value').first().waitFor();
  // Discover/consume the source before measuring incremental append.
  await refreshAndWait();
  let previous=await invoke('summary',{q:query});
  const format=n=>new Intl.NumberFormat('en',{notation:'compact',maximumFractionDigits:1}).format(n);
  await page.waitForFunction(expected=>document.querySelector('.today-cards .card:first-child .value')?.textContent?.trim()===expected,format(previous.totals.call_count));
  for(let run=0;run<runs;run++) {
    await appendFile(file,Array.from({length:1000},(_,i)=>event(`${run}-${i}`)).join(''));
    const expected=previous.totals.call_count+1000;
    const started=performance.now();
    await page.locator('.collect-button').click();
    await page.waitForFunction(expected=>document.querySelector('.today-cards .card:first-child .value')?.textContent?.trim()===expected,format(expected),{timeout:30000});
    samples.push(performance.now()-started);
    const result=await invoke('summary',{q:query});
    assert.equal(result.totals.call_count,expected);
    assert.equal(BigInt(result.totals.total_tokens_known)-BigInt(previous.totals.total_tokens_known),15000n);
    assert.ok(result.data_revision>previous.data_revision,'cached summaries observe committed new events');
    assert.equal((await invoke('storage_stats')).events,1000001+(run+1)*1000);
    previous=result;
    await page.locator('.collect-button').waitFor({state:'visible'});
    await page.waitForFunction(()=>!document.querySelector('.collect-button')?.disabled);
  }
  await refreshAndWait();
  assert.equal((await invoke('summary',{q:query})).totals.call_count,previous.totals.call_count,'unchanged rescan is idempotent');
  assert.deepEqual(errors,[]);
  assert.deepEqual(outbound,[]);
  const sorted=[...samples].sort((a,b)=>a-b),p95=sorted[Math.ceil(sorted.length*.95)-1];
  const report={baseline_events:1000000,added_per_run:1000,runs,samples_ms:samples,p95_ms:p95,within_budget:p95<=2000,final_events:1000001+runs*1000,errors,outbound};
  await writeFile(join(root,'result.json'),JSON.stringify(report,null,2));
  console.log(JSON.stringify({root,...report}));
  if(args.includes('--enforce-budget'))assert.ok(report.within_budget,'incremental UI P95 exceeds 2 seconds');
}catch(error){if(page)await page.screenshot({path:join(root,'failure.png')}).catch(()=>{});await writeFile(join(root,'failure.json'),JSON.stringify({error:String(error),logs,errors},null,2));throw error;}
finally {if(browser)await browser.close();if(child&&child.exitCode===null){child.kill();await once(child,'exit');}}
