<script lang="ts">
  import { onMount } from 'svelte';
  import { setChartOption, setupTooltipAutoHide, setupRangeSelection, escapeHtml, CHART_PALETTE } from '../lib/chart';
  import * as echarts from 'echarts/core';
  import { LineChart } from 'echarts/charts';
  import type { LineSeriesOption } from 'echarts/charts';
  import { DataZoomComponent, GridComponent, LegendComponent, TooltipComponent } from 'echarts/components';
  import { CanvasRenderer } from 'echarts/renderers';
  import { api, parseError } from '../lib/api';
  import type { ChartDimension, PeriodDto, SummaryQuery } from '../lib/api';
  import { t, i18n, fmtSmart, fmtPrecise, fmtDurationShort } from '../lib/i18n.svelte';
  import { durationStatsOf, pivotChartSeries, type ChartGroupData } from '../lib/derive';
  import DimensionPicker from './DimensionPicker.svelte';

  echarts.use([LineChart, GridComponent, LegendComponent, TooltipComponent, DataZoomComponent, CanvasRenderer]);

  let {
    periods,
    query,
    granularity,
    isDark = false,
    onperiodclick,
    onrangechange,
  }: {
    periods: PeriodDto[];
    /** 当前查询（chart_series 分组数据用；随筛选/范围变化重新拉取）。 */
    query: SummaryQuery;
    granularity: 'hour' | 'day' | 'week' | 'month';
    /** 深色主题（父级传入；变化时重绘轴/legend 文字与分隔线）。 */
    isDark?: boolean;
    /** 点击数据点回调（携带该点的时间轴标签；总用量/分组两模式均生效）。 */
    onperiodclick?: (label: string) => void;
    onrangechange?: (first:string,last:string)=>void;
  } = $props();

  let dimension = $state<ChartDimension>('total');
  let grouped = $state<ChartGroupData | null>(null);
  let groupedError = $state('');
  let groupedScope = '';

  let el: HTMLDivElement;
  let chart: echarts.ECharts | null = null;

  /**
   * 增量渲染（2026-09-26 用户反馈）：渲染结构签名（维度/标签/系列名/主题/语言）
   * 与上次相同 → setOption 合并更新（ECharts 内部 diff，不整图重建）；结构变化
   * → notMerge 整体重建。数据刷新时避免可见闪烁。
   */
  let lastRenderKey = '';

  function applyOption(option: echarts.EChartsCoreOption, key: string): void {
    if (!chart) return;
    if (key === lastRenderKey) setChartOption(chart, isDark, option);
    else setChartOption(chart, isDark, option, { notMerge: true });
    lastRenderKey = key;
  }

  const duration = $derived(durationStatsOf(periods));

  /** 主题感知色：全局文字（axis/legend 继承）与 y 轴分隔线。 */
  const chartText = $derived(isDark ? '#b0bfd4' : '#5b6a82');
  const splitColor = $derived(isDark ? '#2c3a52' : '#e8edf5');

  const PALETTE = CHART_PALETTE;

  // 分组数据：dimension/query 变化时经 chart_series 拉取；失败时保底显示错误文案。
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
        dataZoom: granularity === 'hour' || granularity === 'day' ? [{ type: 'inside' }] : [],
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
        dataZoom: [{ type: 'inside' }],
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
    chart = echarts.init(el, i18n.locale === 'zh-CN' ? 'ZH' : 'EN');

    // Tooltip：hideDelay 0（ECharts 6 手动 hideTip 也走 hideLater(hideDelay)，不可用大值）+ 离开画布/移出窗口/失焦即隐藏（统一封装）。
    const disposeTipHide = setupTooltipAutoHide(chart!);
    // 数据点/横轴任意位置点击：网格内像素 → 最近类目索引（不要求命中数据点，
    // 2026-09-26 用户需求）；zr 级事件覆盖整个画布，legend/坐标轴外区域被
    // containPixel('grid') 排除。
    const disposeSelection=setupRangeSelection(chart,()=>dimension==='total'?periods.map((p)=>p.label):grouped?.labels??[],
      (label)=>onperiodclick?.(label),(first,last)=>onrangechange?.(first,last));
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

  $effect(() => {
    void periods;
    void granularity;
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
<div bind:this={el} class="calls-chart"></div>
<p class="dur-summary">
  {t('panel.durationSummary', {
    avg: fmtDurationShort(duration.avgMs),
    total: fmtDurationShort(duration.totalMs),
  })}
</p>

<style>
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
