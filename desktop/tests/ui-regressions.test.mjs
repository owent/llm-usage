import test from 'node:test';
import assert from 'node:assert/strict';
import { calendarDay, offsetDay, calendarYearRange } from '../src/lib/calendar.ts';
import { loadPanelGroup, savePanelGroup, clearPanelPage } from '../src/lib/panels.ts';
import { durationStatsOf, pivotChartSeries, mergeNamedValues, observedTokenTotal, tokenTotalLabel } from '../src/lib/derive.ts';
import {modelKey} from '../src/lib/model-names.ts';
import { escapeHtml } from '../src/lib/chart.ts';
import {formatAmount,formatCostRange,formatUnitPrice,unpricedReasonKey} from '../src/lib/costs.ts';
import {cnyToUsd,referenceFx} from '../src/lib/exchange-rates.ts';

test('compact total labels distinguish complete, lower-bound and missing usage',()=>{
  assert.equal(tokenTotalLabel('en-US',120,100,20),'120');
  assert.equal(tokenTotalLabel('en-US',null,100,20),'≥ 120');
  assert.equal(tokenTotalLabel('en-US',null,0,0),'≥ 0');
  assert.equal(tokenTotalLabel('en-US',null,null,20),'—');
  assert.equal(tokenTotalLabel('en-US','9007199254740993',null,null),'9,007,199,254,740,993');
});

test('statistics dates follow the selected timezone across midnight and DST', () => {
  const time = new Date('2026-09-27T23:30:00Z');
  assert.equal(calendarDay(time, 'Asia/Shanghai'), '2026-09-28');
  assert.equal(calendarDay(time, 'America/Los_Angeles'), '2026-09-27');
  assert.equal(offsetDay('2026-03-09', -2), '2026-03-07');
  assert.equal(offsetDay('2024-03-01', -1), '2024-02-29');
  assert.equal(offsetDay('2026-01-01', -1), '2025-12-31');
});

test('activity calendar uses an entire local calendar year including leap day', () => {
  assert.deepEqual(calendarYearRange('2026-09-28'), { year: 2026, first_day: '2026-01-01', last_day: '2026-12-31' });
  assert.deepEqual(calendarYearRange('2026-09-28', 2024), { year: 2024, first_day: '2024-01-01', last_day: '2024-12-31' });
  const range = calendarYearRange('2024-03-01');
  assert.equal((Date.parse(range.last_day)-Date.parse(range.first_day))/86400000+1, 366);
});

test('panel order survives restart; stale ids, duplicates and corrupt dimensions are removed', () => {
  const records = new Map();
  globalThis.localStorage = { getItem: (key) => records.get(key) ?? null, setItem: (key,value) => records.set(key,value), removeItem: (key) => records.delete(key) };
  savePanelGroup('overview','today',{order:['c','a','c','removed'],hidden:['a','removed'],sizes:{a:{span:9,height:50},c:{span:4,height:320}}});
  const loaded = loadPanelGroup('overview','today',['a','b','c','new']);
  assert.deepEqual(loaded,{order:['c','a','b','new'],hidden:['a'],sizes:{c:{span:4,height:320}}});
  savePanelGroup('overview','history',{order:['d'],hidden:[],sizes:{}});
  assert.deepEqual(loadPanelGroup('overview','today',['a','b','c','new']),loaded);
  clearPanelPage('overview');
  assert.deepEqual(loadPanelGroup('overview','today',['a','b']).order,['a','b']);
  globalThis.localStorage = { getItem() { throw new Error('unavailable'); } };
  assert.deepEqual(loadPanelGroup('overview','today',['a']).order,['a']);
});

test('duration mean uses only known samples and preserves unknown values', () => {
  assert.deepEqual(durationStatsOf([{sums:{total_duration_ms:'100',duration_sample_count:1,call_count:100}},{sums:{total_duration_ms:'900',duration_sample_count:3,call_count:200}}]),{avgMs:250,totalMs:1000});
  assert.deepEqual(durationStatsOf([{sums:{total_duration_ms:null,duration_sample_count:0,call_count:1}}]),{avgMs:null,totalMs:null});
});

test('chart pivot preserves unknown token values and exact series names', () => {
  const grouped = pivotChartSeries([{label:'2026-09-27',series:'model-a',calls:2,input:null,cache_read:null,output:'0',total:null}]);
  assert.deepEqual(grouped.cell('model-a','2026-09-27'),{calls:2,input:null,cacheRead:null,cacheWrite:null,uncached:null,cacheRatio:null,output:0,total:null});
  assert.equal(grouped.cell('missing','2026-09-27'),undefined);
});

test('name-only shares combine provider rows without splitting case', () => {
  assert.deepEqual(mergeNamedValues([{name:'GPT',value:100},{name:'gpt',value:50},{name:'Other',value:10}]),[{name:'gpt',value:150},{name:'other',value:10}]);
});

test('source-controlled chart labels cannot inject tooltip HTML', () => {
  assert.equal(escapeHtml('<img src=x onerror="alert(1)">&'), '&lt;img src=x onerror=&quot;alert(1)&quot;&gt;&amp;');
});

test('unit rates preserve small prices, known zero and unavailable components',()=>{
  assert.equal(formatUnitPrice('en','USD',750),'USD 0.075');
  assert.equal(formatUnitPrice('en','USD',36),'USD 0.0036');
  assert.equal(formatUnitPrice('en','CNY',80000),'CNY 8');
  assert.equal(formatUnitPrice('en','USD',0),'USD 0');
  assert.equal(formatUnitPrice('en','USD',null),'—');
});

test('currency formatting distinguishes real zero from chart gaps and invalid values',()=>{
  assert.equal(formatAmount('en','USD',0),'$0.00');
  assert.equal(formatAmount('en','USD',123),'$1.23');
  for(const value of [null,undefined,'-','',NaN,Infinity,'0']) {
    assert.equal(formatAmount('en','USD',value),'—');
  }
});

test('archived tariff bounds remain a range including zero and partial amounts',()=>{
  assert.equal(formatCostRange('en',{currency:'USD',total_amount_minor:1200,upper_amount_minor:1900}),'$12.00 – $19.00');
  assert.equal(formatCostRange('en',{currency:'USD',total_amount_minor:0,upper_amount_minor:0}),'$0.00 – $0.00');
  assert.equal(formatCostRange('en',{currency:'USD',total_amount_minor:30,upper_amount_minor:null}),'$0.30');
});

test('CNY reference conversion preserves units and uses same-day ECB cross rates',()=>{
  assert.equal(referenceFx.date,'2026-10-02');
  // EUR 1 = CNY 7.5259 = USD 1.1225, not the inverse conversion.
  assert.equal(cnyToUsd('CNY',75259),11225);
  assert.equal(cnyToUsd('CNY',200000),29830); // CNY 20/M → USD 2.9830/M
  assert.equal(cnyToUsd('CNY',10000),1492); // CNY 100 → USD 14.92
  assert.equal(cnyToUsd('CNY',0),0);
  for(const value of [null,-1,1.5,Number.NaN,Number.POSITIVE_INFINITY,Number.MAX_SAFE_INTEGER+1]) assert.equal(cnyToUsd('CNY',value),null);
  assert.equal(cnyToUsd('USD',10000),null);
  assert.equal(cnyToUsd('EUR',10000),null);
});

test('unpriced explanations distinguish missing usage from missing rates',()=>{
  assert.equal(unpricedReasonKey('no_known_usage'),'cost.reason.no_known_usage');
  assert.equal(unpricedReasonKey('no_price_row'),'cost.reason.no_price_row');
  assert.equal(unpricedReasonKey('future-reason'),'cost.reason.other');
});

test('observed total lower bounds preserve zeros, unknowns and integer precision',()=>{
  assert.equal(observedTokenTotal('67162','14715'),'81877');
  assert.equal(observedTokenTotal('0','0'),'0');
  assert.equal(observedTokenTotal('9007199254740993','1'),'9007199254740994');
  assert.equal(observedTokenTotal(null,'10'),null);
  assert.equal(observedTokenTotal('10',null),null);
  assert.equal(observedTokenTotal('-1','10'),null);
});

test('model lookup normalizes separators without guessing releases or suffixes',()=>{
  assert.equal(modelKey(' Claude_Opus 4.8 '),'claude-opus-4-8');
  assert.equal(modelKey('GPT_6.1_sol'),'gpt-6.1-sol');
  assert.notEqual(modelKey('gpt-6-1-sol'),modelKey('gpt-6.1-sol'));
  assert.equal(modelKey('hy4-preview-f'),'hy4-preview-f');
});
