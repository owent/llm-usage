<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { setChartOption, setupTooltipAutoHide, setupRangeSelection, showRangeSelection, enableRangeBrush, RANGE_BRUSH, escapeHtml, CHART_PALETTE } from '../lib/chart';
  import * as echarts from 'echarts/core';
  import { LineChart } from 'echarts/charts';
  import type { LineSeriesOption } from 'echarts/charts';
  import { BrushComponent, GridComponent, LegendComponent, TooltipComponent } from 'echarts/components';
  import { CanvasRenderer } from 'echarts/renderers';
  import { api, parseError } from '../lib/api';
  import type { ChartDimension, SummaryQuery } from '../lib/api';
  import { t, i18n, fmtSmart, fmtPrecise } from '../lib/i18n.svelte';
  import { pivotChartSeries, tokenTotalLabel, type ChartGroupData } from '../lib/derive';
  import DimensionPicker from './DimensionPicker.svelte';

  echarts.use([LineChart, GridComponent, LegendComponent, TooltipComponent, BrushComponent, CanvasRenderer]);

  let {
    hourly,
    query,
    isDark = false,
    selectedRange = null,
    onrangechange,
  }: {
    hourly: {
      hour: number;
      calls: number;
      total_tokens: string | null;
      input_total: string | null;
      cache_read: string | null;
      output_total: string | null;
      sessions: number | null;
      avg_duration_ms: string | null;
    }[];
    /** 今日查询（first=last=今天；分组维度的 chart_series 数据源）。 */
    query: SummaryQuery;
    /** 深色主题（父级传入；变化时重绘轴/legend 文字与分隔线）。 */
    isDark?: boolean;
    selectedRange?: {first:string;last:string} | null;
    onrangechange?: (first:string,last:string)=>void;
  } = $props();

  let dimension = $state<ChartDimension>('total');
  let grouped = $state<ChartGroupData | null>(null);
  let groupedError = $state('');
  let groupedScope = '';

  let el: HTMLDivElement;
  let chart: echarts.ECharts | null = null;

  /**
   * 增量渲染（2026-09-26 用户反馈）：渲染结构签名与上次相同 → setOption 合并
   * 更新（不整图重建）；结构变化 → notMerge 重建。数据刷新时避免闪烁。
   */
  let lastRenderKey = '';

  function applyOption(option: echarts.EChartsCoreOption, key: string): void {
    if (!chart) return;
    option = {...option, brush: RANGE_BRUSH, toolbox:{show:false}};
    if (key === lastRenderKey) setChartOption(chart, isDark, option);
    else setChartOption(chart, isDark, option, { notMerge: true });
    enableRangeBrush(chart);
    untrack(()=>showRangeSelection(chart!,selectionLabels(),selectedRange));
    lastRenderKey = key;
  }

  /** 主题感知色：全局文字（axis/legend 继承）与 y 轴分隔线。 */
  const chartText = $derived(isDark ? '#b0bfd4' : '#5b6a82');
  const splitColor = $derived(isDark ? '#2c3a52' : '#e8edf5');

  const PALETTE = CHART_PALETTE;

  /** 有数据的小时桶（call_count>0 或 total_tokens 非空；无数据小时不补零）。 */
  const activeHours = $derived(
    hourly
      .filter((h) => h.calls > 0 || h.total_tokens !== null)
      .slice()
      .sort((a, b) => a.hour - b.hour)
  );

  // 分组数据：dimension/query 变化时经 chart_series 拉取；失败时保底显示错误文案。
  $effect(() => {
    const q = query;
    void hourly;
    void dimension;
    const scope = JSON.stringify([q, dimension]);
    if (groupedScope !== scope) grouped = null;
    groupedScope = scope;
    groupedError = '';
    if (dimension === 'total') return;
    const dim = dimension;
    let cancelled = false;
    api
      .chartSeries({ ...q, granularity: 'hour' }, dim)
      .then((r) => {
        if (!cancelled) grouped = pivotChartSeries(r.rows);
      })
      .catch((e) => {
        if (!cancelled) groupedError = parseError(e);
      });
    return () => {
      cancelled = true;
    };
  });

  function renderTotal() {
    if (!chart) return;
    const rows = activeHours;
    if (!rows.length) {
      chart.clear();
      lastRenderKey = '';
      return;
    }
    const labels = rows.map((h) => `${query.last_day} ${String(h.hour).padStart(2, '0')}:00`);
    const calls = rows.map((h) => h.calls);
    const tokens = rows.map((h) => h.total_tokens === null ? null : Number(h.total_tokens));
    applyOption(
      {
        tooltip: {
          trigger: 'axis',
          hideDelay: 0, transitionDuration: 0,
          formatter: (params: { dataIndex: number }[]) => {
            const h = rows[params[0]?.dataIndex ?? 0];
            if (!h) return '';
            return [
              `<b>${String(h.hour).padStart(2, '0')}:00</b>`,
              `${t('trend.calls')}: ${fmtPrecise(h.calls)}`,
              `${t('cards.total')}: ${fmtPrecise(h.total_tokens)}`,
              `${t('cards.input')}: ${fmtPrecise(h.input_total)}`,
              `${t('cards.cacheRead')}: ${fmtPrecise(h.cache_read)}`,
              `${t('cards.output')}: ${fmtPrecise(h.output_total)}`,
              `${t('trend.sessions')}: ${fmtPrecise(h.sessions)}`,
            ].join('<br/>');
          },
        },
        legend: { top: 0, left: 'center', type: 'scroll', itemWidth: 12, itemHeight: 8, textStyle: { fontSize: 11 } },
        textStyle: { color: chartText },
        grid: { left: 70, right: 80, top: 48, bottom: 28, containLabel: true },
        xAxis: { type: 'category', data: labels, triggerEvent:true, axisLabel:{formatter:(label:string)=>label.slice(-5)} },
        yAxis: [
          {
            type: 'value',
            name: t('trend.calls'),
            nameGap: 14,
            nameTextStyle: { align: 'left' },
            axisLabel: { formatter: (v: number) => fmtSmart(v) },
            splitLine: { lineStyle: { color: splitColor } },
          },
          {
            type: 'value',
            name: t('trend.tokens'),
            splitLine: { show: false },
            axisLabel: { formatter: (v: number) => fmtSmart(v) },
          },
        ],
        series: [
          {
            type: 'line',
            name: t('trend.calls'),
            smooth: true,
            symbolSize: 4,
            itemStyle: { color: '#1a56c4' },
            data: calls,
          },
          {
            type: 'line',
            name: t('trend.tokens'),
            yAxisIndex: 1,
            smooth: true,
            symbolSize: 4,
            itemStyle: { color: '#3f8f5f' },
            data: tokens,
          },
        ],
      },
      `total|${i18n.locale}|${chartText}|${labels.join('\u0001')}`
    );
  }


  /** Every grouped view uses the same hourly token metric, including one point. */
  function renderGroupedByTime(g: ChartGroupData) {
    const series: LineSeriesOption[] = [];
    g.names.forEach((name, i) => {
      const color = PALETTE[i % PALETTE.length];
      series.push({
        type: 'line',
        name,
        smooth: true,
        symbolSize: 5,
        showSymbol: true,
        connectNulls: false,
        itemStyle: { color },
        data: g.labels.map((l) => g.cell(name, l)?.total ?? null),
      });
    });
    applyOption(
      {
        tooltip: {
          trigger: 'axis',
          hideDelay: 0, transitionDuration: 0,
          formatter: (params: { dataIndex: number; marker: string; seriesName?: string; value: number | null }[]) => {
            const label = g.labels[params[0]?.dataIndex ?? 0] ?? '';
            const lines = [`<b>${escapeHtml(label)}</b>`];
            const markers=new Map(params.map(p=>[p.seriesName,p.marker]));
            let hasLowerBound=false;
            const selected = (chart?.getOption().legend as { selected?: Record<string, boolean> }[] | undefined)?.[0]?.selected ?? {};
            for (const name of g.names) {
              const cell = g.cell(name, label);
              if (!cell || selected[name] === false) continue;
              const total=tokenTotalLabel(i18n.locale,cell.total,cell.input,cell.output);
              hasLowerBound ||= total.startsWith('≥');
              lines.push(`${markers.get(name)??''}${escapeHtml(name)}: ${total}`);
            }
            if(hasLowerBound) lines.push(`<small>${escapeHtml(t('tokens.lowerBound'))}</small>`);
            return lines.join('<br/>');
          },
        },
        legend: { top: 0, left: 'center', type: 'scroll', itemWidth: 12, itemHeight: 8, textStyle: { fontSize: 11 } },
        textStyle: { color: chartText },
        grid: { left: 70, right: 80, top: 48, bottom: 28, containLabel: true },
        xAxis: { type: 'category', data: g.labels, triggerEvent:true, axisLabel:{formatter:(label:string)=>label.slice(-5)} },
        yAxis: [
          {
            type: 'value',
            name: t('trend.tokens'),
            nameGap: 14,
            nameTextStyle: { align: 'left' },
            axisLabel: { formatter: (v: number) => fmtSmart(v) },
            splitLine: { lineStyle: { color: splitColor } },
          },
        ],
        color: PALETTE,
        series,
      },
      `grpN|${dimension}|${i18n.locale}|${chartText}|${g.labels.join('\u0001')}|${g.names.join('\u0001')}`
    );
  }

  function render() {
    if (!chart) return;
    if (dimension === 'total') {
      renderTotal();
      return;
    }
    if (!grouped || groupedError) {
      chart.clear();
      lastRenderKey = '';
      return;
    }
    const g = grouped;
    if (!g.names.length) {
      chart.clear();
      lastRenderKey = '';
      return;
    }
    renderGroupedByTime(g);
  }

  function selectionLabels(): string[] {
    return dimension === 'total' ? activeHours.map((h)=>`${query.last_day} ${String(h.hour).padStart(2,'0')}:00`) : grouped?.labels ?? [];
  }

  onMount(() => {
    chart = echarts.init(el, i18n.locale === 'zh-CN' ? 'ZH' : 'EN');

    // Tooltip：hideDelay 0（ECharts 6 手动 hideTip 也走 hideLater(hideDelay)，不可用大值）+ 离开画布/移出窗口/失焦即隐藏（统一封装）。
    const disposeTipHide = setupTooltipAutoHide(chart!);
    const disposeSelection=setupRangeSelection(chart,selectionLabels,(label)=>onrangechange?.(label,label),(first,last)=>onrangechange?.(first,last));
    render();
    const onResize = () => chart?.resize();
    window.addEventListener('resize', onResize);
    // 面板显示/隐藏或网格变化时容器尺寸变化（含 display:none 恢复），自动重设画布。
    const observer = new ResizeObserver(() => chart?.resize());
    observer.observe(el);
    return () => {
      disposeTipHide();
      disposeSelection();
      observer.disconnect();
      window.removeEventListener('resize', onResize);
      chart?.dispose();
      chart = null;
    };
  });

  // 数据/维度/语言/主题变化时重绘。
  $effect(()=>{const selected=selectedRange;if(chart)showRangeSelection(chart,selectionLabels(),selected);});
  $effect(() => {
    void hourly;
    void dimension;
    void grouped;
    void i18n.locale;
    void isDark;
    render();
  });
</script>

<div class="dim-row">
  <DimensionPicker value={dimension} onselect={(v) => (dimension = v)} />
  {#if groupedError}
    <span class="dim-error">{t('chart.loadFailed', { message: groupedError })}</span>
  {/if}
</div>
{#if dimension!=='total'}<p class="coverage-hint" title={t('tokens.observedTotalHint')}>{t('tokens.lowerBound')}</p>{/if}
{#if dimension === 'total' && activeHours.length === 0}
  <p class="muted">{t('common.empty')}</p>
{/if}
<div bind:this={el} class="hourly" role="group" aria-label={`${t('hourly.title')}. ${t('dashboard.keyboardHint')}`}></div>

<style>
  .coverage-hint {font-size:11px;color:var(--text-muted);margin:2px 0 0;}
  /* 分组维度选择行（面板顶部；维度为分段按钮组，见 DimensionPicker）。 */
  .dim-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 0 0;
    flex-wrap: wrap;
    font-size: 13px;
  }
  .dim-error {
    color: var(--danger);
    font-size: 12px;
    overflow-wrap: anywhere;
  }
  .muted {
    color: var(--text-muted);
    font-size: 13px;
    margin: 4px 0 0;
  }
  .hourly {
    width: 100%;
    height: 280px;
    margin-top: 4px;
  }
</style>
