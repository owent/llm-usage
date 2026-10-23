import assert from 'node:assert/strict';

export async function settingsInvalidationChecks(page) {
  const nav = (index) => page.locator('.app-shell > aside nav button').nth(index);
  const calls = () => page.evaluate(() => window.appCalls.filter(c => ['summary','cost_summary','heatmap','weekday_distribution'].includes(c.cmd)).length);
  await nav(1).click(); await page.clock.runFor(500);
  await page.locator('[data-panel-group="trendMain"] .token-chart').evaluate(async el => {
    const echarts = await import(window.chartModuleUrl), chart = echarts.getInstanceByDom(el);
    chart.trigger('click', {componentType:'xAxis', value:chart.getOption().xAxis[0].data[0]});
  });
  await page.clock.runFor(500);
  const selected = await page.locator('.range-summary .range-caption').textContent();
  await nav(4).click(); await page.clock.runFor(500);
  const before = await calls();
  await page.locator('[data-settings-section="identity"]').click();
  await page.getByLabel('显示别名',{exact:true}).fill('Issue 4 autosave'); await page.clock.runFor(1200);
  assert.equal(await calls(), before, 'saving an alias does not invalidate usage or cost queries');
  await page.locator('[data-settings-section="general"]').click();
  await page.getByLabel('手工数据源根目录（每行一个）',{exact:true}).fill('C:/Demo/issue-4'); await page.clock.runFor(1200);
  assert.equal(await calls(), before, 'saving source roots does not invalidate collected usage');
  await page.getByLabel('手工数据源根目录（每行一个）',{exact:true}).fill(''); await page.clock.runFor(700);
  await nav(1).click(); await page.clock.runFor(500);
  assert.equal(await page.locator('.range-summary .range-caption').textContent(), selected, 'unrelated autosaves preserve the selected trend period');
  await nav(0).click(); await page.clock.runFor(500);
  await page.locator('[data-panel-group="overviewToday"] .hourly').evaluate(async el => {
    const echarts = await import(window.chartModuleUrl), chart = echarts.getInstanceByDom(el);
    chart.trigger('click', {componentType:'xAxis', value:chart.getOption().xAxis[0].data[0]});
  });
  await page.clock.runFor(500);
  const todaySelected = await page.locator('.today-range-caption').textContent();
  await nav(4).click(); await page.clock.runFor(500);
  const beforeOtherPreferences = await calls();
  await page.locator('[data-settings-section="updates"]').click();
  await page.getByTestId('update-schedule').selectOption('weekly'); await page.clock.runFor(1200);
  await page.getByTestId('update-schedule').selectOption('daily'); await page.clock.runFor(1200);
  await page.locator('[data-settings-section="general"]').click();
  await page.getByRole('radio',{name:'深色',exact:true}).click(); await page.clock.runFor(1200);
  await page.getByRole('radio',{name:'浅色',exact:true}).click(); await page.clock.runFor(1200);
  assert.equal(await calls(), beforeOtherPreferences, 'update and theme preferences do not reload statistics');
  await nav(0).click(); await page.clock.runFor(500);
  assert.equal(await page.locator('.today-range-caption').textContent(), todaySelected, 'unrelated autosaves preserve the selected hour');
  await nav(4).click(); await page.locator('[data-settings-section="general"]').click();
  const beforeLanguage = await calls();
  await page.getByTestId('language-select').selectOption('en'); await page.clock.runFor(700);
  assert.ok(await calls() > beforeLanguage, 'a language change still refreshes statistics');
  await nav(0).click(); await page.clock.runFor(500);
  assert.equal(await page.locator('.today-range-caption').count(), 0, 'a language change resets the selected hour');
  await nav(1).click(); await page.clock.runFor(500);
  assert.match(await page.locator('.range-summary .range-caption').textContent(), /Query range/, 'calendar/locale changes reset the trend selection');
  await nav(4).click(); await page.getByTestId('language-select').selectOption('zh-CN'); await page.clock.runFor(700);
  await page.locator('[data-settings-section="identity"]').click();
  await page.getByLabel('显示别名',{exact:true}).fill(''); await page.clock.runFor(700);
  await page.locator('[data-settings-section="general"]').click();
}
