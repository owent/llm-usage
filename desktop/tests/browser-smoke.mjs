import { chromium } from 'playwright';
import { writeFile, mkdir } from 'node:fs/promises';
import { resolve } from 'node:path';
import { spawn } from 'node:child_process';
import assert from 'node:assert/strict';

const out = resolve('build/browser-smoke') + '/';
await mkdir(out, {recursive:true});
const server = spawn(process.execPath, [resolve('desktop/node_modules/vite/bin/vite.js'), '--host', '127.0.0.1', '--port', '1421', '--strictPort'], {cwd:resolve('desktop'),stdio:['ignore','pipe','pipe'],windowsHide:true});
let browser;
let testPage;
const errors = [];
const browserConsole = [];
const serverMessages = [];
server.stdout.on('data',(chunk)=>serverMessages.push(String(chunk)));
server.stderr.on('data',(chunk)=>serverMessages.push(String(chunk)));
try {
  let available = false;
  for (let i=0;i<600;i++) {
    if (server.exitCode !== null) throw new Error('Vite test server exited before startup');
    try { available = (await fetch('http://127.0.0.1:1421')).ok; } catch {}
    if (available) break;
    await new Promise(r=>setTimeout(r,100));
  }
  assert.ok(available, 'test server started');
  browser = await chromium.launch({ channel: process.platform === 'win32' ? 'msedge' : undefined, headless: true });
const context = await browser.newContext({ viewport: { width: 1440, height: 1100 }, timezoneId: 'America/Los_Angeles', colorScheme: 'light' });
const page = await context.newPage();
testPage = page;
page.on('pageerror', (e) => errors.push(e.message));
page.on('console', (message) => { if (message.type() === 'error') browserConsole.push(message.text()); });
await page.clock.install({ time: new Date('2026-09-27T23:30:00Z') });
await page.addInitScript(() => {
  const stats = (n=1) => ({ input_total_known: String(125000*n), uncached_known: String(25000*n), cache_read_known: String(100000*n), cache_write_known: '0', output_total_known: String(8000*n), total_tokens_known: String(133000*n), input_known_count: 40*n, input_unknown_count: 2*n, output_known_count: 42*n, output_unknown_count: 0, total_known_count: 40*n, total_unknown_count: 2*n, event_count: 42*n, call_count: 42*n, attempt_count: 0, conflict_count: 0, cache_input_ratio: .8, avg_duration_ms:'4250', total_duration_ms:String(170000*n), duration_sample_count: 40*n });
  let settings = { timezone: 'Asia/Shanghai', week_start: 0, retention: { events_days: 7, hourly_days: 3, daily_days: 90, weekly_days: 1095, monthly_days: 3650, yearly_days: null }, refresh_interval_secs: 300, language: 'zh-CN', theme: 'light', manual_roots: [], hostname_alias: null, pricing: {enabled:true,provider_defaults:[],online_refresh_enabled:false} };
  let current = 'default';
  const users = [{user_id:'default',name:'本机用户',created_at_ms:0},{user_id:'empty',name:'新用户',created_at_ms:0}];
  const owners = {};
  let revision = 5;
  let finished = 1790550000000;
  window.appCalls = [];
  window.detailTotal = 123;
  const telemetryTarget = {id:'copilot-vscode',name:'Copilot · Code',config_path:'C:/Users/local/Code/User/settings.json',output_path:'C:/Users/local/telemetry/events.jsonl',status:'missing',reason:'',configurable:true,kind:'jsonc',docs_url:'https://code.visualstudio.com/docs/agents/guides/monitoring-agents'};
  const telemetryTargets = [telemetryTarget,
    {...telemetryTarget,id:'qwen',name:'Qwen Code',config_path:'C:/Users/local/.qwen/settings.json'},
    {...telemetryTarget,id:'codex',name:'Codex',kind:'toml',config_path:'C:/Users/local/.codex/config.toml'},
    {...telemetryTarget,id:'claude',name:'Claude Code',status:'blocked',reason:'managed_policy',configurable:false,config_path:'C:/Users/local/.claude/settings.json'},
    {...telemetryTarget,id:'gemini',name:'Gemini CLI',status:'configured',configurable:false,config_path:'C:/Users/local/.gemini/settings.json'}];
  const telemetryConfigured = new Set(['gemini']);
  let failQwenPreview = true;
  window.holdTelemetryApply = true;
  // 事件订阅（clear-all-progress 等）：transformCallback 注册回调，
  // plugin:event|listen/unlisten 登记；emitLater 供 mock 命令推送阶段事件。
  let cbSeq = 0;
  const tauriCallbacks = new Map();
  const eventListeners = {};
  const emitLater = (event, payload, delay) => setTimeout(() => {
    for (const id of eventListeners[event] ?? []) tauriCallbacks.get(id)?.({ event, id, payload });
  }, delay);
  window.__TAURI_INTERNALS__ = {
    transformCallback: (callback) => { const id = ++cbSeq; tauriCallbacks.set(id, callback); return id; },
  };
  // @tauri-apps/api 2.12 的 listen/unlisten 需要的插件内部对象（冒烟环境无真实运行时）。
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
  window.__TAURI_INTERNALS__.invoke = async (cmd, args={}) => {
    window.appCalls.push({cmd,args});
    const q=args.q;
    if(cmd==='plugin:event|listen') { (eventListeners[args.event] ??= []).push(args.handler); return; }
    if(cmd==='plugin:event|unlisten') return;
    if(cmd==='get_settings') return settings;
    if(cmd==='set_settings') { settings=args.settings; return; }
    if(cmd==='list_users') return {users,current};
    if(cmd==='set_current_user') {current=args.userId;return;}
    if(cmd==='summary') {
      const empty = current==='empty';
      const models = ['gpt-5.4','claude-sonnet-4.6','glm-5.3'];
      const selected = q.models?.[0];
      if(selected==='gpt-5.4') await new Promise(r=>setTimeout(r,900));
      const n=empty?0:q.first_period?(q.first_period==='2026-09-19'?3:2):selected==='gpt-5.4'?1:selected==='glm-5.3'?3:owners['codex@C:/Users/local/.codex']==='empty'?11:12;
      if (q.first_period && window.selectionDelay) await new Promise(r=>setTimeout(r,window.selectionDelay));
      return { data_revision:revision, timezone:settings.timezone, totals:stats(n), distinct_sessions:empty?0:27, active_days:empty?0:12,
        periods:empty?[]:Array.from({length:q.granularity==='hour'?12:q.first_day===q.last_day?1:14},(_,i)=>{
          const day=q.first_day===q.last_day?q.last_day:`2026-09-${String(15+i).padStart(2,'0')}`;
          return {label:q.granularity==='hour'?`${q.last_day} ${String(8+i).padStart(2,'0')}:00`:day,start_day:q.granularity==='hour'?q.last_day:day,end_day:q.granularity==='hour'?q.last_day:day,in_progress:i===13,partial_history:i<5,sums:stats(i%4+1),distinct_sessions:5+i,active_days:1};
        }),
        models:empty?[]:models.filter(m=>!selected||selected===m).map((m,i)=>({model:m,provider:['openai','anthropic','zhipu'][i],sums:stats(6-i)})),
        agents:empty?[]:[...['codex','claude','zcode'].filter(a=>!q.agents?.length||q.agents.includes(a)).map((agent,i)=>({agent,sums:stats(6-i)})),
          ...(!q.agents?.length||q.agents.includes('vscode-copilot-chat')?[{agent:'vscode-copilot-chat',sums:{...stats(1),call_count:0,total_tokens_known:null,input_total_known:'300000',output_total_known:'1234'}}]:[])],
        today_hourly:empty?[]:Array.from({length:12},(_,i)=>({hour:8+i,calls:12+i*3,total_tokens:String((i+1)*150000),input_total:String((i+1)*120000),cache_read:String(i*80000),output_total:String((i+1)*30000),sessions:5,avg_duration_ms:'4250'})),excluded_event_count:0 };
    }
    if(cmd==='list_sources') return {sources:['codex','claude','zcode','opencode','gemini','kimi-code'].map((agent,i)=>({instance_id:`${agent}@C:/Users/local/.${agent}`,agent,format:'local',health:i===5||i===2?'degraded':'ok',enabled:i!==4,origin_host_id:'local',user_id:owners[`${agent}@C:/Users/local/.${agent}`]??'default',last_success_ms:finished,compat_files:i===3?1:0,degraded_files:i===5?1:0,unsupported_files:i===2?1:0,incompatible_files:0,missing_files:i===0?2:0}))};
    if(cmd==='refresh_status') return {running:false,started_ms:finished-1000,last_finished_ms:finished,trigger:'manual',progress_percent:100,eta_seconds:null,completed_adapters:[],instances:[]};
    if(cmd==='refresh_sources') {revision++;finished++;return {started:true,running:false,last_finished_ms:finished};}
    if(cmd==='cost_summary') {
      if(q.first_period && window.selectionDelay) await new Promise(resolve=>setTimeout(resolve,window.selectionDelay));
      const row={currency:'USD',total_amount_minor:1234,priced_tokens:100000,known_tokens:200000,priced_event_count:10,unpriced_event_count:0,partial_event_count:2,ttl_defaulted_events:0,fallback_event_count:8};
      const mode={rows:[row],unpriced_reasons:{},as_of_ms:finished,detail_limited:false};
      const models=['gpt-5.4','claude-sonnet-4.6','glm-5.3'].map((model,i)=>({model,provider:['openai','anthropic','zhipu'][i],at_time:[{...row,total_amount_minor:[600,400,234][i],priced_event_count:[4,3,3][i]}],current_sim:[{...row,total_amount_minor:(q.first_period?[200,200,100]:[1200,800,468])[i],priced_event_count:[4,3,3][i]}],
        unit_prices:i===2?[]:[{price_id:`mock-${i}`,snapshot_id:'Synthetic unit prices',provider_id:['openai','anthropic'][i],model,currency:'USD',region:'global',channel:'api',service_tier:'standard',context_threshold_tokens:i===1?200000:0,input_per_mtok_hundredths:30000,output_per_mtok_hundredths:150000,cache_read_per_mtok_hundredths:3000,cache_write_5m_per_mtok_hundredths:37500,cache_write_1h_per_mtok_hundredths:null}]}));
      const day=(i)=>q.first_day===q.last_day?q.last_day:`2026-09-${15+i}`;
      return {at_time:mode,current_sim:{...mode,rows:[{...row,total_amount_minor:q.first_period?500:2468}]},source_amounts:[],price_basis:['Synthetic official reference'],data_revision:revision,models,
        daily:models.map((model,i)=>({day:day(i),provider:model.provider,model:model.model,sums:model.at_time[0]})),
        daily_current:models.map((model,i)=>({day:day(i),provider:model.provider,model:model.model,sums:model.current_sim[0]}))};
    }
    if(cmd==='chart_series') return {rows:Array.from({length:window.singleChartPeriod?1:12},(_,i)=>{
      const label=q.granularity==='hour'?`${q.last_day} ${String(8+i).padStart(2,'0')}:00`:`2026-09-${String(15+i).padStart(2,'0')}`;
      return [{label,series:args.dimension==='agent'?'codex':'gpt-5.4',calls:10+i,input:String(100000*(i+1)),cache_read:String(80000*(i+1)),cache_write:'0',uncached:String(20000*(i+1)),cache_ratio:.8,output:'15000',total:String(115000*(i+1))},
        {label,series:'vscode-copilot-chat',calls:i===0?0:2,input:String(300000+i),cache_read:null,cache_write:null,uncached:null,cache_ratio:null,output:i===0?'0':'1234',total:null}];
    }).flat()};
    if(cmd==='telemetry_check') return telemetryTargets.map(row=>{
      const hasData = row.status==='blocked' ? window.telemetryBlockedHasData
        : telemetryConfigured.has(row.id) && row.id==='copilot-vscode' && window.telemetryHasData;
      return {...row, ...(row.status==='blocked'?{}:{status:telemetryConfigured.has(row.id)?'configured':'missing',configurable:!telemetryConfigured.has(row.id)}),
        verification:hasData?'verified':'waiting',verified_records:hasData?30:0};
    });
    if(cmd==='telemetry_preview') {
      if(args.id==='qwen'&&failQwenPreview) {failQwenPreview=false;throw 'config_changed';}
      const row=telemetryTargets.find(row=>row.id===args.id);
      if(!row) throw 'not_installed';
      return {token:'mock-preview-'+args.id,target:{...row,config_path:args.id==='copilot-vscode'?'C:/Users/local/Code/User/profiles/local/settings.json':row.config_path},keys:['github.copilot.chat.otel.enabled'],changes:[['github.copilot.chat.otel.enabled',true]],receiver:false,sync_config_path:args.id==='copilot-vscode'?telemetryTarget.config_path:null,sync_changes:args.id==='copilot-vscode'?[['settingsSync.ignoredSettings',['github.copilot.chat.otel.outfile']]]:[]};
    }
    if(cmd==='telemetry_apply') {
      if(args.token==='mock-preview-copilot-vscode' && window.holdTelemetryApply) {
        await new Promise(resolve=>{ window.releaseTelemetryApply=resolve; });
      }
      telemetryConfigured.add(args.token.slice('mock-preview-'.length));return;
    }
    if(cmd==='telemetry_undo') {telemetryConfigured.delete(args.token.slice('mock-preview-'.length));return [];}
    if(cmd==='heatmap') {
      const first = Date.parse(q.first_day+'T00:00:00Z');
      const last = Date.parse(q.last_day+'T00:00:00Z');
      return {cells:Array.from({length:(last-first)/86400000+1},(_,i)=>{
        const day = new Date(first+i*86400000);
        return {day:day.toISOString().slice(0,10),weekday:(day.getUTCDay()+6)%7+1,calls:current==='empty'?0:(i*7)%40,total_tokens:current==='empty'?null:String(i*10000),available:i!==0,partial:i===1};
      })};
    }
    if(cmd==='event_details') return {total:window.detailTotal,page:args.page,page_size:args.pageSize,rows:Array.from({length:Math.max(0,Math.min(args.pageSize,window.detailTotal-args.page*args.pageSize))},(_,i)=>({event_id:String(i),agent:'codex',model:'gpt-5.4',category:'primary',occurred_at_ms:1790550000000-i*100000,session:`session-${i}`,input:'154000',cache_read:'125000',output:'5240',total:'159240',duration_ms:'4530',lifecycle:'final'}))};
    if(cmd==='app_info') return {schema_version:7,data_revision:revision,db_path:'local/usage.sqlite',host_id:'local',exchange_format_version:'llm-usage-exchange-1'};
    if(cmd==='system_task_status') return {platform:'windows',auto_start:false,refresh_task:false};
    if(cmd==='storage_stats') return {events:10000,hourly:300,daily:500,period:45,diagnostics:3,db_bytes:5800000,wal_bytes:0};
    if(cmd==='export_filter_options') return {users:[{user_id:'default',name:'本机用户',is_current:true}],hosts:[{host_id:'local',hostname:'Local',is_current:true}]};
    if(cmd==='diagnostic_logs') return {rows:[]};
    if(cmd==='quota_summary') return {quotas:[{agent:'copilot',quota_id:'premium_interactions',kind:'rate_limit',unit:'milli_requests',limit_value:100000,used:50000,remaining:50000,percent_remaining:50,locality_verified:false,observed_at_ms:1790550000000}]};
    if(cmd==='quota_series') return {agent:args?.agent??'copilot',quota_id:args?.quotaId??'premium_interactions',points:[]};
    if(cmd==='assign_source_user') {owners[args.instanceId]=args.userId;return;}
    if(cmd==='set_source_enabled') return;
    if(cmd==='clear_all_preview') return {event_count:42, missing_files:1, total_files:7};
    if(cmd==='clear_all_data') {
      // 后台任务阶段事件序列（fake clock 驱动）：waiting→backup→clearing→cleared→rescan→done。
      emitLater('clear-all-progress', {phase:'waiting'}, 0);
      emitLater('clear-all-progress', {phase:'backup'}, 200);
      emitLater('clear-all-progress', {phase:'clearing'}, 600);
      emitLater('clear-all-progress', {phase:'cleared', cleared:{usage_events:42, diagnostics:3}, data_revision:6, backup:null}, 1000);
      emitLater('clear-all-progress', {phase:'rescan'}, 1100);
      emitLater('clear-all-progress', {phase:'done', rescan_started:true}, 1700);
      return {started:true};
    }
    throw new Error(`Unhandled command ${cmd}`);
  };
});
await page.goto('http://127.0.0.1:1421');
await page.waitForSelector('.today-cards .value');
await page.clock.runFor(800);
assert.equal(await page.locator('.quota-card .used').textContent(),'50','fractional quota units are displayed as requests');
const quotaTime = await page.evaluate(() => new Intl.DateTimeFormat('zh-CN',{timeZone:'Asia/Shanghai',dateStyle:'short',timeStyle:'short'}).format(new Date(1790550000000)));
assert.ok((await page.locator('.quota-card .at').textContent()).includes(quotaTime),'quota time follows the statistics timezone instead of browser timezone');
assert.equal(await page.evaluate(()=>window.appCalls.filter(c=>c.cmd==='telemetry_check').length),1,'background telemetry discovery runs once');
assert.equal(await page.evaluate(()=>window.appCalls.filter(c=>c.cmd==='telemetry_apply').length),0,'background check never configures an Agent');
const overviewTelemetry=page.locator('.telemetry-setup.compact');
assert.equal(await overviewTelemetry.locator('button').count(),2,'overview contains only enable-all and details actions');
assert.equal(await overviewTelemetry.locator('.target,.path,.preview').count(),0,'overview does not enumerate Agents or configuration details');
assert.match(await overviewTelemetry.textContent(),/3 待开启[\s\S]*2 暂无数据[\s\S]*0 已核验/);
assert.doesNotMatch(await overviewTelemetry.textContent(),/配置受限/,'targets without verified data are reported as no data in the overview');
assert.ok((await overviewTelemetry.boundingBox()).height < 70,'overview telemetry fits one compact row');
assert.match(await overviewTelemetry.locator('h4').getAttribute('title'),/Copilot[\s\S]*CodeBuddy/,'exporter guidance is available without filling the overview');
await overviewTelemetry.getByRole('button',{name:'一键开启全部',exact:true}).click();
assert.equal(await overviewTelemetry.getByRole('button',{name:'一键开启全部',exact:true}).isDisabled(),true,'duplicate bulk clicks are blocked');
await overviewTelemetry.getByRole('button',{name:'查看详情',exact:true}).click();
assert.equal(await page.locator('[data-settings-section="telemetry"].active').count(),1,'details jumps directly to the telemetry settings section');
assert.equal(await page.locator('#telemetry-settings').evaluate(el=>el===document.activeElement),true,'the destination panel receives keyboard focus');
assert.equal(await page.locator('#telemetry-settings').getByRole('button',{name:'一键开启全部',exact:true}).isDisabled(),true,'batch busy state is shared across pages');
await page.waitForFunction(()=>typeof window.releaseTelemetryApply==='function');
await page.evaluate(()=>{ window.holdTelemetryApply=false; window.releaseTelemetryApply(); });
await page.clock.runFor(600);
assert.equal(await page.locator('#telemetry-settings .target').count(),5,'details lists only the installed Agents returned by discovery');
assert.equal(await page.locator('[data-telemetry-id="claude"] .status').textContent(),'配置受限','details retain the real policy restriction when there is no data');
assert.match(await page.locator('[data-telemetry-id="claude"] .data-state').textContent(),/尚未收到导出数据/,'configuration and data states remain separate in details');
assert.equal(await page.locator('[data-telemetry-id="codebuddy"]').count(),0,'uninstalled Agent templates are not shown');
assert.match(await page.locator('#telemetry-settings').textContent(),/1 待开启[\s\S]*3 等待数据[\s\S]*0 已核验[\s\S]*1 配置受限/,'configured exports are waiting for data rather than pending setup');
assert.ok(await page.locator('[data-telemetry-id="qwen"] .error').count(),'partial failure remains visible on its target');
assert.equal(await page.evaluate(()=>window.appCalls.filter(c=>c.cmd==='telemetry_preview'&&['gemini','claude'].includes(c.args.id)).length),0,'configured outputs and managed Agents are skipped');
await page.locator('#telemetry-settings').getByRole('button',{name:'一键开启全部',exact:true}).click();
await page.clock.runFor(500);
assert.match(await page.locator('#telemetry-settings').textContent(),/0 待开启[\s\S]*4 等待数据[\s\S]*1 配置受限/,'retry clears pending configuration without pretending data exists');
await page.evaluate(()=>window.telemetryHasData=true);
await page.clock.runFor(30_500);
assert.match(await page.locator('[data-telemetry-id="copilot-vscode"] .data-state').textContent(),/30/,'newly produced data is verified automatically');
assert.deepEqual(await page.evaluate(()=>window.appCalls.filter(c=>c.cmd==='telemetry_apply').map(c=>c.args.token)),['mock-preview-copilot-vscode','mock-preview-codex','mock-preview-qwen']);
await page.locator('[data-telemetry-id="copilot-vscode"]').getByRole('button',{name:'撤销本次配置',exact:true}).click();
await page.clock.runFor(300);
await page.locator('[data-telemetry-id="copilot-vscode"]').getByRole('button',{name:'一键配置',exact:true}).click();
assert.match(await page.locator('.telemetry-setup .preview').textContent(),/github.copilot.chat.otel.enabled = true/,'preview shows only prospective setup values');
assert.match(await page.locator('.telemetry-setup .preview').textContent(),/profiles\/local\/settings.json[\s\S]*Code\/User\/settings.json[\s\S]*settingsSync.ignoredSettings = \["github.copilot.chat.otel.outfile"\]/,'profile preview names both modified files and the application-wide sync exclusion');
await page.getByRole('button',{name:'应用配置',exact:true}).click();
await page.clock.runFor(300);
assert.match(await page.locator('[data-telemetry-id="copilot-vscode"]').textContent(),/用户层已配置/);
await page.locator('[data-telemetry-id="copilot-vscode"]').getByRole('button',{name:'撤销本次配置',exact:true}).click();
await page.clock.runFor(300);
assert.equal(await page.locator('[data-telemetry-id="copilot-vscode"]').getByRole('button',{name:'一键配置',exact:true}).count(),1,'undo restores the merge setup entry');
await page.screenshot({path:out+'telemetry-settings.png',fullPage:true});
await page.setViewportSize({width:760,height:1000});
assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth > innerWidth),false,'telemetry detail controls and paths fit narrow layout');
await page.screenshot({path:out+'telemetry-settings-narrow.png',fullPage:true});
await page.setViewportSize({width:1440,height:1100});
await page.getByRole('navigation').first().getByRole('button',{name:'总览',exact:true}).click();
await page.clock.runFor(300);
assert.equal(await page.locator('.telemetry-setup.compact .target,.telemetry-setup.compact .path,.telemetry-setup.compact .preview').count(),0,'returning to overview keeps all details in settings');
assert.match(await overviewTelemetry.textContent(),/1 待开启[\s\S]*4 暂无数据[\s\S]*0 已核验/);
assert.doesNotMatch(await overviewTelemetry.textContent(),/配置受限/);
await page.evaluate(()=>window.telemetryBlockedHasData=true);
await page.clock.runFor(30_500);
assert.match(await overviewTelemetry.textContent(),/3 暂无数据[\s\S]*1 已核验[\s\S]*1 配置受限/,'received data does not erase a genuine configuration restriction');
await page.evaluate(()=>window.telemetryBlockedHasData=false);
await page.clock.runFor(30_500);
assert.match(await overviewTelemetry.textContent(),/4 暂无数据[\s\S]*0 已核验/,'automatic checks restore the no-data summary when no valid data is available');
assert.doesNotMatch(await overviewTelemetry.textContent(),/配置受限/);
const todayPanel=page.locator('[data-panel-group="overviewToday"]').filter({has:page.locator('.hourly')});
await todayPanel.getByRole('button',{name:'按Agent',exact:true}).click();await page.clock.runFor(300);
await page.evaluate(async () => {
  const echarts=await import('/node_modules/.vite/deps/echarts_core.js');
  echarts.getInstanceByDom(document.querySelector('[data-panel-group="overviewToday"] .hourly')).dispatchAction({type:'showTip',seriesIndex:0,dataIndex:0});
});await page.clock.runFor(100);
assert.match(await todayPanel.textContent(),/vscode-copilot-chat · 总 token: 未知[\s\S]*输入 token: 300,000[\s\S]*输出 token: 0/,'overview hover retains partial Copilot usage');
assert.match(await page.locator('.cost-panel').textContent(),/API[\s\S]*参考/,'overview labels the API price reference');
const overviewModels=page.locator('[data-panel-group="overviewToday"]').filter({has:page.getByRole('heading',{name:'今日模型明细',exact:true})});
assert.match(await overviewModels.locator('tfoot').textContent(),/24\.68/,'model footer includes the current currency subtotal');
assert.match(await overviewModels.locator('tbody tr').filter({hasText:'gpt-5.4'}).first().textContent(),/12\.00/,'each model includes its current API estimate');
assert.equal(await overviewModels.locator('thead th').count(),7,'model table has one reference amount column');
assert.match(await page.locator('.reference-summary').textContent(),/24\.68/,'today summary includes the same current estimate');
assert.doesNotMatch(await page.locator('.cost-panel').textContent(),/按发生时价|按当前价格模拟|12\.34/,'only the current reference estimate is displayed');
const priceRequests=await page.evaluate(()=>window.appCalls.filter(c=>c.cmd==='cost_summary').length);
await page.locator('.unit-prices summary').click();
assert.match(await page.locator('.unit-prices').textContent(),/gpt-5\.4[\s\S]*USD 3[\s\S]*USD 15[\s\S]*glm-5\.3[\s\S]*暂无适用单价/,'unit prices show actual model rates and unavailable rows');
assert.equal(await page.evaluate(()=>window.appCalls.filter(c=>c.cmd==='cost_summary').length),priceRequests,'opening prices reuses the loaded reference');
await page.locator('.unit-prices').screenshot({path:out+'unit-prices.png'});
await page.locator('.unit-prices summary').click();
assert.equal(await overviewModels.locator('section').evaluate(el=>el.scrollWidth>el.clientWidth),false,'full-width model table avoids desktop scrolling');
assert.ok(await page.locator('.curve canvas').count(),'overview renders the cost curve');
assert.match(await page.locator('tr').filter({hasText:'vscode-copilot-chat'}).first().textContent(),/—[\s\S]*300,000/,'token observations do not display an invented zero call count');
const todayAgentPie=page.locator('[data-panel-group="overviewToday"]').filter({has:page.getByRole('heading',{name:'今日 Agent 用量',exact:true})});
assert.match(await todayAgentPie.textContent(),/vscode-copilot-chat/,'unknown totals retain the Agent name beside the pie');
await todayAgentPie.getByRole('button',{name:'输入 token',exact:true}).click();await page.clock.runFor(300);
assert.ok(await todayAgentPie.locator('.pie').evaluate(async el=>{
  const echarts=await import('/node_modules/.vite/deps/echarts_core.js');return echarts.getInstanceByDom(el).getOption().series[0].data.some(d=>d.name==='vscode-copilot-chat'&&d.value===300000);
}),'known Copilot input appears in the input pie');
assert.equal(await page.evaluate(()=>window.appCalls.find(c=>c.cmd==='summary').args.q.last_day),'2026-09-28');

await page.screenshot({path:out+'overview-light.png',fullPage:true});
const summaryCount = () => page.evaluate(()=>window.appCalls.filter(c=>c.cmd==='summary').length);
const initialRequests = await summaryCount();
assert.equal(initialRequests,2,'initial load queries history and today once each');
await page.clock.runFor(31_000);
assert.equal(await summaryCount(),initialRequests,'idle status polling does not re-query charts');
await page.getByLabel('模型',{exact:true}).selectOption('gpt-5.4');
await page.clock.runFor(250);
await page.getByLabel('模型',{exact:true}).selectOption('glm-5.3');
await page.clock.runFor(1_300);
assert.equal(await page.locator('.today-cards .value').first().textContent(),'126','late filtered response cannot replace the newer selection');
assert.equal(await page.getByLabel('模型',{exact:true}).locator('option').count(),4,'filter catalog keeps unselected models');
await page.getByLabel('模型',{exact:true}).selectOption('');
await page.clock.runFor(500);
await page.locator('.user-picker select').selectOption('empty');
await page.clock.runFor(500);
assert.equal(await page.locator('.today-cards .value').count(),0,'empty user cannot inherit old totals');
assert.equal(await page.locator('.quota-card .used').textContent(),'50','quota remains visible when token history is empty');
assert.match(await page.locator('main > .empty').textContent(),/尚无数据/);
await page.locator('.user-picker select').selectOption('default');
await page.clock.runFor(500);
assert.match(await page.locator('[data-panel-group="overviewHistory"] .phead h3').first().textContent(),/token 用量/,'overview defaults to tokens before calls');
await page.getByRole('navigation').first().getByRole('button',{name:'趋势',exact:true}).click();
await page.clock.runFor(500);
assert.match(await page.locator('[data-panel-group="trendMain"] .phead h3').first().textContent(),/token 用量/,'trend defaults to tokens before calls');
assert.equal(await page.locator('.heatmap .days [data-day]').count(),365,'heatmap includes the whole year even with a 30-day statistics filter');
assert.equal(await page.locator('.heatmap .days [data-day="2026-01-01"].unavailable').count(),1,'pruned daily history is visibly different from zero calls');
assert.equal(await page.locator('.heatmap .days .partial').count(),1,'partial daily coverage retains known calls but marks uncertainty');
assert.match(await page.locator('.heatmap .days [data-day]').first().getAttribute('aria-label'),/无按日数据/);
assert.match(await page.locator('.heatmap .days [data-day="2026-12-31"]').getAttribute('aria-label'),/尚未到来/,'future cells are not reported as measured zero usage');
const getChartOptions = async (selector) => page.evaluate(async (sel) => {
  const echarts = await import('/node_modules/.vite/deps/echarts_core.js');
  return echarts.getInstanceByDom(document.querySelector(sel)).getOption();
}, selector);
const weekdayBefore=await getChartOptions('.weekday');
assert.equal(weekdayBefore.xAxis[0].data[0],'周一','weekday labels follow calendar weekdays in the configured America/Los_Angeles zone');
assert.ok(weekdayBefore.series[0].data.some(v=>v>0),'weekday distribution queries the statistics date range');
await page.getByRole('button',{name:'上一年',exact:true}).click();await page.clock.runFor(500);
assert.equal(await page.locator('.heatmap .days [data-day="2025-01-01"]').count(),1);
await page.getByRole('button',{name:'上一年',exact:true}).click();await page.clock.runFor(500);
assert.equal(await page.locator('.heatmap .days [data-day]').count(),366,'leap years include February 29');
assert.deepEqual((await getChartOptions('.weekday')).series[0].data,weekdayBefore.series[0].data,'heatmap year navigation cannot alter the weekday statistics');
await page.getByRole('button',{name:'下一年',exact:true}).click();await page.clock.runFor(500);
await page.getByRole('button',{name:'下一年',exact:true}).click();await page.clock.runFor(500);
const trendTokens=page.locator('[data-panel-group="trendMain"]').filter({has:page.locator('.token-chart')});
await trendTokens.getByRole('button',{name:'按Agent',exact:true}).click();await page.clock.runFor(300);
const tokenSelector='[data-panel-group="trendMain"] .token-chart';
const unknownTotal=await getChartOptions(tokenSelector);
assert.ok(unknownTotal.series.find(s=>s.name==='vscode-copilot-chat').data.every(v=>v===null),'unknown Copilot totals are preserved');
assert.equal(unknownTotal.series.filter(s=>s.name.startsWith('vscode-copilot-chat')).length,1,'total usage renders exactly one Copilot series');
await trendTokens.getByRole('button',{name:'输入 token',exact:true}).click();await page.clock.runFor(300);
const inputOptions=await getChartOptions(tokenSelector);
assert.equal(inputOptions.series.find(s=>s.name==='vscode-copilot-chat · 输入 token').data[0],300000,'unknown cache splits do not hide reported input');
assert.equal(inputOptions.series.find(s=>s.name==='vscode-copilot-chat · 缓存读取').data[0],null,'unknown cache input is not invented');
await trendTokens.getByRole('button',{name:'总 token',exact:true}).click();await page.clock.runFor(300);
await page.evaluate(async (sel) => {
  const echarts = await import('/node_modules/.vite/deps/echarts_core.js');
  const chart=echarts.getInstanceByDom(document.querySelector(sel));
  chart.dispatchAction({type:'showTip',seriesIndex:0,dataIndex:0});
},tokenSelector);await page.clock.runFor(100);
assert.match(await trendTokens.textContent(),/vscode-copilot-chat · 完整总 token 未提供[\s\S]*输入 token: 300,000[\s\S]*输出 token: 0/,'hover identifies the missing metric without an unknown Agent suffix');
assert.match(await page.locator('.cost-panel').textContent(),/API[\s\S]*参考/,'trend labels the API price reference');
const callsPanel=page.locator('[data-panel-group="trendMain"]').filter({has:page.locator('.calls-chart')});
await callsPanel.getByRole('button',{name:'按Agent',exact:true}).click();await page.clock.runFor(300);
const callsOptions=await getChartOptions('[data-panel-group="trendMain"] .calls-chart');
assert.ok(callsOptions.series.some(s=>s.name==='vscode-copilot-chat'),'calls legend contains the Agent');
assert.ok(callsOptions.series.every(s=>!/token/i.test(s.name)),'calls chart has no token series');
const summaryLayout=await page.locator('.range-summary .summary-cards').evaluate(el=>({columns:getComputedStyle(el).gridTemplateColumns.split(' ').length,children:el.children.length}));
assert.deepEqual(summaryLayout,{columns:7,children:7},'wide summaries keep seven compact metrics in one row');
assert.ok((await page.locator('.summary-quota').boundingBox()).height < 80,'quota is a compact independent strip');
await page.locator('.summary-quota summary').click();
assert.match(await page.locator('.summary-quota .detail-content').textContent(),/快照|账户/,'quota details retain snapshot and account scope');
await page.locator('.summary-quota summary').click();
assert.match(await page.locator('.range-summary .reference-summary').textContent(),/24\.68/,'range summary includes the current full estimate');
await page.setViewportSize({width:760,height:1000});await page.clock.runFor(100);
assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false,'compact trend summaries fit a narrow window');
await page.locator('.range-summary').screenshot({path:out+'trend-summary-narrow.png'});
await page.setViewportSize({width:1440,height:1100});await page.clock.runFor(100);
assert.ok((await trendTokens.boundingBox()).width > 900,'token and call charts have room across the page');
const trendModels=page.locator('[data-panel-group="trendMain"]').filter({has:page.locator('tfoot')});
assert.match(await trendModels.locator('tfoot').textContent(),/24\.68/,'trend model table also includes the current subtotal');
const costOption=await getChartOptions('[data-panel-group="trendMain"] .curve');
assert.deepEqual(costOption.series[0].data.slice(0,3),[1200,800,468],'cost curve uses current per-day amounts rather than historical prices');
await page.locator('[data-panel-group="trendMain"] .curve').scrollIntoViewIfNeeded();
await page.locator('[data-panel-group="trendMain"] .controls').getByRole('button',{name:'按模型',exact:true}).click();await page.clock.runFor(100);
assert.equal((await getChartOptions('[data-panel-group="trendMain"] .curve')).series.length,3,'cost curve can compare individual models');
await trendTokens.scrollIntoViewIfNeeded();
const selectionPoint=await page.locator(tokenSelector).evaluate(async el=>{
  const echarts=await import('/node_modules/.vite/deps/echarts_core.js');const chart=echarts.getInstanceByDom(el);
  const bounds=el.getBoundingClientRect();const point=chart.convertToPixel({gridIndex:0},[1,100000]);
  return {x:bounds.left+point[0],y:bounds.top+point[1]};
});
await page.mouse.click(selectionPoint.x,selectionPoint.y);await page.clock.runFor(400);
assert.match(await page.locator('.range-caption').textContent(),/选定范围[：:]2026-09-16/,'clicking a curve point scopes the summary');
assert.equal(await page.locator('.range-summary .scard').first().locator('.svalue').textContent(),'84','selected summary comes from the scoped backend query');
assert.match(await page.locator('.range-summary .reference-summary').textContent(),/5\.00/,'selected reference uses the same scoped query');
await page.evaluate(async sel=>{
  const echarts=await import('/node_modules/.vite/deps/echarts_core.js');
  echarts.getInstanceByDom(document.querySelector(sel)).dispatchAction({type:'dataZoom',startValue:2,endValue:4});
},tokenSelector);await page.clock.runFor(400);
assert.match(await page.locator('.range-caption').textContent(),/2026-09-17 ~ 2026-09-19/,'zooming selects the visible x-axis range');
const selectionRequest=await page.evaluate(()=>window.appCalls.filter(c=>c.cmd==='summary'&&c.args.q.first_period).at(-1).args.q);
assert.equal(selectionRequest.first_day,'2026-09-17');assert.equal(selectionRequest.last_day,'2026-09-19');
await page.evaluate(async sel=>{
  const echarts=await import('/node_modules/.vite/deps/echarts_core.js');
  echarts.getInstanceByDom(document.querySelector(sel)).trigger('click',{componentType:'xAxis',value:'2026-09-18'});
},tokenSelector);await page.clock.runFor(300);
assert.match(await page.locator('.range-caption').textContent(),/选定范围[：:]2026-09-18/,'x-axis labels also select a period');
await page.evaluate(async sel=>{
  window.selectionDelay=900;
  const echarts=await import('/node_modules/.vite/deps/echarts_core.js');
  echarts.getInstanceByDom(document.querySelector(sel)).trigger('click',{componentType:'xAxis',value:'2026-09-19'});
},tokenSelector);await page.clock.runFor(100);
await page.evaluate(async sel=>{
  window.selectionDelay=0;
  const echarts=await import('/node_modules/.vite/deps/echarts_core.js');
  echarts.getInstanceByDom(document.querySelector(sel)).trigger('click',{componentType:'xAxis',value:'2026-09-18'});
},tokenSelector);await page.clock.runFor(1_000);
assert.equal(await page.locator('.range-summary .scard').first().locator('.svalue').textContent(),'84','an older selection response cannot replace the current summary');
assert.match(await page.locator('.range-summary .reference-summary').textContent(),/5\.00/,'an older cost response cannot replace the selected estimate');
await page.locator('.range-reset').click();await page.clock.runFor(100);
assert.equal(await page.locator('.range-summary .scard').first().locator('.svalue').textContent(),'504','reset restores the original query summary');
assert.match(await page.locator('.range-summary .reference-summary').textContent(),/24\.68/,'reset restores the original current reference');
const pieLayout=await getChartOptions('[data-panel-group="trendMain"] .pie');
assert.equal(pieLayout.legend[0].orient,'horizontal','pie legends stay below the circle');
await trendModels.screenshot({path:out+'model-cost-table.png'});
await page.locator('[data-panel-group="trendMain"]').filter({has:page.locator('.curve')}).screenshot({path:out+'model-cost-curve.png'});
await page.screenshot({path:out+'trend-light.png',fullPage:true});
await page.locator('.filters select').first().selectOption('today');await page.clock.runFor(500);
const hourlyCost=await getChartOptions('[data-panel-group="trendMain"] .curve');
assert.equal(hourlyCost.xAxis[0].data.length,1,'hourly usage keeps costs on one daily axis');
await page.evaluate(async sel=>{
  const echarts=await import('/node_modules/.vite/deps/echarts_core.js');
  const chart=echarts.getInstanceByDom(document.querySelector(sel));
  chart.dispatchAction({type:'dataZoom',startValue:1,endValue:3});
},tokenSelector);await page.clock.runFor(300);
assert.match(await page.locator('.range-caption').textContent(),/09:00 ~ .*11:00/,'hour selection preserves the time bounds');
const hourlySelection=await page.evaluate(()=>window.appCalls.filter(c=>c.cmd==='summary'&&c.args.q.first_period).at(-1).args.q);
assert.equal(hourlySelection.first_period.slice(-5),'09:00');assert.equal(hourlySelection.last_period.slice(-5),'11:00');
const hourlyPrice=await page.evaluate(()=>window.appCalls.filter(c=>c.cmd==='cost_summary'&&c.args.q.first_period).at(-1).args.q);
assert.equal(hourlyPrice.first_period.slice(-5),'09:00');assert.equal(hourlyPrice.last_period.slice(-5),'11:00');
await page.locator('.filters select').first().selectOption('30');await page.clock.runFor(500);
assert.equal(await page.locator('.range-reset').count(),0,'changing the query clears the old selection');
await page.getByRole('navigation').first().getByRole('button',{name:'数据源',exact:true}).click();
assert.match(await page.locator('.source-card').filter({has:page.getByRole('heading',{name:'opencode',exact:true})}).textContent(),/已就绪[\s\S]*兼容读取 1 个文件/,'compatible parser is informational when the source is healthy');
assert.match(await page.locator('.source-card').filter({has:page.getByRole('heading',{name:'zcode',exact:true})}).textContent(),/部分数据需核对[\s\S]*未识别 1 个文件/,'unrecognized usage carrier remains visible');
assert.match(await page.locator('.source-card').filter({has:page.getByRole('heading',{name:'kimi-code',exact:true})}).textContent(),/部分数据需核对[\s\S]*需核对 1 个文件/,'degraded source shows a concrete file count');
await page.screenshot({path:out+'sources-light.png',fullPage:true});
await page.getByLabel('codex: 归属用户',{exact:true}).selectOption('empty');
await page.clock.runFor(500);
await page.getByRole('navigation').first().getByRole('button',{name:'总览',exact:true}).click();
await page.clock.runFor(500);
assert.equal(await page.locator('.today-cards .value').first().textContent(),'462','source assignment updates totals even when revision is unchanged');
await page.getByRole('navigation').first().getByRole('button',{name:'详情',exact:true}).click();
await page.clock.runFor(500);
await page.screenshot({path:out+'details-light.png',fullPage:true});
await page.getByRole('button',{name:'下一页',exact:true}).click();
await page.clock.runFor(200);
await page.getByRole('button',{name:'采集并刷新',exact:true}).click();
await page.clock.runFor(800);
assert.match(await page.locator('.page-info').textContent(),/第 2 \/ 3 页/,'background refresh retains the current detail page');
assert.equal(await page.evaluate(()=>window.appCalls.filter(c=>c.cmd==='event_details').at(-1).args.page),1);
await page.evaluate(()=>{ window.detailTotal = 12; });
await page.getByRole('button',{name:'采集并刷新',exact:true}).click();
await page.clock.runFor(800);
assert.match(await page.locator('.page-info').textContent(),/第 1 \/ 1 页/,'retention shrinking the result clamps to the last available page');
assert.equal(await page.locator('.details tbody tr').count(),12);
await page.getByRole('navigation').first().getByRole('button',{name:'设置',exact:true}).click();
assert.equal(await page.locator('#telemetry-settings').count(),0,'normal settings navigation starts at general');
await page.locator('[data-settings-section="telemetry"]').click();
assert.equal(await page.locator('[data-telemetry-id="copilot-vscode"]').count(),1,'settings provides the same merge-configuration entry');
await page.locator('[data-settings-section="general"]').click();
await page.screenshot({path:out+'settings-light.png',fullPage:true});
await page.getByRole('radio',{name:'暗色',exact:true}).click();
await page.getByRole('button',{name:'保存',exact:true}).click();
await page.getByRole('navigation').first().getByRole('button',{name:'总览',exact:true}).click();
await page.clock.runFor(500);
await page.screenshot({path:out+'overview-dark.png',fullPage:true});
await page.getByRole('navigation').first().getByRole('button',{name:'趋势',exact:true}).click();
await page.clock.runFor(500);
await page.screenshot({path:out+'trend-dark.png',fullPage:true});
await page.getByRole('navigation').first().getByRole('button',{name:'设置',exact:true}).click();
await page.getByLabel('界面语言',{exact:true}).selectOption('ja');
await page.getByRole('button',{name:'保存',exact:true}).click();
assert.equal(await page.getByRole('navigation').first().getByRole('button',{name:'推移',exact:true}).count(),1,'Japanese navigation updates immediately');
await page.getByRole('navigation').first().getByRole('button',{name:'推移',exact:true}).click();
await page.clock.runFor(500);
assert.equal(await page.locator('.heatmap .days [data-day]').count(),365,'full-year cells persist after locale switch');
assert.match(await page.locator('.heatmap .days [data-day]').first().getAttribute('aria-label'),/日別データなし/);
await page.screenshot({path:out+'trend-ja.png',fullPage:true});
await page.getByRole('navigation').first().getByRole('button',{name:'設定',exact:true}).click();
await page.getByLabel('表示言語',{exact:true}).selectOption('es');
await page.getByRole('button',{name:'保存',exact:true}).click();
assert.equal(await page.getByRole('navigation').first().getByRole('button',{name:'Tendencias',exact:true}).count(),1,'Spanish navigation updates immediately');
for (const locale of ['zh-TW','ko','fr','de','pt-BR','ru','en','zh-CN']) {
  await page.locator('[data-testid="language-select"]').selectOption(locale);
  await page.locator('form button[type="submit"]').click();
  assert.equal(await page.evaluate(()=>window.appCalls.filter(c=>c.cmd==='set_settings').at(-1).args.settings.language),locale,locale+' persists');
  assert.equal(await page.locator('[data-testid="language-select"]').inputValue(),locale,locale+' remains selected');
  if (locale==='de' || locale==='ru') {
    await page.setViewportSize({width:760,height:1000});
    assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth > innerWidth),false,locale+' settings fit narrow layout');
    await page.locator('[data-settings-section="telemetry"]').click();
    await page.clock.runFor(200);
    assert.equal(await page.locator('#telemetry-settings').getByRole('button',{name:locale==='de'?'Alle aktivieren':'Включить всё',exact:true}).count(),1,locale+' batch setup action is translated');
    assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth > innerWidth),false,locale+' telemetry controls fit narrow layout');
    await page.locator('[data-settings-section="general"]').click();
    await page.setViewportSize({width:1440,height:1100});
  }
}
// 清理全部数据（后台任务事件驱动）：确认层展示缺失文件预警与阶段进度，
// done 后自动关闭并给出已清理汇总。
await page.getByRole('button', { name: '归档保留', exact: true }).click();
await page.getByRole('button', { name: '清理全部数据并重新采集', exact: true }).click();
await page.clock.runFor(300);
assert.match(await page.locator('.dialog').textContent(), /无法重新采集/, 'preview warning is shown');
await page.locator('.dialog').getByRole('button', { name: '清理全部数据并重新采集' }).click();
assert.match(await page.locator('.dialog').getByRole('status').textContent(), /等待当前采集结束/, 'clear-all reports its background phase');
await page.clock.runFor(700);
assert.match(await page.locator('.dialog').getByRole('status').textContent(), /正在清理数据/, 'phase text advances from events');
await page.clock.runFor(1200);
assert.equal(await page.locator('.dialog').count(), 0, 'dialog closes when the background job finishes');
assert.match(await page.locator('.ok').filter({ hasText: '已清理' }).first().textContent(), /已清理：/, 'cleared counts are summarized');
await page.getByRole('navigation').first().getByRole('button',{name:'总览',exact:true}).click();
await page.setViewportSize({width:760,height:1000});
await page.evaluate(()=>{window.singleChartPeriod=true;});
await page.getByRole('button',{name:'采集并刷新',exact:true}).click();await page.clock.runFor(800);
const singlePanel=page.locator('[data-panel-group="overviewToday"]').filter({has:page.locator('.hourly')});
await singlePanel.getByRole('button',{name:'按Agent',exact:true}).click();await page.clock.runFor(300);
const singleCalls=await getChartOptions('[data-panel-group="overviewToday"] .hourly');
assert.deepEqual(singleCalls.series.map(s=>s.name),['codex','vscode-copilot-chat'],'single-period call legend uses Agent names');
assert.equal(singleCalls.yAxis.length,1,'call grouping contains a single call axis');
await page.evaluate(async ()=>{const echarts=await import('/node_modules/.vite/deps/echarts_core.js');echarts.getInstanceByDom(document.querySelector('[data-panel-group="overviewToday"] .hourly')).dispatchAction({type:'showTip',seriesIndex:1,dataIndex:1});});await page.clock.runFor(100);
assert.match(await singlePanel.textContent(),/vscode-copilot-chat[\s\S]*输入 token: 300,000/,'single-period hover keeps partial usage information');
await page.screenshot({path:out+'overview-narrow.png',fullPage:true});
assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth > innerWidth),false,'no horizontal page overflow');
assert.deepEqual(errors,[]);
await writeFile(out+'browser-results.json',JSON.stringify({errors,requests:await page.evaluate(()=>window.appCalls.length),checks:['five pages','full-year light/dark heatmap','year navigation and leap days','future/retained dates','Copilot input with unknown cache split','Copilot unknown total hover and zero output','async telemetry discovery','compact overview with two actions','details navigation and focus','batch partial failure and retry','existing/managed outputs preserved','cross-page progress and undo','installed Agents only','merge configuration preview/apply/undo','ten locale switches','narrow telemetry layout in Chinese/German/Russian','default panel order','source health and compatibility','statistics timezone','initial/idle query counts','stale filter responses','user isolation','source membership without revision','refresh preserves pagination','retention clamps pagination'],screenshots:11},null,2));
console.log('Browser checks passed: total-only series, Agent calls and single-period hover, partial share pies, overview/trend API references, filled summary layout, telemetry batch setup/retry/undo, ten locales, themes, timezone, filters and pagination.');
} catch (error) {
  const snapshot = testPage ? await testPage.evaluate(() => ({ calls: window.appCalls ?? [], text: document.body.innerText })).catch(() => null) : null;
  await writeFile(out+'failure.json',JSON.stringify({error:String(error),errors,console:browserConsole,snapshot},null,2));
  if (testPage) await testPage.screenshot({path:out+'failure.png',fullPage:true}).catch(() => {});
  throw error;
} finally {
  await browser?.close();
  server.kill();
  await writeFile(out+'vite.log',serverMessages.join(''));
}
