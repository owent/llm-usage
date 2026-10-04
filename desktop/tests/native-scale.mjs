// Actual release/WebView2/IPC measurements against a synthetic million-event DB.
import { chromium } from 'playwright';
import { DatabaseSync } from 'node:sqlite';
import { spawnIsolated } from './isolated-child.mjs';
import { mkdir, readFile, writeFile, realpath } from 'node:fs/promises';
import { resolve, join, relative, isAbsolute } from 'node:path';
import { createServer } from 'node:net';
import { once } from 'node:events';
import assert from 'node:assert/strict';

const args=process.argv.slice(2);
const option=(key,fallback)=>args.includes(key)?args[args.indexOf(key)+1]:fallback;
if(args.includes('--help')){
  console.log('node desktop/tests/native-scale.mjs [--data <synthetic bench_v20 directory under build/>] [--exe <release.exe>] [--runs 20] [--idle-seconds 600] [--ui-cancel]');
  process.exit(0);
}
assert.equal(process.platform,'win32');
const data=resolve(option('--data','build/plan-finalization/million-1'));
// This test invokes maintenance IPC. Reject real Agent databases before launch.
const fromBuild=relative(await realpath(resolve('build')),await realpath(data));
assert.ok(fromBuild&&!isAbsolute(fromBuild)&&fromBuild!=='..'&&!fromBuild.startsWith('..\\'),'use an owned synthetic directory under build/');
const baseline=new DatabaseSync(join(data,'llm-usage.sqlite'),{readOnly:true});
try{
  assert.equal(baseline.prepare('SELECT COUNT(*) AS n FROM usage_events').get().n,1000000);
  assert.equal(baseline.prepare('SELECT COUNT(*) AS n FROM source_instances').get().n,20);
  assert.equal(baseline.prepare("SELECT COUNT(*) AS n FROM source_instances WHERE format='synthetic' AND parser_version='bench'").get().n,20);
}finally{baseline.close();}
const root=resolve('build/plan-finalization/native-scale',String(Date.now()));
const exe=resolve(option('--exe','desktop/src-tauri/target/release/LLMUsage.exe'));
const runs=Number(option('--runs','20')),idleSeconds=Number(option('--idle-seconds','600'));
assert.ok(Number.isInteger(runs)&&runs>=1&&runs<=100);
assert.ok(Number.isInteger(idleSeconds)&&idleSeconds>=0&&idleSeconds<=600);
await mkdir(root,{recursive:true});
const temp=join(root,'temp');await mkdir(temp,{recursive:true});
const env=Object.fromEntries(['PATH','SystemRoot','WINDIR','COMPUTERNAME','USERNAME','USERDOMAIN','ProgramFiles','ProgramFiles(x86)'].filter(key=>process.env[key]).map(key=>[key,process.env[key]]));
Object.assign(env,{TEMP:temp,TMP:temp,APPDATA:join(root,'config'),LOCALAPPDATA:join(root,'local'),CODEX_HOME:join(root,'absent-codex')});
const errors=[],outbound=[],timings=[],logs=[];
let child,browser,page;
const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));
async function invoke(cmd,args={}){return page.evaluate(({cmd,args})=>window.__TAURI_INTERNALS__.invoke(cmd,args),{cmd,args});}
async function close(){if(browser){await browser.close();browser=undefined;}if(child&&child.exitCode===null){child.kill();await once(child,'exit');}child=undefined;page=undefined;}
async function launch(){
  const server=createServer();server.listen(0,'127.0.0.1');await once(server,'listening');const port=server.address().port;await new Promise(resolve=>server.close(resolve));
  const started=performance.now();
  child=spawnIsolated(exe,['--data-dir',data],{env:{...env,WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-port=${port}`,WEBVIEW2_USER_DATA_FOLDER:join(root,'webview')},stdio:['ignore','pipe','pipe'],windowsHide:true});
  child.stdout.on('data',chunk=>logs.push(String(chunk)));child.stderr.on('data',chunk=>logs.push(String(chunk)));
  let endpoint;
  for(let n=0;n<300;n++){assert.equal(child.exitCode,null,'app remains running');try{endpoint=await(await fetch(`http://127.0.0.1:${port}/json/version`)).json();}catch{}if(endpoint?.webSocketDebuggerUrl)break;await pause(100);}
  assert.ok(endpoint?.webSocketDebuggerUrl);
  browser=await chromium.connectOverCDP(endpoint.webSocketDebuggerUrl);
  for(let n=0;n<100;n++){page=browser.contexts().flatMap(c=>c.pages()).find(p=>/tauri\.localhost/.test(p.url()));if(page)break;await pause(100);}
  assert.ok(page);
  page.on('pageerror',error=>errors.push(error.message));
  page.on('request',request=>{if(/^https?:/.test(request.url())&&!/\/\/(tauri\.localhost|ipc\.localhost|localhost|127\.0\.0\.1)([:/]|$)/.test(request.url()))outbound.push(request.url());});
  await page.locator('.today-cards').waitFor({state:'visible',timeout:30000});
  await page.waitForFunction(()=>{const v=document.querySelector('.today-cards .value')?.textContent;return v&&!['…','—'].includes(v);});
  await page.locator('[data-panel-group="overviewHistory"] .token-chart canvas').waitFor({state:'visible'});
  timings.push(performance.now()-started);
}
async function resources(){
  const script=await readFile(resolve('desktop/tests/process-resources.ps1'),'utf8');
  const proc=spawnIsolated(join(env.SystemRoot,'System32/WindowsPowerShell/v1.0/powershell.exe'),['-NoProfile','-NonInteractive','-Command',`& {\n${script}\n} -RootPid ${child.pid} -Seconds ${idleSeconds}`],{env,stdio:['ignore','pipe','pipe'],windowsHide:true});
  let stdout='',stderr='';proc.stdout.on('data',v=>stdout+=v);proc.stderr.on('data',v=>stderr+=v);
  const timeout=setTimeout(()=>proc.kill(),(idleSeconds+60)*1000);
  try{const [code]=await once(proc,'exit');assert.equal(code,0,stderr);return JSON.parse(stdout);}finally{clearTimeout(timeout);}
}
try{
  for(let n=0;n<runs;n++){await launch();if(n<runs-1)await close();}
  const stats=await invoke('storage_stats');assert.equal(stats.events,1000000);
  const before=await invoke('app_info');
  const preview=await invoke('clear_all_preview');assert.equal(preview.event_count,1000000);
  // Starting/cancelling maintenance uses the actual IPC; no invoke is replaced.
  await page.evaluate(()=>{window.__cleanupResult=null;window.__cleanupPromise=window.__TAURI_INTERNALS__.invoke('manual_cleanup',{daysBefore:1}).then(v=>window.__cleanupResult={value:v},e=>window.__cleanupResult={error:String(e)});});
  await pause(50);assert.equal(await invoke('cancel_cleanup'),true,'cancellation accepted during backup');
  await page.waitForFunction(()=>window.__cleanupResult!==null);
  assert.match((await page.evaluate(()=>window.__cleanupResult)).error,/operation_cancelled/);
  assert.equal((await invoke('storage_stats')).events,1000000);
  assert.equal((await invoke('app_info')).data_revision,before.data_revision);
  await page.evaluate(async()=>{
    window.__clearPhases=[];
    const handler=window.__TAURI_INTERNALS__.transformCallback(e=>window.__clearPhases.push(e.payload.phase));
    window.__clearListener=await window.__TAURI_INTERNALS__.invoke('plugin:event|listen',{event:'clear-all-progress',target:{kind:'Any'},handler});
  });
  assert.equal((await invoke('clear_all_data')).started,true);
  assert.equal(await invoke('cancel_cleanup'),true);
  await page.waitForFunction(()=>window.__clearPhases.includes('cancelled'));
  assert.equal((await invoke('storage_stats')).events,1000000);
  assert.equal((await invoke('app_info')).data_revision,before.data_revision);
  assert.equal(await invoke('cancel_cleanup'),false,'finished operation cannot report cancellation');
  assert.equal((await invoke('refresh_status')).last_finished_ms,0,'cancelled clear does not rescan');
  await page.evaluate(async()=>{await window.__TAURI_INTERNALS__.invoke('plugin:event|unlisten',{event:'clear-all-progress',eventId:window.__clearListener});});
  if(args.includes('--ui-cancel')){
    await page.getByRole('button',{name:'Settings',exact:true}).click();
    await page.locator('[data-settings-section="retention"]').click();
    await page.locator('.days').fill('1');
    await page.locator('.days').locator('..').locator('button.danger').click();
    const cancel=page.getByRole('button',{name:'Cancel cleanup',exact:true});
    await cancel.click();
    assert.ok(!(await cancel.isVisible())||await cancel.isDisabled(),'accepted cancellation cannot be requested again before completion');
    await page.getByText('Cleanup cancelled. Data is unchanged.',{exact:true}).waitFor();
    assert.equal((await invoke('storage_stats')).events,1000000);
    assert.equal((await invoke('app_info')).data_revision,before.data_revision);
    await page.getByRole('button',{name:'Overview',exact:true}).click();
  }
  // Let prior maintenance UI work settle before ten-minute idle measurement.
  await pause(2000);
  const idle=idleSeconds?await resources():null;
  if(idle){assert.ok(idle.duration_seconds>=idleSeconds,'idle sampling covers the full requested duration');assert.ok(idle.samples.every(sample=>sample.process_count>0),'native root remains running throughout idle measurement');}
  const sorted=[...timings].sort((a,b)=>a-b);
  const report={event_count:stats.events,model_count:50,agent_count:20,first_screen_ms:timings,p95_ms:sorted[Math.ceil(sorted.length*.95)-1],samples:runs,idle,cancellation_checks:2,ui_cancellation:args.includes('--ui-cancel'),errors,outbound};
  assert.deepEqual(errors,[]);assert.deepEqual(outbound,[]);
  await writeFile(join(root,'result.json'),JSON.stringify(report,null,2));
  console.log(JSON.stringify({root,event_count:stats.events,samples:runs,p95_ms:report.p95_ms,idle}));
}catch(error){if(page)await page.screenshot({path:join(root,'failure.png')}).catch(()=>{});await writeFile(join(root,'failure.json'),JSON.stringify({error:String(error),errors,logs},null,2));throw error;}finally{await close();}
