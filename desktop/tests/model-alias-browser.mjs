import assert from 'node:assert/strict';

export async function modelAliasBrowserChecks(page) {
  const recomputes=await page.evaluate(()=>window.appCalls.filter(call=>call.cmd==='recompute_costs').length);
  await page.evaluate(() => {
    const original = window.__TAURI_INTERNALS__.invoke;
    window.aliasMock = { generation: 0, label: 'Qwen4-Max-Preview', error: null, invalidImport: false };
    window.__TAURI_INTERNALS__.invoke = async (cmd, args = {}) => {
      const state = window.aliasMock;
      const revision = `mock-aliases:${state.generation}`;
      if (['model_alias_status','refresh_model_aliases','import_model_aliases','pick_open_path'].includes(cmd)) {
        window.appCalls.push({cmd,args});
        if (cmd === 'pick_open_path') return 'C:/synthetic/catalogue.json';
        if (cmd === 'refresh_model_aliases') { state.error = 'model_alias_download'; return true; }
        if (cmd === 'import_model_aliases') {
          if (state.invalidImport) throw new Error('model_alias_json');
          state.generation++; state.label = 'Catalogue model'; state.error = null;
          return {generation:state.generation,inserted_rules:1,already_present:false};
        }
        return {enabled:true,ttl_days:7,running:false,generation:state.generation,revision,rules:state.generation,cache:null,error:null,
          last_outcome:state.error ? {status:'failed',error:state.error,first_error:state.error} : null};
      }
      const value = await original(cmd,args);
      if (cmd === 'refresh_status') return {...value,model_alias_revision:revision};
      if (cmd === 'summary') return {...value,model_alias_revision:revision,
        models:[{model:'qmodel-latest',display_model:state.label,provider:null,sums:value.totals}],
        agents:[{agent:'qoder',sums:value.totals}]};
      if (cmd === 'chart_series') {
        const name=args.dimension==='agent_model'?'qoder/qmodel-latest':'qmodel-latest';
        return {rows:[{...value.rows[0],label:args.q.granularity==='hour'?`${args.q.last_day} 10:00`:args.q.last_day,
          series:name,display_series:args.dimension==='agent_model'?`qoder/${state.label}`:state.label}]};
      }
      if (cmd === 'event_details') return {...value,rows:value.rows.map(row=>({...row,agent:'qoder',model:'qmodel_latest',display_model:state.label}))};
      return value;
    };
  });
  const nav = index => page.locator('.app-shell > aside nav button').nth(index);
  await page.setViewportSize({width:1440,height:1100});
  await nav(4).click(); await page.clock.runFor(300);
  await page.locator('[data-settings-section="models"]').click(); await page.clock.runFor(300);
  assert.equal(await page.getByTestId('model-alias-enabled').isChecked(),false,'legacy settings keep metadata downloads off');
  assert.equal(await page.getByTestId('model-alias-ttl').inputValue(),'3');
  assert.equal(await page.getByTestId('model-alias-refresh').isDisabled(),true);
  await page.getByTestId('model-alias-enabled').check();
  await page.getByTestId('model-alias-ttl').fill('7'); await page.clock.runFor(1200);
  const writes=()=>page.evaluate(()=>window.appCalls.filter(call=>call.cmd==='set_settings'));
  assert.equal((await writes()).at(-1).args.settings.model_aliases.cache_ttl_days,7);
  assert.equal((await writes()).at(-1).args.settings.model_aliases.online_refresh_enabled,true);
  const count=(await writes()).length;
  await page.getByTestId('model-alias-ttl').fill('0'); await page.clock.runFor(1200);
  assert.equal((await writes()).length,count,'invalid TTL stays in the draft');
  assert.equal(await page.getByTestId('model-alias-ttl').inputValue(),'0');
  await page.getByTestId('model-alias-ttl').fill('7'); await page.clock.runFor(1200);
  await page.evaluate(()=>window.aliasMock.invalidImport=true);
  await page.getByTestId('model-alias-import').click(); await page.clock.runFor(300);
  assert.match(await page.getByTestId('model-alias-settings').textContent(),/model_alias_json/);
  await page.evaluate(()=>window.aliasMock.invalidImport=false);
  await page.getByTestId('model-alias-import').click(); await page.clock.runFor(600);
  assert.match(await page.getByTestId('model-alias-status').textContent(),/1/);
  await page.getByTestId('model-alias-refresh').click(); await page.clock.runFor(300);
  assert.match(await page.getByTestId('model-alias-settings').textContent(),/model_alias_download/);
  await nav(0).click(); await page.clock.runFor(600);
  const rawRow = page.locator('strong[title="qmodel-latest"]');
  assert.equal(await rawRow.first().textContent(),'Catalogue model');
  assert.equal(await page.locator('option[value="qmodel-latest"]').textContent(),'Catalogue model');
  const lastRevision=await page.evaluate(()=>window.appCalls.filter(call=>call.cmd==='summary').length);
  await page.evaluate(()=>{window.aliasMock.generation++;window.aliasMock.label='Next catalogue model';});
  await page.clock.runFor(11_000);
  assert.equal(await rawRow.first().textContent(),'Next catalogue model','metadata-only polling refreshes existing data');
  assert.ok(await page.evaluate(()=>window.appCalls.filter(call=>call.cmd==='summary').length)>lastRevision);
  const chartBox=page.locator('[data-panel-group="overviewToday"]').filter({has:page.locator('.hourly')});
  await chartBox.locator('.dim-seg button').nth(1).click(); await page.clock.runFor(500);
  const chart=await page.evaluate(async()=>{
    const echarts=await import(window.chartModuleUrl);
    const option=echarts.getInstanceByDom(document.querySelector('[data-panel-group="overviewToday"] .hourly')).getOption();
    return {raw:option.series[0].name,label:option.legend[0].formatter(option.series[0].name)};
  });
  assert.equal(chart.raw,'qmodel-latest');
  assert.equal(chart.label,'Next catalogue model');
  await nav(3).click(); await page.clock.runFor(400);
  assert.equal(await page.locator('td[title="qmodel_latest"]').first().textContent(),'Next catalogue model');
  await page.evaluate(()=>{window.aliasMock.generation++;window.aliasMock.label='qmodel-latest';});
  await page.clock.runFor(11_000);
  assert.equal(await page.locator('td[title="qmodel_latest"]').first().textContent(),'qmodel-latest','raw conflict result is not replaced by the bundled fallback');
  assert.equal(await page.evaluate(()=>window.appCalls.filter(call=>call.cmd==='recompute_costs').length),recomputes);
}
