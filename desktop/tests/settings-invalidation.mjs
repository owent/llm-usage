import assert from 'node:assert/strict';

export async function settingsInvalidationChecks(page) {
  const nav = (index) => page.locator('.app-shell > aside nav button').nth(index);
  const trendReset = page.locator('.range-summary .range-reset');
  const trendCaption = page.locator('.range-summary .range-caption');
  const todayCaption = page.locator('.today-range-caption');
  const go = async (index) => {
    await nav(index).click();
    await page.clock.runFor(500);
  };
  const settleSave = () => page.clock.runFor(1200);
  const calls = () => page.evaluate(() => {
    const result = {
      summary: 0,
      summarySelected: 0,
      cost_summary: 0,
      costSelected: 0,
      heatmap: 0,
      weekday: 0,
      recompute_costs: 0,
    };
    for (const call of window.appCalls) {
      if (call.cmd === 'summary' || call.cmd === 'cost_summary') {
        const key = call.cmd === 'summary'
          ? (call.args.q.first_period ? 'summarySelected' : 'summary')
          : (call.args.q.first_period ? 'costSelected' : 'cost_summary');
        result[key]++;
      } else if (call.cmd === 'heatmap') {
        const q = call.args.q;
        const annual = q.first_day.endsWith('-01-01') && q.last_day.endsWith('-12-31');
        result[annual ? 'heatmap' : 'weekday']++;
      } else if (call.cmd === 'recompute_costs') {
        result.recompute_costs++;
      }
    }
    return result;
  });
  const expectDelta = async (before, expected, message, ignored = []) => {
    const after = await calls();
    const keys = Object.keys(before).filter((key) => !ignored.includes(key));
    const actual = Object.fromEntries(keys.map((key) => [key, after[key] - before[key]]));
    const complete = Object.fromEntries(keys.map((key) => [key, expected[key] ?? 0]));
    assert.deepEqual(actual, complete, message);
    return after;
  };
  const selectPeriod = async (selector) => {
    await page.locator(selector).evaluate(async (el) => {
      const echarts = await import(window.chartModuleUrl);
      const chart = echarts.getInstanceByDom(el);
      chart.trigger('click', {componentType: 'xAxis', value: chart.getOption().xAxis[0].data[0]});
    });
    await page.clock.runFor(500);
  };
  const ensureSelections = async () => {
    await go(1);
    if (await trendReset.count() === 0) {
      await selectPeriod('[data-panel-group="trendMain"] .token-chart');
    }
    assert.equal(await trendReset.count(), 1, 'the trend selection is established');
    const trend = await trendCaption.textContent();
    await go(0);
    if (await todayCaption.count() === 0) {
      await selectPeriod('[data-panel-group="overviewToday"] .hourly');
    }
    assert.equal(await todayCaption.count(), 1, 'the today selection is established');
    return {trend, today: await todayCaption.textContent()};
  };
  const assertSelectionsPreserved = async (selected, message) => {
    await go(1);
    assert.equal(await trendCaption.textContent(), selected.trend, message + ': trend');
    await go(0);
    assert.equal(await todayCaption.textContent(), selected.today, message + ': today');
  };
  const openCosts = async () => {
    await go(4);
    await page.locator('[data-settings-section="costs"]').click();
    await page.clock.runFor(500);
  };
  const assertPricingReturn = async (before, selected, message) => {
    await go(1);
    await expectDelta(before, {
      cost_summary: 1, costSelected: 1, recompute_costs: 1,
    }, message + ': visible costs and one recomputation', ['summarySelected', 'heatmap', 'weekday']);
    assert.equal(await trendCaption.textContent(), selected.trend, message + ': trend selection');
    await go(0);
    assert.equal(await todayCaption.textContent(), selected.today, message + ': today selection');
  };

  const unrelated = await ensureSelections();
  await go(4);
  const beforeUnrelated = await calls();
  await page.locator('[data-settings-section="identity"]').click();
  await page.getByLabel('显示别名', {exact: true}).fill('Issue 4 autosave');
  await settleSave();
  await page.locator('[data-settings-section="general"]').click();
  await page.getByLabel('手工数据源根目录（每行一个）', {exact: true}).fill('C:/Demo/issue-4');
  await settleSave();
  await page.getByLabel('手工数据源根目录（每行一个）', {exact: true}).fill('');
  await settleSave();
  await page.locator('[data-settings-section="updates"]').click();
  await page.getByTestId('update-schedule').selectOption('weekly');
  await settleSave();
  await page.getByTestId('update-schedule').selectOption('daily');
  await settleSave();
  await page.locator('[data-settings-section="general"]').click();
  await page.getByRole('radio', {name: '深色', exact: true}).click();
  await settleSave();
  await page.getByRole('radio', {name: '浅色', exact: true}).click();
  await settleSave();
  await page.locator('[data-settings-section="identity"]').click();
  await page.getByLabel('显示别名', {exact: true}).fill('');
  await settleSave();
  await expectDelta(beforeUnrelated, {}, 'alias, roots, update and theme autosaves do not reload statistics or recompute costs');
  await assertSelectionsPreserved(unrelated, 'unrelated autosaves preserve selections');

  const calendarChange = async (name, change) => {
    await ensureSelections();
    await go(4);
    await page.locator('[data-settings-section="general"]').click();
    await page.clock.runFor(500);
    const before = await calls();
    await change();
    await settleSave();
    await go(1);
    await expectDelta(before, {
      summary: 2, cost_summary: 1, heatmap: 1, weekday: 1,
    }, name + ' refreshes summary, cost and calendar queries exactly once');
    assert.equal(await trendReset.count(), 0, name + ' clears the trend selection');
    await go(0);
    assert.equal(await todayCaption.count(), 0, name + ' clears the today selection');
  };
  await calendarChange('a timezone change', async () => {
    await page.locator('.tz-select').click();
    await page.getByRole('option', {name: 'UTC', exact: true}).click();
  });
  await calendarChange('a week-start change', async () => {
    await page.getByLabel('每周起始日', {exact: true}).selectOption('6');
  });
  await calendarChange('a language change', async () => {
    await page.getByTestId('language-select').selectOption('en');
  });

  await go(4);
  await page.getByTestId('language-select').selectOption('zh-CN');
  await settleSave();
  await page.locator('.tz-select').click();
  await page.getByRole('option', {name: 'Asia/Shanghai', exact: true}).click();
  await settleSave();
  await page.getByLabel('每周起始日', {exact: true}).selectOption('0');
  await settleSave();

  const pricingSelection = await ensureSelections();
  const deferredCostQueries = ['cost_summary', 'costSelected'];
  await openCosts();
  const beforeDisable = await calls();
  await page.locator('#cost-enabled').uncheck();
  await settleSave();
  const disabled = await expectDelta(beforeDisable, {}, 'disabling estimates does not reload statistics or recompute costs');
  await page.locator('#cost-enabled').check();
  await settleSave();
  await expectDelta(disabled, {recompute_costs: 1}, 'enabling estimates recomputes costs exactly once', deferredCostQueries);
  await assertPricingReturn(disabled, pricingSelection, 'enabling estimates');

  await openCosts();
  const beforeProvider = await calls();
  await page.getByRole('button', {name: '添加供应商设置', exact: true}).click();
  await page.clock.runFor(700);
  const incompleteProvider = await expectDelta(beforeProvider, {}, 'an incomplete provider default does not recompute or reload');
  await page.getByPlaceholder('供应商').fill('openai');
  await page.getByPlaceholder('地区').fill('global');
  await page.getByPlaceholder('渠道').fill('api');
  await settleSave();
  await expectDelta(incompleteProvider, {recompute_costs: 1}, 'a complete provider default recomputes costs exactly once', deferredCostQueries);
  await assertPricingReturn(incompleteProvider, pricingSelection, 'adding a provider default');

  await openCosts();
  const beforeProviderEdit = await calls();
  await page.getByPlaceholder('地区').fill('eu');
  await settleSave();
  await expectDelta(beforeProviderEdit, {recompute_costs: 1}, 'editing an enabled provider default recomputes costs exactly once', deferredCostQueries);
  await assertPricingReturn(beforeProviderEdit, pricingSelection, 'editing a provider default');

  await openCosts();
  const beforeProviderRemoval = await calls();
  await page.getByRole('button', {name: '删除', exact: true}).click();
  await settleSave();
  await expectDelta(beforeProviderRemoval, {recompute_costs: 1}, 'removing an enabled provider default recomputes costs exactly once', deferredCostQueries);
  await assertPricingReturn(beforeProviderRemoval, pricingSelection, 'removing a provider default');

  await go(1);
  await trendReset.click();
  await page.clock.runFor(500);
  await go(0);
  await page.locator('.today-range-reset').click();
  await page.clock.runFor(500);
  await go(4);
  await page.locator('[data-settings-section="general"]').click();
}
