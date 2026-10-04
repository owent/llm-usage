// Empty real release -> synthetic Codex sources -> SQLite -> visible dashboard.
// All inputs/configuration live under build/. No Agent or model is launched.
import { chromium } from 'playwright';
import { spawnIsolated } from './isolated-child.mjs';
import { mkdir, readFile, writeFile, access } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { createServer } from 'node:net';
import { once } from 'node:events';
import assert from 'node:assert/strict';
import { importPeakBudgetBytes } from './resource-budgets.mjs';

if(process.argv.includes('--help')){
  console.log('node desktop/tests/native-first-import.mjs (Windows; 1,000,000 synthetic events, empty owned DB, full GUI, all-process peak)');
  process.exit(0);
}
assert.equal(process.platform,'win32');
const root=resolve('build/plan-continuation/native-first-import',String(Date.now()));
const data=join(root,'data'),temp=join(root,'temp'),home=join(root,'home');
await Promise.all([data,temp,home].map(p=>mkdir(p,{recursive:true})));
const env=Object.fromEntries(['PATH','SystemRoot','WINDIR','COMPUTERNAME','USERNAME','USERDOMAIN','ProgramFiles','ProgramFiles(x86)'].filter(k=>process.env[k]).map(k=>[k,process.env[k]]));
Object.assign(env,{HOME:home,CODEX_HOME:join(home,'.codex'),TEMP:temp,TMP:temp,APPDATA:join(root,'config'),LOCALAPPDATA:join(root,'local')});
const expectedEvents=1000000,eventsPerSource=10000,sourceCount=expectedEvents/eventsPerSource;
const timestamp=new Date().toISOString(),day=timestamp.slice(0,10);
const query={first_day:day,last_day:day,granularity:'day',agents:[],providers:[],models:[]};
const ready=join(root,'sampling.ready'),stop=join(root,'sampling.stop');
const errors=[],outbound=[],rounds=[];
let child,browser,page,sampler,sampled='',samplingErrors='',samplingDone,timeout,failed=false;
const pause=ms=>new Promise(r=>setTimeout(r,ms));
const invoke=(cmd,args={})=>page.evaluate(({cmd,args})=>window.__TAURI_INTERNALS__.invoke(cmd,args),{cmd,args});
const row=payload=>JSON.stringify({timestamp,...payload})+'\n';
async function waitIdle(previous){
  const started=performance.now();
  for(;;){
    assert.equal(child.exitCode,null);
    const status=await invoke('refresh_status');
    if(!status.running&&(previous===undefined||status.last_finished_ms>previous))return status;
    assert.ok(performance.now()-started<900000,'collection must finish in the sampling window');
    await pause(250);
  }
}
try{
  const server=createServer();server.listen(0,'127.0.0.1');await once(server,'listening');
  const port=server.address().port;await new Promise(r=>server.close(r));
  child=spawnIsolated(resolve('desktop/src-tauri/target/release/LLMUsage.exe'),['--data-dir',data],{env:{...env,WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-port=${port}`,WEBVIEW2_USER_DATA_FOLDER:join(root,'webview')},stdio:'ignore',windowsHide:true});
  let endpoint;
  for(let n=0;n<300;n++){assert.equal(child.exitCode,null);try{endpoint=await(await fetch(`http://127.0.0.1:${port}/json/version`)).json();}catch{}if(endpoint?.webSocketDebuggerUrl)break;await pause(100);}
  assert.ok(endpoint?.webSocketDebuggerUrl);
  browser=await chromium.connectOverCDP(endpoint.webSocketDebuggerUrl);
  for(let n=0;n<100;n++){page=browser.contexts().flatMap(c=>c.pages()).find(p=>/tauri\.localhost/.test(p.url()));if(page)break;await pause(100);}
  assert.ok(page);await page.locator('.collect-button').waitFor();
  page.on('pageerror',()=>errors.push('page_error'));
  page.on('request',r=>{if(/^https?:/.test(r.url())&&!/\/\/(tauri\.localhost|ipc\.localhost|localhost|127\.0\.0\.1)([:/]|$)/.test(r.url()))outbound.push('external_request');});
  await waitIdle();assert.equal((await invoke('storage_stats')).events,0);
  const settings=await invoke('get_settings');
  Object.assign(settings,{manual_roots_only:true,manual_roots:[],refresh_interval_secs:0,language:'en',theme:'dark',timezone:'UTC'});
  await invoke('set_settings',{settings});
  // Saving through backend IPC does not mutate the already mounted frontend locale.
  await page.reload();await page.waitForFunction(()=>document.documentElement.lang==='en');
  // App.svelte deliberately replaces all-zero cards with its empty-state view.
  await page.locator('.collect-button').waitFor();
  // Generate one bounded 10k-event file per source, never the whole input in RAM.
  for(let source=0;source<sourceCount;source++){
    const sourceRoot=join(root,'source',String(source),'.codex'),session=`import-${source}`;
    await mkdir(join(sourceRoot,'sessions'),{recursive:true});settings.manual_roots.push(sourceRoot);
    let content=row({type:'session_meta',payload:{id:session,session_id:session,timestamp,originator:'codex_vscode',cli_version:'0.999.0-synthetic',model_provider:'openai',source:'vscode'}})+row({type:'turn_context',payload:{model:'gpt-6-sol'}});
    for(let event=0;event<eventsPerSource;event++)content+=row({type:'token_usage_record',payload:{thread_id:session,session_id:session,turn_id:`turn-${event}`,response_id:`response-${event}`,usage:{input_tokens:10,cached_input_tokens:0,cache_write_input_tokens:0,output_tokens:5,reasoning_output_tokens:0,total_tokens:15}}});
    await writeFile(join(sourceRoot,'sessions','rollout-import.jsonl'),content);
  }
  await invoke('set_settings',{settings});await waitIdle();
  assert.equal((await invoke('storage_stats')).events,0,'first import starts with an empty database');
  const samplerScript=await readFile(resolve('desktop/tests/process-resources.ps1'),'utf8');
  const psLiteral=value=>"'"+value.replace(/'/g,"''")+"'";
  sampler=spawnIsolated(join(env.SystemRoot,'System32/WindowsPowerShell/v1.0/powershell.exe'),['-NoProfile','-NonInteractive','-Command',`& {\n${samplerScript}\n} -RootPid ${child.pid} -Seconds 900 -IntervalMs 500 -ReadyFile ${psLiteral(ready)} -StopFile ${psLiteral(stop)}`],{env,stdio:['ignore','pipe','pipe'],windowsHide:true});
  sampler.stdout.on('data',v=>sampled+=v);sampler.stderr.on('data',v=>samplingErrors+=v);
  samplingDone=once(sampler,'exit');
  timeout=setTimeout(()=>{if(child.exitCode===null)child.kill();if(sampler.exitCode===null)sampler.kill();},960000);
  for(let n=0;n<300;n++){assert.equal(sampler.exitCode,null,samplingErrors);try{await access(ready);break;}catch{}await pause(100);}
  await access(ready);
  const started=performance.now();
  for(let round=0;round<30;round++){
    const previous=(await invoke('refresh_status')).last_finished_ms;
    await page.locator('.collect-button').click();await waitIdle(previous);
    const events=(await invoke('storage_stats')).events;rounds.push(events);
    assert.ok(events>(rounds.at(-2)??0),'each continuation must commit new events');
    console.log(JSON.stringify({phase:'import',round:round+1,events}));
    if(events===expectedEvents)break;
    await page.waitForFunction(()=>!document.querySelector('.collect-button')?.disabled);
  }
  assert.equal(rounds.at(-1),expectedEvents);
  const summary=await invoke('summary',{q:query});
  assert.equal(summary.totals.call_count,expectedEvents);assert.equal(BigInt(summary.totals.total_tokens_known),BigInt(expectedEvents)*15n);
  await page.waitForFunction(()=>document.querySelector('.today-cards .card:first-child .value')?.textContent?.trim()==='1M',null,{timeout:30000});
  await page.locator('[data-panel-group="overviewHistory"] .token-chart svg').waitFor();
  const duration=performance.now()-started;
  await writeFile(stop,'stop');const [samplingCode]=await samplingDone;
  assert.equal(samplingCode,0,samplingErrors);
  const resources=JSON.parse(sampled);
  assert.ok(resources.sample_count>2&&resources.duration_seconds>=duration/1000);
  assert.ok(resources.samples.every(s=>s.process_count>0&&Object.values(s.private_bytes_by_role).reduce((a,b)=>a+b,0)===s.private_bytes));
  const before=(await invoke('refresh_status')).last_finished_ms;await invoke('refresh_sources');await waitIdle(before);
  assert.equal((await invoke('storage_stats')).events,expectedEvents,'unchanged reread is idempotent');
  assert.deepEqual(errors,[]);assert.deepEqual(outbound,[]);
  const report={source:'synthetic Codex JSONL',source_count:sourceCount,events:expectedEvents,total_tokens:expectedEvents*15,import_to_visible_ms:duration,continuation_events:rounds,sampling_interval_ms:500,resources,peak_limit_bytes:importPeakBudgetBytes,within_peak_budget:resources.peak_private_bytes<=importPeakBudgetBytes,errors,outbound};
  await writeFile(join(root,'result.json'),JSON.stringify(report,null,2));
  console.log(JSON.stringify({root,events:expectedEvents,import_to_visible_ms:duration,peak_private_mib:resources.peak_private_bytes/1048576,within_peak_budget:report.within_peak_budget}));
}catch(error){
  failed=true;
  await writeFile(join(root,'failure.json'),JSON.stringify({error:String(error),samplingErrors,rounds,errors,outbound}));
  if(sampled)await writeFile(join(root,'partial-resources.json'),sampled);
  throw error;
}
finally{
  clearTimeout(timeout);
  if(sampler&&sampler.exitCode===null){
    // Preserve the last sample even if a UI assertion fails while the app survives.
    await writeFile(stop,'stop');
    let samplerTimeout;
    try{
      await Promise.race([samplingDone,new Promise(r=>{samplerTimeout=setTimeout(()=>{if(sampler.exitCode===null)sampler.kill();r();},5000);})]);
    }finally{clearTimeout(samplerTimeout);}
    await samplingDone;
  }
  if(failed&&sampled)await writeFile(join(root,'partial-resources.json'),sampled);
  if(browser)await browser.close();
  if(child&&child.exitCode===null){child.kill();await once(child,'exit');}
}
