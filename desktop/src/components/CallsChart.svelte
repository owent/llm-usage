<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { setChartOption, setupTooltipAutoHide, setupRangeSelection, showRangeSelection, enableRangeBrush, RANGE_BRUSH, escapeHtml, CHART_PALETTE } from '../lib/chart';
  import * as echarts from 'echarts/core';
  import { LineChart } from 'echarts/charts';
  import type { LineSeriesOption } from 'echarts/charts';
  import { BrushComponent, DataZoomComponent, GridComponent, LegendComponent, TooltipComponent } from 'echarts/components';
  import { SVGRenderer } from 'echarts/renderers';
  import { api, parseError } from '../lib/api';
  import type { ChartDimension, PeriodDto, SummaryQuery } from '../lib/api';
  import { t, i18n, fmtSmart, fmtPrecise, fmtDurationShort } from '../lib/i18n.svelte';
  import { durationStatsOf, pivotChartSeries, type ChartGroupData } from '../lib/derive';
  import DimensionPicker from './DimensionPicker.svelte';

  echarts.use([LineChart, GridComponent, LegendComponent, TooltipComponent, DataZoomComponent, BrushComponent, SVGRenderer]);

  let {
    periods,
    query,
    granularity,
    isDark = false,
    onperiodclick,
    onrangechange,
    selectedRange = null,
  }: {
    periods: PeriodDto[];
    /** Current query for grouped chart_series; reload on filter/range changes. */
    query: SummaryQuery;
    granularity: 'hour' | 'day' | 'week' | 'month';
    /** Parent-provided dark theme; redraw axes/legend text and separators when changed. */
    isDark?: boolean;
    /** Point-click callback receives the time label in both total and grouped views. */
    onperiodclick?: (label: string) => void;
    onrangechange?: (first:string,last:string)=>void;
    selectedRange?: {first:string;last:string} | null;
  } = $props();

  let dimension = $state<ChartDimension>('total');
  let grouped = $state<ChartGroupData | null>(null);
  let groupedError = $state('');
  let groupedScope = '';

  let el: HTMLDivElement;
  let chart: echarts.ECharts | null = null;

  /**
   * Incremental rendering for 2026-09-26 feedback: unchanged grouping/labels/series/theme/language
   * uses setOption merging and ECharts diff; changed structure uses notMerge rebuilds.
   * Refresh data without visibly rebuilding the chart.
   */
  let lastRenderKey = '';

  function applyOption(option: echarts.EChartsCoreOption, key: string): void {
    if (!chart) return;
    option = {...option, brush: RANGE_BRUSH, toolbox: {show:false}};
    if (key === lastRenderKey) setChartOption(chart, isDark, option);
    else setChartOption(chart, isDark, option, { notMerge: true });
    enableRangeBrush(chart);
    untrack(() => showRangeSelection(chart!, dimension === 'total' ? periods.map((p) => p.label) : grouped?.labels ?? [], selectedRange));
    lastRenderKey = key;
  }

  const duration = $derived(durationStatsOf(periods));

  /** Theme colors for global text inherited by axes/legend, and y-axis separators. */
  const chartText = $derived(isDark ? '#b0bfd4' : '#5b6a82');
  const splitColor = $derived(isDark ? '#2c3a52' : '#e8edf5');

  const PALETTE = CHART_PALETTE;

  // Fetch chart_series when dimension/query changes; show errors on failure.
  $effect(() => {
    const q = query;
    void periods;
    void dimension;
    const scope = JSON.stringify([q, dimension]);
    if (groupedScope !== scope) grouped = null;
    groupedScope = scope;
    groupedError = '';
    if (dimension === 'total') return;
    const dim = dimension;
    let cancelled = false;
    api
      .chartSeries(q, dim)
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
    if (!periods.length) { chart.clear(); lastRenderKey = ''; return; }
    const labels = periods.map((p) => {
      let label = p.label;
      if (p.in_progress) label += t('trend.inProgress');
      else if (p.partial_history) label += t('trend.partial');
      return label;
    });
    const calls = periods.map((p) => p.sums.call_count);
    const sessions = periods.map((p) => p.distinct_sessions);
    applyOption(
      {
        tooltip: {
          trigger: 'axis',
          hideDelay: 0, transitionDuration: 0,
          formatter: (params: { dataIndex: number }[]) => {
            const p = periods[params[0]?.dataIndex ?? 0];
            if (!p) return '';
            const lines = [
              `<b>${p.label}</b> (${p.start_day} ~ ${p.end_day})`,
              `${t('trend.calls')}: ${fmtPrecise(p.sums.call_count)}`,
              `${t('trend.sessions')}: ${p.distinct_sessions === null ? '—' : fmtPrecise(p.distinct_sessions)}`,
            ];
            if (p.sums.avg_duration_ms !== null) {
              lines.push(
                `${t('cards.avgDuration')}: ${fmtDurationShort(Number(p.sums.avg_duration_ms))}`
              );
            }
            return lines.join('<br/>');
          },
        },
        legend: { top: 0, left: 'center', type: 'scroll', itemWidth: 12, itemHeight: 8, textStyle: { fontSize: 11 } },
        textStyle: { color: chartText },
        grid: { left: 70, right: 80, top: 48, bottom: 44, containLabel: true },
        xAxis: { type: 'category', data: labels, triggerEvent:true },
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
            name: t('trend.sessions'),
            splitLine: { show: false },
            axisLabel: { formatter: (v: number) => fmtSmart(v) },
          },
        ],
        dataZoom: granularity === 'hour' || granularity === 'day' ? [{ type: 'inside', moveOnMouseMove:false }] : [],
        series: [
          {
            type: 'line',
            name: t('trend.calls'),
            smooth: true,
            symbolSize: 5,
            itemStyle: { color: '#1a56c4' },
            lineStyle: { width: 2 },
            data: calls,
          },
          {
            type: 'line',
            name: t('trend.sessions'),
            yAxisIndex: 1,
            smooth: true,
            symbolSize: 5,
            itemStyle: { color: '#3f8f5f' },
            lineStyle: { width: 2 },
            data: sessions,
          },
        ],
      },
      `total|${i18n.locale}|${chartText}|${granularity}|${labels.join('\u0001')}`
    );
  }

  function renderGrouped() {
    if (!chart || !grouped) return;
    const g = grouped;
    if (!g.labels.length) {
      chart.clear();
      lastRenderKey = '';
      return;
    }
    applyOption(
      {
        tooltip: {
          trigger: 'axis',
          hideDelay: 0, transitionDuration: 0,
          formatter: (params: { dataIndex: number; marker: string; seriesName?: string; value: number }[]) => {
            const label = g.labels[params[0]?.dataIndex ?? 0] ?? '';
            const lines = [`<b>${escapeHtml(label)}</b>`];
            for (const p of params) {
              if (p.value != null) lines.push(`${p.marker}${escapeHtml(p.seriesName)}: ${fmtPrecise(p.value)}`);
            }
            return lines.join('<br/>');
          },
        },
        legend: { top: 0, left: 'center', type: 'scroll', itemWidth: 12, itemHeight: 8, textStyle: { fontSize: 11 } },
        textStyle: { color: chartText },
        grid: { left: 70, right: 80, top: 48, bottom: 44, containLabel: true },
        xAxis: { type: 'category', data: g.labels, triggerEvent:true },
        yAxis: {
          type: 'value',
          name: t('trend.calls'),
          nameGap: 14,
          nameTextStyle: { align: 'left' },
          axisLabel: { formatter: (v: number) => fmtSmart(v) },
          splitLine: { lineStyle: { color: splitColor } },
        },
        dataZoom: [{ type: 'inside', moveOnMouseMove:false }],
        color: PALETTE,
        series: g.names.map(
          (name): LineSeriesOption => ({
            type: 'line',
            name,
            smooth: true,
            symbolSize: 4,
            lineStyle: { width: 1.5 },
            data: g.labels.map((label) => g.cell(name, label)?.calls ?? 0),
          })
        ),
      },
      `grp|${dimension}|${i18n.locale}|${chartText}|${g.labels.join('\u0001')}|${g.names.join('\u0001')}`
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
    renderGrouped();
  }

  onMount(() => {
    chart = echarts.init(el, i18n.locale === 'zh-CN' ? 'ZH' : 'EN', { renderer: 'svg' });

    // Tooltip hideDelay=0: ECharts 6 hideTip uses hideLater too; avoid large delays and hide on canvas/window leave or blur.
    const disposeTipHide = setupTooltipAutoHide(chart!);
    // Click anywhere inside the grid: pixels map to the nearest category, without hitting a point;
    // requested 2026-09-26. zr events span the canvas, while legend/outside-axis regions are
    // excluded by containPixel('grid').
    const disposeSelection=setupRangeSelection(chart,()=>dimension==='total'?periods.map((p)=>p.label):grouped?.labels??[],
      (label)=>onperiodclick?.(label),(first,last)=>onrangechange?.(first,last));
    render();
    const onResize = () => chart?.resize();
    window.addEventListener('resize', onResize);
    // Resize canvas after panel/grid size changes, including restoration from display:none.
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

  $effect(() => {
    void periods;
    void granularity;
    void dimension;
    void grouped;
    void i18n.locale;
    void isDark;
    render();
  });

  $effect(() => {
    const selected = selectedRange;
    if (chart) showRangeSelection(chart, dimension === 'total' ? periods.map((p) => p.label) : grouped?.labels ?? [], selected);
  });
</script>

<div class="dim-row">
  <DimensionPicker value={dimension} onselect={(v) => (dimension = v)} />
  {#if groupedError}
    <span class="dim-error">{t('chart.loadFailed', { message: groupedError })}</span>
  {/if}
</div>
<div bind:this={el} class="calls-chart" role="group" aria-label={`${t('trend.chart.calls')}. ${t('dashboard.keyboardHint')}`}></div>
<p class="dur-summary">
  {t('panel.durationSummary', {
    avg: fmtDurationShort(duration.avgMs),
    total: fmtDurationShort(duration.totalMs),
  })}
</p>

<style>
  /* Top grouping selector uses segmented buttons; see DimensionPicker. */
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
  .calls-chart {
    width: 100%;
    height: 320px;
    margin-top: 4px;
  }
  .dur-summary {
    margin: 2px 0 0;
    font-size: 12px;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }
</style>
