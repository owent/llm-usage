<script lang="ts">
  import {onMount,untrack} from 'svelte';
  import * as echarts from 'echarts/core';
  import {LineChart} from 'echarts/charts';
  import {BrushComponent,DataZoomComponent,GridComponent,LegendComponent,TooltipComponent} from 'echarts/components';
  import {SVGRenderer} from 'echarts/renderers';
  import {setChartOption,setupTooltipAutoHide,setupRangeSelection,showRangeSelection,enableRangeBrush,RANGE_BRUSH,escapeHtml,CHART_PALETTE} from '../lib/chart';
  import {formatAmount} from '../lib/costs';
  import {i18n,t} from '../lib/i18n.svelte';
  import type {CostSummaryDto,PeriodDto} from '../lib/api';
  echarts.use([LineChart,GridComponent,LegendComponent,TooltipComponent,DataZoomComponent,BrushComponent,SVGRenderer]);
  let {summary,periods,isDark=false,onperiodclick,onrangechange,selectedRange=null}:{summary:CostSummaryDto|null;periods:PeriodDto[];isDark?:boolean;onperiodclick?:(label:string)=>void;onrangechange?:(first:string,last:string)=>void;selectedRange?:{first:string;last:string}|null}=$props();
  let el:HTMLDivElement;
  let chart:echarts.ECharts|null=null;
  let currency=$state('');
  let modelMode=$state(false);
  const currencies=$derived([...new Set((summary?.daily_current??[]).filter((r)=>r.sums.currency && r.sums.priced_event_count>0).map((r)=>r.sums.currency))].sort());
  const selectedCurrency=$derived(currencies.includes(currency)?currency:currencies[0]??'');
  // Reference curve groups current estimates by day; never assign a whole day to one hour.
  const dailyAxis=$derived(periods.some((p)=>/^\d{4}-\d{2}-\d{2} \d{2}:00$/.test(p.label)));
  const plotPeriods=$derived(dailyAxis ? [...new Map(periods.map((p)=>[p.start_day,{...p,label:p.start_day,end_day:p.start_day}])).values()] : periods);
  const groups=$derived.by(()=>{
    const map=new Map<string,{values:(number|null)[];upper:(number|null)[];bounded:boolean}>();
    for(const row of summary?.daily_current??[]) {
      if(row.sums.currency!==selectedCurrency || !row.sums.priced_event_count) continue;
      let low=0, high=plotPeriods.length;
      while(low<high) {const mid=(low+high)>>>1;if(plotPeriods[mid].start_day<=row.day)low=mid+1;else high=mid;}
      const index=low-1;
      if(index<0 || row.day>plotPeriods[index].end_day) continue;
      const name=modelMode ? row.model||t('common.unknown') : selectedCurrency;
      const group=map.get(name)??{values:plotPeriods.map(()=>null),upper:plotPeriods.map(()=>null),bounded:false};
      group.values[index]=(group.values[index]??0)+row.sums.total_amount_minor;
      group.upper[index]=(group.upper[index]??0)+(row.sums.upper_amount_minor??row.sums.total_amount_minor);
      group.bounded ||= row.sums.upper_amount_minor!=null;
      map.set(name,group);
    }
    return [...map].flatMap(([name,group])=>[
      {name,values:group.values,upper:false},
      ...(group.bounded?[{name:`${name} · ${t('cost.upperBound')}`,values:group.upper,upper:true}]:[]),
    ]);
  });
  function render() {
    if(!chart) return;
    if(!groups.length) {chart.clear();return;}
    setChartOption(chart,isDark,{
      brush:RANGE_BRUSH, toolbox:{show:false},
      color:CHART_PALETTE,
      tooltip:{trigger:'axis',hideDelay:0,transitionDuration:0,formatter:(params:{seriesName:string;value:unknown;dataIndex:number}[])=>{
        const period=plotPeriods[params[0]?.dataIndex??0];
        return [escapeHtml(period?.label??''),...params.map((p)=>`${escapeHtml(p.seriesName)}: ${formatAmount(i18n.locale,selectedCurrency,p.value)}`)].join('<br/>');
      }},
      legend:{type:'scroll',top:0},grid:{left:12,right:20,top:42,bottom:52,containLabel:true},
      xAxis:{type:'category',data:plotPeriods.map((p)=>p.label),triggerEvent:true},
      yAxis:{type:'value',name:selectedCurrency,axisLabel:{formatter:(v:number)=>(v/100).toLocaleString(i18n.locale)}},
      dataZoom:[{type:'slider',bottom:0,height:20},{type:'inside',moveOnMouseMove:false}],
      series:groups.map((group)=>({name:group.name,type:'line',lineStyle:{type:group.upper?'dashed':'solid'},showSymbol:true,symbolSize:7,connectNulls:false,data:group.values})),
    },{notMerge:true});
    enableRangeBrush(chart);
    untrack(()=>showRangeSelection(chart!,plotPeriods.map((p)=>p.label),selectedRange));
  }
  onMount(()=>{
    chart=echarts.init(el, undefined, { renderer: 'svg' });
    const disposeTip=setupTooltipAutoHide(chart);
    const disposeSelection=setupRangeSelection(chart,()=>plotPeriods.map((p)=>p.label),(label)=>onperiodclick?.(label),(a,b)=>onrangechange?.(a,b));
    const observer=new ResizeObserver(()=>chart?.resize()); observer.observe(el);render();
    return ()=>{disposeTip();disposeSelection();observer.disconnect();chart?.dispose();chart=null;};
  });
  $effect(()=>{void groups;void i18n.locale;void isDark;render();});
  $effect(()=>{const selected=selectedRange;if(chart)showRangeSelection(chart,plotPeriods.map((p)=>p.label),selected);});
</script>
<div class="controls">
  <strong title={t('dashboard.costCurveHint')}>{t('dashboard.costCurve')}</strong>
  {#if dailyAxis}<span>{t('filter.granularity.day')}</span>{/if}
  {#each currencies as item}<button class:active={selectedCurrency===item} onclick={()=>currency=item}>{item}</button>{/each}
  <button class:active={modelMode} onclick={()=>modelMode=!modelMode}>{t('chart.dimension.model')}</button>
</div>
<div bind:this={el} class="curve" style:height={groups.length ? '240px' : '4px'} role="group" aria-label={`${t('dashboard.costCurve')}. ${t('dashboard.keyboardHint')}`}></div>
{#if !groups.length}<p class="hint">{t('cost.noData')}</p>{/if}
{#if groups.some(group=>group.upper)}<p class="hint">{t('cost.tierRange')}</p>{/if}
<style>
  .controls {display:flex;align-items:center;flex-wrap:wrap;gap:8px;margin:12px 0 6px;font-size:12px;}
  button {padding:4px 8px;border:1px solid var(--border);border-radius:6px;background:var(--bg-input);color:var(--text-secondary);cursor:pointer;}
  button.active {background:var(--accent-bg);color:var(--accent);border-color:var(--accent);}
  .curve {width:100%;height:240px;min-width:0;}
  .hint {font-size:12px;color:var(--text-muted);margin:4px 0;}
</style>
