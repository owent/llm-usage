// Real NSIS packages, installed executables, WebView2 IPC, SQLite and native task definitions.
import { spawn } from 'node:child_process';
import { mkdir, writeFile, readFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { once } from 'node:events';
import { DatabaseSync } from 'node:sqlite';
import { chromium } from 'playwright';
import assert from 'node:assert/strict';
import { createServer } from 'node:net';
import { spawnIsolated } from './isolated-child.mjs';
const args=process.argv.slice(2);
if(args.includes('--help')) {
 console.log('Usage: npm run test:install:windows -- --previous-installer PATH --previous-version VERSION [--installer PATH]\nRuns actual current-user NSIS installation/upgrade/rollback/uninstall/reinstall in build/install-lifecycle/windows. Requires no existing LLMUsage installation, process or startup entry.');
 process.exit(0);
}
assert.equal(process.platform,'win32');
const option=(name,fallback)=>args.includes(name)?args[args.indexOf(name)+1]:fallback;
assert.ok(option('--previous-installer'),'--previous-installer is required');
const previousVersion=option('--previous-version');
assert.ok(previousVersion,'--previous-version is required');
const currentVersion=JSON.parse(await readFile(resolve('desktop/src-tauri/tauri.conf.json'),'utf8')).version;
const root=resolve('build/install-lifecycle/windows',String(Date.now()));
const install=join(root,'含空格 install'), data=join(root,'含空格 data'), source=join(root,'source'),temp=join(root,'temp');
const label=`LLMUsageDataRefresh-lifecycle-${Date.now()}`;
await Promise.all([mkdir(data,{recursive:true}),mkdir(join(source,'.codex/sessions'),{recursive:true}),mkdir(temp,{recursive:true})]);
const old=resolve(option('--previous-installer'));
const current=resolve(option('--installer',`desktop/src-tauri/target/release/bundle/nsis/LLMUsage_${currentVersion}_x64-setup.exe`));
const exe=join(install,'LLMUsage.exe');
const env={...process.env,LLM_USAGE_INSTALL_PATH:install,LLM_USAGE_TASK_LABEL:label,LLM_USAGE_TEST_DATA:data};
const appEnv=Object.fromEntries(['PATH','SystemRoot','WINDIR','COMPUTERNAME','USERNAME','USERDOMAIN','ProgramFiles','ProgramFiles(x86)'].filter(k=>process.env[k]).map(k=>[k,process.env[k]]));
Object.assign(appEnv,{CODEX_HOME:join(source,'.codex'),TEMP:temp,TMP:temp,APPDATA:join(source,'AppData/Roaming'),LOCALAPPDATA:join(source,'AppData/Local')});
const checks=[],operations=[];
let child,browser,page;
async function control(action,extra={}) {
 const proc=spawn('pwsh.exe',['-NoProfile','-NonInteractive','-File',resolve('desktop/tests/windows-install-control.ps1'),action],{env:{...env,...extra},windowsHide:true,stdio:['ignore','pipe','pipe']});
 let out='',err='';proc.stdout.on('data',v=>out+=v);proc.stderr.on('data',v=>err+=v);
 const [code]=await once(proc,'exit');assert.equal(code,0,err||out);
 return out.trim().startsWith('{')?JSON.parse(out):out.trim();
}
async function packageRun(pkg,uninstall=false,fresh=false) {
 await new Promise(r=>setTimeout(r,1500));
 const result=await control('installer',{LLM_USAGE_INSTALLER:pkg,LLM_USAGE_UNINSTALL:uninstall?'1':'0',LLM_USAGE_FRESH_INSTALL:fresh?'1':'0'});
 operations.push({pkg,uninstall,...result});assert.equal(result.code,0);
}
function stats() {
 const conn=new DatabaseSync(join(data,'llm-usage.sqlite'),{readOnly:true});
 try {return {events:conn.prepare('SELECT COUNT(*) n FROM usage_events').get().n,tokens:conn.prepare('SELECT SUM(CAST(total_tokens AS INTEGER)) n FROM usage_events').get().n,schema:conn.prepare('PRAGMA user_version').get().user_version,task:JSON.parse(conn.prepare("SELECT value FROM settings WHERE key='background_task'").get()?.value||'{"enabled":false}').enabled};}finally{conn.close();}
}
async function scan() {
 const proc=spawnIsolated(exe,['--scan-once','--data-dir',data],{env:appEnv,windowsHide:true,stdio:['ignore','pipe','pipe']});
 let out='';proc.stdout.on('data',v=>out+=v);proc.stderr.on('data',v=>out+=v);
 const [code]=await once(proc,'exit');operations.push({scan:out,code});assert.equal(code,0,out);return stats();
}
const pause=ms=>new Promise(r=>setTimeout(r,ms));
async function launch() {
 const server=createServer();server.listen(0,'127.0.0.1');await once(server,'listening');const port=server.address().port;await new Promise(r=>server.close(r));
 child=spawnIsolated(exe,['--data-dir',data],{env:{...appEnv,WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-port=${port}`,WEBVIEW2_USER_DATA_FOLDER:join(root,'webview')},windowsHide:true,stdio:'ignore'});
 let endpoint;
 for(let n=0;n<300;n++){assert.equal(child.exitCode,null);try{endpoint=await(await fetch(`http://127.0.0.1:${port}/json/version`)).json();}catch{}if(endpoint?.webSocketDebuggerUrl)break;await pause(100);}
 assert.ok(endpoint?.webSocketDebuggerUrl);browser=await chromium.connectOverCDP(endpoint.webSocketDebuggerUrl);
 for(let n=0;n<100;n++){page=browser.contexts().flatMap(c=>c.pages()).find(p=>/tauri\.localhost/.test(p.url()));if(page)break;await pause(100);}
 assert.ok(page);await page.waitForFunction(()=>!!window.__TAURI_INTERNALS__?.invoke);await page.locator('.today-cards').waitFor();
}
async function ipc(cmd,args={}) {return page.evaluate(({cmd,args})=>window.__TAURI_INTERNALS__.invoke(cmd,args),{cmd,args});}
async function close() {if(browser){await browser.close();browser=undefined;}if(child?.exitCode===null){child.kill();await once(child,'exit');}child=undefined;page=undefined;}
let baseline,failure;
try {
 baseline=await control('snapshot');assert.equal(baseline.product,null,'Lifecycle needs an unoccupied current-user product registration');assert.equal(baseline.startup,null,'Lifecycle must preserve existing startup configuration');assert.equal(baseline.running.length,0);
  assert.equal(baseline.machineProducts.length,0,'Lifecycle must preserve existing machine-wide installations');
 await packageRun(old,false,baseline.shortcuts.length===0);const installed=await control('snapshot');assert.equal(installed.product.version,previousVersion);checks.push('old actual package installed in isolated directory');
 if(baseline.shortcuts.length===0){assert.equal(installed.shortcuts.length,2);assert.ok(installed.shortcuts.every(link=>link.target===exe));checks.push('fresh installation creates owned desktop and Start menu shortcuts');}
 else {assert.deepEqual(installed.shortcuts,baseline.shortcuts);checks.push('existing foreign shortcuts preserved in no-shortcut update mode');}
 const at=new Date().toISOString();
 const rows=[{timestamp:at,type:'session_meta',payload:{id:'install-real-binary-synthetic-input',session_id:'install-real-binary-synthetic-input',timestamp:at,originator:'codex_vscode',cli_version:'0.999.0-synthetic',model_provider:'openai',source:'vscode'}},{timestamp:at,type:'token_usage_record',payload:{thread_id:'lifecycle',turn_id:'lifecycle',session_id:'install-real-binary-synthetic-input',response_id:'lifecycle',usage:{input_tokens:10,cached_input_tokens:0,cache_write_input_tokens:0,output_tokens:5,reasoning_output_tokens:0,total_tokens:15}}}];
 await writeFile(join(source,'.codex/sessions/rollout-lifecycle.jsonl'),rows.map(JSON.stringify).join('\n')+'\n');
 assert.equal((await scan()).tokens,15);checks.push('old installed executable imports one synthetic observation');
 await pause(1500);await packageRun(current);assert.equal((await control('snapshot')).product.version,currentVersion);assert.equal((await scan()).tokens,15);assert.equal(stats().events,1);checks.push('actual upgrade retains data and deduplication');
 await launch();
 const isolatedSettings=await ipc('get_settings');isolatedSettings.refresh_interval_secs=0;
 isolatedSettings.manual_roots_only=true;isolatedSettings.manual_roots=[join(source,'.codex')];
 await ipc('set_settings',{settings:isolatedSettings});
 await ipc('set_refresh_task',{install:true});await ipc('set_auto_start',{enabled:true});
 const enabled=await ipc('system_task_status');assert.equal(enabled.refresh_task,true);assert.equal(enabled.auto_start,true);await close();
 await control('foreign-create');checks.push('native IPC registers refresh task and startup');
 await packageRun(old);assert.equal((await control('snapshot')).product.version,previousVersion);assert.equal((await scan()).tokens,15);await launch();assert.equal((await ipc('system_task_status')).refresh_task,true);await close();checks.push('actual binary rollback reads newer data and keeps native task');
 await packageRun(current);assert.equal((await scan()).tokens,15);await launch();assert.equal((await ipc('system_task_status')).refresh_task,true);await close();checks.push('second upgrade preserves consent and installed executable path');
 operations.push({before_uninstall:await control('snapshot')});await packageRun(join(install,'uninstall.exe'),true);
 const after=await control('snapshot');operations.push({after_uninstall:after,stats:stats()});assert.equal(after.product,null);assert.equal(after.startup,null);assert.deepEqual(after.shortcuts,baseline.shortcuts);assert.equal(stats().tokens,15);assert.equal(after.tasks.length,1,'Uninstall must remove owned scheduler tasks and preserve foreign task');assert.equal(after.tasks[0].name,label);assert.equal(stats().task,false);checks.push('uninstall removes owned integration and shortcuts, retains data and decoy');
 await packageRun(current,false,baseline.shortcuts.length===0);assert.equal((await scan()).tokens,15);await launch();const reset=await ipc('system_task_status');assert.equal(reset.refresh_task,false);assert.equal(reset.auto_start,false);checks.push('reinstall keeps data, remains opted out');
 await ipc('set_refresh_task',{install:true});
 const cleanup=spawnIsolated(exe,['--uninstall-cleanup'],{env:appEnv,windowsHide:true,stdio:['ignore','pipe','pipe']});
 const [cleanupCode]=await once(cleanup,'exit');assert.equal(cleanupCode,1);assert.equal((await ipc('system_task_status')).refresh_task,true);assert.equal(stats().task,true);checks.push('cleanup refuses running writer without removing task or consent');
 const refused=await control('installer',{LLM_USAGE_INSTALLER:join(install,'uninstall.exe'),LLM_USAGE_UNINSTALL:'1'});
 operations.push({refused_uninstall:refused});assert.notEqual(refused.code,0);assert.equal((await control('snapshot')).product.version,currentVersion);
 assert.equal((await ipc('system_task_status')).refresh_task,true);await close();checks.push('NSIS aborts failed cleanup and keeps executable and task for retry');
 await control('foreign-startup-create');await packageRun(join(install,'uninstall.exe'),true);
 const foreign=await control('snapshot');assert.equal(foreign.product,null);assert.equal(foreign.startup,`"${install}\\foreign-LLMUsage.exe"`);assert.equal(foreign.tasks.length,1);assert.equal(stats().task,false);checks.push('uninstaller preserves foreign startup while removing owned task');
}catch(error){failure=String(error.stack||error);console.error(failure);process.exitCode=1;}
finally {
 await close();await control('cleanup-test');
 const state=await control('snapshot');if(state.product?.location?.replace(/^"|"$/g,'').replace(/[\\/]$/,'')===install)await packageRun(join(install,'uninstall.exe'),true);
 await writeFile(join(root,'result.json'),JSON.stringify({root,previousVersion,currentVersion,baseline,checks,operations,failure,cleanup:await control('snapshot')},null,2));
 console.log(JSON.stringify({root,checks:checks.length,failure}));
}
