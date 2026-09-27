import { chromium } from 'playwright';
import { writeFile, mkdir } from 'node:fs/promises';
import { resolve } from 'node:path';
import { spawn } from 'node:child_process';
import assert from 'node:assert/strict';

const out = resolve('build/browser-smoke') + '/';
await mkdir(out, {recursive:true});
const server = spawn(process.execPath, [resolve('desktop/node_modules/vite/bin/vite.js'), '--host', '127.0.0.1', '--port', '1421', '--strictPort'], {cwd:resolve('desktop'),stdio:'ignore',windowsHide:true});
let browser;
try {
  let available = false;
  for (let i=0;i<100;i++) {
    if (server.exitCode !== null) throw new Error('Vite test server exited before startup');
    try { available = (await fetch('http://127.0.0.1:1421')).ok; } catch {}
    if (available) break;
    await new Promise(r=>setTimeout(r,100));
  }
  assert.ok(available, 'test server started');
  browser = await chromium.launch({ channel: process.platform === 'win32' ? 'msedge' : undefined, headless: true });
const context = await browser.newContext({ viewport: { width: 1440, height: 1100 }, timezoneId: 'America/Los_Angeles', colorScheme: 'light' });
const page = await context.newPage();
const errors = [];
page.on('pageerror', (e) => errors.push(e.message));
await page.clock.install({ time: new Date('2026-09-27T23:30:00Z') });
await page.addInitScript(() => {
  const stats = (n=1) => ({ input_total_known: String(125000*n), uncached_known: String(25000*n), cache_read_known: String(100000*n), cache_write_known: '0', output_total_known: String(8000*n), total_tokens_known: String(133000*n), input_known_count: 40*n, input_unknown_count: 2*n, output_known_count: 42*n, output_unknown_count: 0, total_known_count: 40*n, total_unknown_count: 2*n, event_count: 42*n, call_count: 42*n, attempt_count: 0, conflict_count: 0, cache_input_ratio: .8, avg_duration_ms:'4250', total_duration_ms:String(170000*n), duration_sample_count: 40*n });
  let settings = { timezone: 'Asia/Shanghai', week_start: 0, retention: { events_days: 7, hourly_days: 3, daily_days: 90, weekly_days: 1095, monthly_days: 3650, yearly_days: null }, refresh_interval_secs: 300, language: 'zh-CN', theme: 'light', manual_roots: [], hostname_alias: null };
  let current = 'default';
  const users = [{user_id:'default',name:'本机用户',created_at_ms:0},{user_id:'empty',name:'新用户',created_at_ms:0}];
  const owners = {};
  let revision = 5;
  let finished = 1790550000000;
  window.appCalls = [];
  window.detailTotal = 123;
  window.__TAURI_INTERNALS__ = { invoke: async (cmd, args={}) => {
    window.appCalls.push({cmd,args});
    const q=args.q;
    if(cmd==='get_settings') return settings;
    if(cmd==='set_settings') { settings=args.settings; return; }
    if(cmd==='list_users') return {users,current};
    if(cmd==='set_current_user') {current=args.userId;return;}
    if(cmd==='summary') {
      const empty = current==='empty';
      const models = ['gpt-5.4','claude-sonnet-4.6','glm-5.3'];
      const selected = q.models?.[0];
      if(selected==='gpt-5.4') await new Promise(r=>setTimeout(r,900));
      const n=empty?0: selected==='gpt-5.4'?1:selected==='glm-5.3'?3:owners['codex@C:/Users/local/.codex']==='empty'?11:12;
      return { data_revision:revision, timezone:settings.timezone, totals:stats(n), distinct_sessions:empty?0:27, active_days:empty?0:12,
        periods:empty?[]:Array.from({length:q.first_day===q.last_day?1:14},(_,i)=>({label:q.first_day===q.last_day?q.last_day:`2026-09-${String(15+i).padStart(2,'0')}`,start_day:q.first_day,end_day:q.last_day,in_progress:i===13,partial_history:i<5,sums:stats(i%4+1),distinct_sessions:5+i,active_days:1})),
        models:empty?[]:models.filter(m=>!selected||selected===m).map((m,i)=>({model:m,provider:['openai','anthropic','zhipu'][i],sums:stats(6-i)})),
        agents:empty?[]:['codex','claude','zcode'].filter(a=>!q.agents?.length||q.agents.includes(a)).map((agent,i)=>({agent,sums:stats(6-i)})),
        today_hourly:empty?[]:Array.from({length:12},(_,i)=>({hour:8+i,calls:12+i*3,total_tokens:String((i+1)*150000),input_total:String((i+1)*120000),cache_read:String(i*80000),output_total:String((i+1)*30000),sessions:5,avg_duration_ms:'4250'})),excluded_event_count:0 };
    }
    if(cmd==='list_sources') return {sources:['codex','claude','zcode','opencode','gemini','kimi-code'].map((agent,i)=>({instance_id:`${agent}@C:/Users/local/.${agent}`,agent,format:'local',health:i===5||i===2?'degraded':'ok',enabled:i!==4,origin_host_id:'local',user_id:owners[`${agent}@C:/Users/local/.${agent}`]??'default',last_success_ms:finished,compat_files:i===3?1:0,degraded_files:i===5?1:0,unsupported_files:i===2?1:0,incompatible_files:0,missing_files:i===0?2:0}))};
    if(cmd==='refresh_status') return {running:false,started_ms:finished-1000,last_finished_ms:finished,trigger:'manual',progress_percent:100,eta_seconds:null,completed_adapters:[],instances:[]};
    if(cmd==='refresh_sources') {revision++;finished++;return {started:true,running:false,last_finished_ms:finished};}
    if(cmd==='chart_series') return {rows:Array.from({length:12},(_,i)=>({label:q.granularity==='hour'?`${q.last_day} ${String(8+i).padStart(2,'0')}:00`:`2026-09-${String(15+i).padStart(2,'0')}`,series:'gpt-5.4',calls:10+i,input:String(100000*(i+1)),cache_read:String(80000*(i+1)),cache_write:'0',uncached:String(20000*(i+1)),cache_ratio:.8,output:'15000',total:String(115000*(i+1))}))};
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
    if(cmd==='assign_source_user') {owners[args.instanceId]=args.userId;return;}
    if(cmd==='set_source_enabled') return;
    throw new Error(`Unhandled command ${cmd}`);
  }};
});
await page.goto('http://127.0.0.1:1421');
await page.waitForSelector('.today-cards .value');
await page.clock.runFor(800);
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
assert.match(await page.locator('main > .empty').textContent(),/尚无数据/);
await page.locator('.user-picker select').selectOption('default');
await page.clock.runFor(500);
assert.match(await page.locator('[data-panel-group="overviewHistory"] .phead h3').first().textContent(),/token 用量/,'overview defaults to tokens before calls');
await page.getByRole('navigation').first().getByRole('button',{name:'趋势',exact:true}).click();
await page.clock.runFor(500);
assert.match(await page.locator('[data-panel-group="trendMain"] .phead h3').first().textContent(),/token 用量/,'trend defaults to tokens before calls');
assert.equal(await page.locator('.heatmap .days [data-day]').count(),30,'heatmap has one cell per local date in range');
assert.equal(await page.locator('.heatmap .days .unavailable').count(),1,'pruned daily history is visibly different from zero calls');
assert.equal(await page.locator('.heatmap .days .partial').count(),1,'partial daily coverage retains known calls but marks uncertainty');
assert.match(await page.locator('.heatmap .days [data-day]').first().getAttribute('aria-label'),/无按日数据/);
await page.screenshot({path:out+'trend-light.png',fullPage:true});
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
assert.equal(await page.locator('.heatmap .days [data-day]').count(),30,'day cells persist after locale switch');
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
    await page.setViewportSize({width:1440,height:1100});
  }
}
await page.getByRole('navigation').first().getByRole('button',{name:'总览',exact:true}).click();
await page.setViewportSize({width:760,height:1000});
await page.screenshot({path:out+'overview-narrow.png',fullPage:true});
assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth > innerWidth),false,'no horizontal page overflow');
assert.deepEqual(errors,[]);
await writeFile(out+'browser-results.json',JSON.stringify({errors,requests:await page.evaluate(()=>window.appCalls.length),checks:['five pages','daily light/dark heatmap','ten locale switches','narrow layout','default panel order','source health and compatibility','statistics timezone','initial/idle query counts','stale filter responses','user isolation','source membership without revision','refresh preserves pagination','retention clamps pagination'],screenshots:9},null,2));
console.log('Browser checks passed: daily heatmap, ten locales, themes, timezone, stale responses, user isolation, membership, pagination, idle polling.');
} finally {
  await browser?.close();
  server.kill();
}
