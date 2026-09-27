import test from 'node:test';
import assert from 'node:assert/strict';
import { calendarDay, offsetDay } from '../src/lib/calendar.ts';
import { loadPanelGroup, savePanelGroup, clearPanelPage } from '../src/lib/panels.ts';
import { durationStatsOf, pivotChartSeries, mergeNamedValues } from '../src/lib/derive.ts';
import { escapeHtml } from '../src/lib/chart.ts';

test('statistics dates follow the selected timezone across midnight and DST', () => {
  const time = new Date('2026-09-27T23:30:00Z');
  assert.equal(calendarDay(time, 'Asia/Shanghai'), '2026-09-28');
  assert.equal(calendarDay(time, 'America/Los_Angeles'), '2026-09-27');
  assert.equal(offsetDay('2026-03-09', -2), '2026-03-07');
  assert.equal(offsetDay('2024-03-01', -1), '2024-02-29');
  assert.equal(offsetDay('2026-01-01', -1), '2025-12-31');
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
