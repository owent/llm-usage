// Real Windows release/IPC/HTTP/vault lifecycle; synthetic installations and exports.
// Tokens stay in this process and owned config files; reports contain no credentials.
import { chromium } from 'playwright';
import { spawnIsolated } from './isolated-child.mjs';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { resolve, join, delimiter } from 'node:path';
import { createServer } from 'node:net';
import { freeHttpPort } from './http-test-port.mjs';
import { once } from 'node:events';
import assert from 'node:assert/strict';
import { DatabaseSync } from 'node:sqlite';

assert.equal(process.platform, 'win32');
const root=resolve('build/plan-continuation/native-receiver',String(Date.now()));
const data=join(root,'data'),home=join(root,'source-home'),bin=join(root,'launchers');
await Promise.all([data,home,bin,join(root,'temp'),join(home,'.codex'),join(home,'.claude'),join(home,'.codebuddy'),join(bin,'node_modules/@tencent-ai/codebuddy-code')].map(p=>mkdir(p,{recursive:true})));
for(const command of ['codex','claude','codebuddy']) await writeFile(join(bin,`${command}.cmd`),'@exit /b 99\r\n');
await writeFile(join(bin,'node_modules/@tencent-ai/codebuddy-code/package.json'),JSON.stringify({name:'@tencent-ai/codebuddy-code',version:'2.98.0'}));
const originalCodex='# synthetic local configuration\n[otel.exporter.otlp-http]\nprotocol="binary"\nheaders={ "x-user-key"="OWNED_SENTINEL" }\n';
const codexConfig=join(home,'.codex/config.toml'),claudeConfig=join(home,'.claude/settings.json'),buddyConfig=join(home,'.codebuddy/settings.json');
await writeFile(codexConfig,originalCodex); await writeFile(claudeConfig,'{}');await writeFile(buddyConfig,'{}');
const env=Object.fromEntries(['PATH','SystemRoot','WINDIR','COMPUTERNAME','USERNAME','USERDOMAIN','ProgramFiles','ProgramFiles(x86)'].filter(k=>process.env[k]).map(k=>[k,process.env[k]]));
Object.assign(env,{PATH:bin+delimiter+env.PATH,HOME:home,CODEX_HOME:join(home,'.codex'),CLAUDE_CONFIG_DIR:join(home,'.claude'),APPDATA:join(root,'config'),LOCALAPPDATA:join(root,'local'),TEMP:join(root,'temp'),TMP:join(root,'temp')});
let child,browser,page,occupied,step='startup';
const plans=[],headers={},checks=[],errors=[],outbound=[];
const pause=ms=>new Promise(r=>setTimeout(r,ms));
async function freePort(){return freeHttpPort();}
async function invoke(cmd,args={}){return page.evaluate(({cmd,args})=>window.__TAURI_INTERNALS__.invoke(cmd,args),{cmd,args});}
let receiverPort;
async function close(){if(browser){await browser.close();browser=undefined;}if(child&&child.exitCode===null){child.kill();await once(child,'exit');}child=undefined;page=undefined;}
const payload=name=>JSON.stringify({resourceLogs:[{scopeLogs:[{logRecords:[{eventName:name,timeUnixNano:'1790899200000000000',attributes:[{key:'input_tokens',value:{intValue:'42'}},{key:'api_key',value:{stringValue:'REJECT_SENTINEL'}}]}]}]}]});
async function send(header,name='codex.api_request',path='/v1/logs'){
  const response=await fetch(`http://127.0.0.1:${receiverPort}${path}`,{method:'POST',headers:{'content-type':'application/json',...(header?{Authorization:header}:{})},body:payload(name),signal:AbortSignal.timeout(5000)});
  await response.text();return response.status;
}
function varint(value){let n=BigInt(value);const bytes=[];do{const byte=Number(n&127n);n>>=7n;bytes.push(byte|(n?128:0));}while(n);return Buffer.from(bytes);}
function field(number,value){const bytes=Buffer.from(value);return Buffer.concat([varint((number<<3)|2),varint(bytes.length),bytes]);}
function attribute(key,value){return Buffer.concat([field(1,key),field(2,value)]);}
async function sendBuddy(header,path='/v1/traces/supplemental'){
  const time=Buffer.alloc(8);time.writeBigUInt64LE(1790899200000000000n);
  const usage=attribute('usage.input_tokens',Buffer.concat([varint(3<<3),varint(42)]));
  const span=Buffer.concat([field(1,Buffer.alloc(16,3)),field(2,Buffer.alloc(8,2)),field(5,'model_stream'),varint(6<<3),varint(3),varint((7<<3)|1),time,field(9,usage)]);
  const resource=field(1,attribute('service.name',field(1,'codebuddy-code')));
  const payload=field(1,Buffer.concat([field(1,resource),field(2,field(2,span))]));
  const response=await fetch(`http://127.0.0.1:${receiverPort}${path}`,{method:'POST',headers:{'content-type':'application/x-protobuf',Authorization:header},body:payload,signal:AbortSignal.timeout(5000)});
  await response.text();return response.status;
}
async function launch(){
  const cdp=await freePort();
  child=spawnIsolated(resolve('desktop/src-tauri/target/release/LLMUsage.exe'),['--data-dir',data],{env:{...env,WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-port=${cdp}`,WEBVIEW2_USER_DATA_FOLDER:join(root,'webview')},stdio:'ignore',windowsHide:true});
  let endpoint;
  for(let n=0;n<300;n++){assert.equal(child.exitCode,null);try{endpoint=await(await fetch(`http://127.0.0.1:${cdp}/json/version`)).json();}catch{}if(endpoint?.webSocketDebuggerUrl)break;await pause(100);}
  assert.ok(endpoint?.webSocketDebuggerUrl);
  step='cdp_connect';browser=await chromium.connectOverCDP(endpoint.webSocketDebuggerUrl);
  for(let n=0;n<100;n++){page=browser.contexts().flatMap(c=>c.pages()).find(p=>/tauri\.localhost/.test(p.url()));if(page)break;await pause(100);}
  step='window_ready';assert.ok(page);await page.waitForFunction(()=>Boolean(window.__TAURI_INTERNALS__?.invoke));
  page.on('pageerror',()=>errors.push('page_error'));
  page.on('request',r=>{if(/^https?:/.test(r.url())&&!/\/\/(tauri\.localhost|ipc\.localhost|localhost|127\.0\.0\.1)([:/]|$)/.test(r.url()))outbound.push('external_request');});
}
async function seedPort(port){
  await close();
  const db=new DatabaseSync(join(data,'llm-usage.sqlite'));
  try{
    const row=db.prepare("SELECT key,value FROM settings WHERE key='app_settings'").get();
    assert.ok(row,'actual app settings row exists');
    const settings=JSON.parse(row.value);settings.otel_receiver_port=port;
    db.prepare('UPDATE settings SET value=? WHERE key=?').run(JSON.stringify(settings),row.key);
  }finally{db.close();}
  await launch();
}
try{
  await launch();step='settings_read';
  let settings=await invoke('get_settings');
  settings.refresh_interval_secs=0;settings.manual_roots_only=true;settings.manual_roots=[];
  occupied=createServer();occupied.listen(0,'127.0.0.1');await once(occupied,'listening');
  step='settings_init';await invoke('set_settings',{settings});
  step='seed_occupied_port';await seedPort(occupied.address().port);
  step='bind_failure';
  const blocked=await invoke('telemetry_preview',{id:'codex'});
  const blockedPlan={id:'codex',token:blocked.token,applied:false};plans.push(blockedPlan);
  assert.ok(!JSON.stringify(blocked).includes('OWNED_SENTINEL'),'preview must redact retained headers');
  let failed=false;try{await invoke('telemetry_apply',{token:blocked.token});blockedPlan.applied=true;}catch(e){failed=String(e).includes('receiver_bind_failed');}
  assert.ok(failed);assert.ok((await readFile(codexConfig,'utf8'))===originalCodex);
  assert.equal((await invoke('get_settings')).otel_receiver_enabled,false);
  checks.push('binding failure preserves config and receiver intent');
  await new Promise(r=>occupied.close(r));occupied=undefined;
  receiverPort=await freePort();step='seed_available_port';await seedPort(receiverPort);
  for(const id of ['codex','claude','codebuddy']){
    step=`configure_${id}`;
    const preview=await invoke('telemetry_preview',{id});plans.push({id,token:preview.token,applied:false});
    assert.ok(JSON.stringify(preview).includes('<generated-on-apply>'));
    assert.ok(!JSON.stringify(preview).includes('OWNED_SENTINEL'));
    await invoke('telemetry_apply',{token:preview.token});plans.at(-1).applied=true;
    if(id==='codex'){
      const config=await readFile(codexConfig,'utf8');
      assert.ok(config.includes('OWNED_SENTINEL'),'unrelated headers retained');
      const match=config.match(/Authorization\s*=\s*"(Bearer [0-9a-f]{32}\.[0-9a-f]{64})"/);
      assert.ok(match,'generated header exists only in local config');headers.codex=match[1];
    }else{
      const config=JSON.parse(await readFile(id==='claude'?claudeConfig:buddyConfig,'utf8'));
      const key=id==='claude'?'OTEL_EXPORTER_OTLP_LOGS_HEADERS':'OTEL_EXPORTER_OTLP_HEADERS';
      headers[id]=decodeURIComponent(config.env[key].split(',').find(v=>v.startsWith('Authorization=')).slice('Authorization='.length));
    }
    assert.equal((await invoke('telemetry_check')).find(r=>r.id===id).status,'configured');
    checks.push(`${id}: real IPC config, native vault validation and hidden preview`);
  }
  step='http_authentication';
  assert.equal(await send(),401);assert.equal(await send(headers.codex,'codex.api_request','/v1/traces'),401);
  assert.equal(await send(headers.codex),200);assert.equal(await send(headers.claude,'claude_code.api_request'),200);
  const output=join(data,'telemetry/otlp-logs.jsonl');
  const saved=await readFile(output,'utf8');assert.equal(saved.trim().split('\n').length,2);
  assert.ok(!saved.includes('REJECT_SENTINEL')&&!saved.includes(headers.codex)&&!saved.includes(headers.claude),'credentials are excluded from records');
  checks.push('actual authenticated HTTP isolates routes and persists only usage fields');
  assert.equal(await sendBuddy(headers.codebuddy),200);assert.equal(await sendBuddy(headers.codebuddy,'/v1/traces'),401);
  assert.equal(await send(headers.codebuddy),401);
  const supplemental=await readFile(join(data,'telemetry/otlp-traces.jsonl'),'utf8');
  assert.equal(supplemental.trim().split('\n').length,1);assert.ok(!supplemental.includes(headers.codebuddy));
  checks.push('verified CodeBuddy version: protobuf traces stay in supplemental output, never logs or primary spans');
  step='undo_codex';
  const codex=plans.find(p=>p.id==='codex'&&p.applied);await invoke('telemetry_undo',{token:codex.token});codex.applied=false;
  assert.ok((await readFile(codexConfig,'utf8'))===originalCodex);
  assert.equal(await send(headers.codex),401);assert.equal(await send(headers.claude,'claude_code.api_request'),200);
  checks.push('undo revokes one source immediately while another source still works');
  step='undo_codebuddy';
  const buddy=plans.find(p=>p.id==='codebuddy'&&p.applied);await invoke('telemetry_undo',{token:buddy.token});buddy.applied=false;
  assert.ok((await readFile(buddyConfig,'utf8'))==='{}');assert.equal(await sendBuddy(headers.codebuddy),401);
  step='undo_claude';
  const claude=plans.find(p=>p.id==='claude');await invoke('telemetry_undo',{token:claude.token});claude.applied=false;
  assert.ok((await readFile(claudeConfig,'utf8'))==='{}');assert.equal(await send(headers.claude,'claude_code.api_request'),401);
  checks.push('all three owned credentials revoked and exact original configs restored');
  assert.deepEqual(errors,[]);assert.deepEqual(outbound,[]);
  await writeFile(join(root,'result.json'),JSON.stringify({checks,errors,outbound,exporter:'synthetic',vault:'native Windows',ipc:'real release',credentials_remaining:0},null,2));
  console.log(JSON.stringify({root,checks:checks.length,credentials_remaining:0}));
}catch(error){
  const classification=error instanceof Error?error.name:'ipc_error';
  await writeFile(join(root,'failure.json'),JSON.stringify({step,classification,child_exit_code:child?.exitCode,errors,outbound}));
  throw new Error(`Native receiver validation failed at ${step}; report omits credentials`);
}finally{
  if(page){for(const plan of plans.filter(p=>p.applied)){try{await invoke('telemetry_undo',{token:plan.token});}catch{if(headers[plan.id])assert.equal(await send(headers[plan.id]),401,'failed config restore must still revoke the owned credential');}}}
  if(occupied)await new Promise(r=>occupied.close(r));
  await close();
}
