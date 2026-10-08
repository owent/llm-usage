/**
 * Hide chart tooltips on exit; browser behavior checked on 2026-09-27.
 * ECharts 6 manual hideTip also calls hideLater(hideDelay), so use hideDelay: 0.
 * The old 999999 value delayed every hide path. ECharts handles ordinary exits;
 * this wrapper handles missed events:
 * - globalout when the pointer leaves the canvas;
 * - document mousemove outside this chart, including its tooltip children,
 *   when selecting a point expands a summary and moves the canvas without mouseout,
 *   or WebView2 misses mouseout;
 * - document mouseout with no relatedTarget when leaving the application window;
 * - window blur when changing apps or clicking the title bar.
 * Returns cleanup for component unmount.
 */
import type { ECharts, EChartsCoreOption } from 'echarts/core';

/** Source names are data, including inside ECharts' HTML tooltips. */
export function escapeHtml(value: unknown): string {
  return String(value ?? '').replace(/[&<>"']/g, (c) => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;',
  })[c]!);
}

export const CHART_PALETTE = ['#5470e8', '#21a590', '#eda853', '#9270dc', '#dc7595', '#52a7ca', '#7b9561', '#ac8064'];

/** ECharts axes/legends have independent defaults. Apply theme colors to each;
 * global textStyle alone does not ensure readability on a dark canvas. */
export function setChartOption(chart: ECharts, dark: boolean, option: EChartsCoreOption, options: { notMerge?: boolean } = {}): void {
  const color = dark ? '#b0bfd4' : '#5b6a82';
  const border = dark ? '#34435d' : '#dfe6f0';
  type Axis = { axisLabel?: object; nameTextStyle?: object; axisLine?: { lineStyle?: object } };
  const paint = (axis: Axis) => ({ ...axis, axisLabel: { ...axis.axisLabel, color },
    nameTextStyle: { ...axis.nameTextStyle, color },
    axisLine: { ...axis.axisLine, lineStyle: { ...axis.axisLine?.lineStyle, color: border } } });
  const axes = (value: Axis | Axis[] | undefined) => value === undefined ? undefined : Array.isArray(value) ? value.map(paint) : paint(value);
  const legend = option.legend as { textStyle?: object } | undefined;
  chart.setOption({ ...option,
    animation: !window.matchMedia('(prefers-reduced-motion: reduce)').matches,
    animationDurationUpdate: 180,
    textStyle: { ...option.textStyle, color },
    xAxis: axes(option.xAxis as Axis | Axis[] | undefined), yAxis: axes(option.yAxis as Axis | Axis[] | undefined),
    legend: legend ? { ...legend, textStyle: { ...legend.textStyle, color }, inactiveColor: dark ? '#687b99' : '#a0acc0' } : undefined,
    tooltip: option.tooltip ? { ...option.tooltip, backgroundColor: dark ? '#1c2940' : '#ffffff', borderColor: border, textStyle: { color }, confine: true,
      extraCssText:'max-width:min(480px, calc(100vw - 48px));white-space:normal;overflow-wrap:anywhere;' } : undefined,
  }, options);
}

export function setupTooltipAutoHide(chart: ECharts): () => void {
  const dom = chart.getDom();
  let visible = false;
  const shown = () => { visible = true; };
  const hidden = () => { visible = false; };
  const hide = (): void => {
    if (!visible || chart.isDisposed()) return;
    visible = false;
    chart.dispatchAction({ type: 'hideTip' });
  };
  chart.on('showTip', shown);
  chart.on('hideTip', hidden);
  const onDocMouseMove = (e: MouseEvent): void => {
    // A target inside this chart or its tooltip means the pointer is still browsing it.
    if (e.target instanceof Node && dom.contains(e.target)) return;
    hide();
  };
  const onDocMouseOut = (e: MouseEvent): void => {
    if (!e.relatedTarget) hide();
  };
  chart.on('globalout', hide);
  document.addEventListener('mousemove', onDocMouseMove);
  document.addEventListener('mouseout', onDocMouseOut);
  window.addEventListener('blur', hide);
  return () => {
    chart.off('globalout', hide);
    chart.off('showTip', shown);
    chart.off('hideTip', hidden);
    document.removeEventListener('mousemove', onDocMouseMove);
    document.removeEventListener('mouseout', onDocMouseOut);
    window.removeEventListener('blur', hide);
  };
}

/**
 * For item-triggered pies/heatmaps, hide when zr mousemove has no target,
 * indicating blank canvas. Do not apply to axis-triggered charts, whose
 * grid areas intentionally retain tooltips.
 */
export function hideTooltipOnBlank(chart: ECharts): () => void {
  const onMove = (e: { target?: unknown }): void => {
    if (!e.target && !chart.isDisposed()) chart.dispatchAction({ type: 'hideTip' });
  };
  chart.getZr().on('mousemove', onMove);
  return () => {
    if (!chart.isDisposed()) chart.getZr().off('mousemove', onMove);
  };
}

/** Select periods on release; dragging does not pan or replace the full chart. */
export const RANGE_BRUSH = {
  xAxisIndex: 0, brushType: 'lineX', brushMode: 'single', transformable: false,
  toolbox: [], seriesIndex: [], removeOnClick: true,
  brushStyle: { color: 'rgba(84,112,232,0.12)', borderColor: '#5470e8', borderWidth: 1 },
};

export function enableRangeBrush(chart: ECharts): void {
  chart.dispatchAction({type: 'takeGlobalCursor', key: 'brush', brushOption: {brushType: 'lineX', brushMode: 'single'}});
}

export function showRangeSelection(chart: ECharts, labels: string[], selection: {first: string; last: string} | null): void {
  const indexOf = (label: string) => labels.findIndex((value) => value === label || value === label.slice(0, 10));
  const first = selection ? indexOf(selection.first) : -1;
  const last = selection ? indexOf(selection.last) : -1;
  chart.dispatchAction({type: 'brush', areas: first >= 0 && last >= 0
    ? [{brushType: 'lineX', xAxisIndex: 0, coordRange: [first, last]}] : []});
}

/** Point/axis selection, horizontal brushing and zoom share period labels. */
export function setupRangeSelection(chart:ECharts,labels:()=>string[],onpoint?:(label:string)=>void,onrange?:(first:string,last:string)=>void):()=>void {
  const dom = chart.getDom();
  dom.tabIndex = 0;
  let cursor = 0, anchor = 0;
  const keyboard = (event: KeyboardEvent) => {
    const list = labels();
    if (!list.length || !['ArrowLeft','ArrowRight','Home','End','Enter'].includes(event.key)) return;
    event.preventDefault();
    if (event.key === 'Enter') {
      const first = list[Math.min(anchor,cursor)], last = list[Math.max(anchor,cursor)];
      if (first === last) onpoint?.(first); else onrange?.(first,last);
      return;
    }
    cursor = event.key === 'Home' ? 0 : event.key === 'End' ? list.length-1 : Math.max(0,Math.min(list.length-1,cursor+(event.key === 'ArrowRight' ? 1 : -1)));
    if (!event.shiftKey) anchor = cursor;
    showRangeSelection(chart,list,{first:list[Math.min(anchor,cursor)],last:list[Math.max(anchor,cursor)]});
  };
  dom.addEventListener('keydown',keyboard);
  let suppressClick = false;
  let clickTimer: ReturnType<typeof setTimeout> | undefined;
  const pick=(index:number)=>{const label=labels()[index];if(label && !suppressClick){onpoint?.(label);chart.dispatchAction({type:'hideTip'});}};
  const brushEnd = (event: unknown) => {
    const area = (event as {areas?: {coordRange?: number[]}[]}).areas?.[0];
    if (!area?.coordRange || area.coordRange.length !== 2) return;
    const list = labels();
    const [low, high] = [...area.coordRange].sort((a, b) => a - b);
    if (!Number.isFinite(low) || !Number.isFinite(high) || !list.length) return;
    const first = list[Math.max(0, Math.min(list.length - 1, Math.round(low)))];
    const last = list[Math.max(0, Math.min(list.length - 1, Math.round(high)))];
    suppressClick = true;
    clearTimeout(clickTimer);
    clickTimer = setTimeout(() => { suppressClick = false; }, 0);
    onrange?.(first, last);
    chart.dispatchAction({type: 'hideTip'});
  };
  const click=(event:{offsetX:number;offsetY:number})=>{
    if(!chart.containPixel('grid',[event.offsetX,event.offsetY]))return;
    const index=chart.convertFromPixel({xAxisIndex:0},event.offsetX);
    if(typeof index==='number' && Number.isFinite(index))pick(Math.max(0,Math.min(labels().length-1,Math.round(index))));
  };
  const axis=(event:unknown)=>{const p=event as {componentType?:string;value?:string};if(p.componentType==='xAxis') {
    const index=labels().findIndex((label)=>p.value===label || p.value?.startsWith(label));if(index>=0)pick(index);
  }};
  const zoom=(event:unknown)=>{
    const e=event as {start?:number;end?:number;batch?:{start?:number;end?:number}[]};
    const range=e.batch?.[0]??e;
    const list=labels(); if(!list.length)return;
    const option=chart.getOption() as {dataZoom?:{startValue?:number|string;endValue?:number|string}[]};
    const current=option.dataZoom?.[0];
    const indexOf=(value:number|string|undefined,percent:number)=>typeof value==='string' ? list.indexOf(value) : value??Math.round(percent*(list.length-1)/100);
    const start=indexOf(current?.startValue,range.start??0);
    const end=indexOf(current?.endValue,range.end??100);
    const first=list[Math.max(0,Math.min(list.length-1,start))];const last=list[Math.max(0,Math.min(list.length-1,end))];
    if(first && last)onrange?.(first,last);
  };
  chart.getZr().on('click',click);chart.on('click',axis);chart.on('datazoom',zoom);chart.on('brushend',brushEnd);
  return ()=>{dom.removeEventListener('keydown',keyboard);clearTimeout(clickTimer);if(!chart.isDisposed()){chart.getZr().off('click',click);chart.off('click',axis);chart.off('datazoom',zoom);chart.off('brushend',brushEnd);}};
}
