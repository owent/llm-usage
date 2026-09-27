<script lang="ts">
  import { onMount } from 'svelte';
  import { setChartOption, setupTooltipAutoHide, escapeHtml, CHART_PALETTE } from '../lib/chart';
  import * as echarts from 'echarts/core';
  import { BarChart, LineChart } from 'echarts/charts';
  import type { LineSeriesOption } from 'echarts/charts';
  import { GridComponent, LegendComponent, TooltipComponent } from 'echarts/components';
  import { CanvasRenderer } from 'echarts/renderers';
  import { api, parseError } from '../lib/api';
  import type { ChartDimension, SummaryQuery } from '../lib/api';
  import { t, i18n, fmtSmart, fmtPrecise } from '../lib/i18n.svelte';
  import { pivotChartSeries, type ChartGroupData } from '../lib/derive';
  import DimensionPicker from './DimensionPicker.svelte';

  echarts.use([BarChart, LineChart, GridComponent, LegendComponent, TooltipComponent, CanvasRenderer]);

  let {
    hourly,
    query,
    isDark = false,
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
    if (key === lastRenderKey) setChartOption(chart, isDark, option);
    else setChartOption(chart, isDark, option, { notMerge: true });
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
    const labels = rows.map((h) => `${String(h.hour).padStart(2, '0')}:00`);
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
        xAxis: { type: 'category', data: labels },
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
            type: 'bar',
            name: t('trend.calls'),
            barMaxWidth: 18,
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

  /** 单时间标签（今天，chart_series 按天聚合）：x = 分组名，双指标（调用+token）。 */
  function renderGroupedSingleLabel(g: ChartGroupData) {
    const label = g.labels[0];
    applyOption(
      {
        tooltip: {
          trigger: 'axis',
          hideDelay: 0, transitionDuration: 0,
          formatter: (params: { dataIndex: number }[]) => {
            const name = g.names[params[0]?.dataIndex ?? 0];
            const c = name === undefined ? undefined : g.cell(name, label);
            if (!c) return '';
            const miss = c.uncached;
            return [
              `<b>${escapeHtml(name)}</b> · ${escapeHtml(label)}`,
              `${t('trend.calls')}: ${fmtPrecise(c.calls)}`,
              `${t('cards.total')}: ${fmtPrecise(c.total)}`,
              `${t('cards.input')}: ${fmtPrecise(c.input)}`,
              `${t('cards.cacheRead')}: ${fmtPrecise(c.cacheRead)}`,
              `${t('cards.cacheMiss')}: ${fmtPrecise(miss)}`,
              `${t('cards.output')}: ${fmtPrecise(c.output)}`,
            ].join('<br/>');
          },
        },
        legend: { top: 0, left: 'center', type: 'scroll', itemWidth: 12, itemHeight: 8, textStyle: { fontSize: 11 } },
        textStyle: { color: chartText },
        grid: { left: 70, right: 80, top: 48, bottom: 28, containLabel: true },
        xAxis: { type: 'category', data: g.names },
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
        color: PALETTE,
        series: [
          {
            type: 'line',
            name: t('trend.calls'),
            smooth: true,
            symbolSize: 5,
            itemStyle: { color: '#1a56c4' },
            lineStyle: { width: 2 },
            data: g.names.map((n) => g.cell(n, label)?.calls ?? 0),
          },
          {
            type: 'line',
            name: t('trend.tokens'),
            yAxisIndex: 1,
            smooth: true,
            symbolSize: 5,
            itemStyle: { color: '#3f8f5f' },
            lineStyle: { width: 2 },
            data: g.names.map((n) => g.cell(n, label)?.total ?? null),
          },
        ],
      },
      `grp1|${dimension}|${i18n.locale}|${chartText}|${label}|${g.names.join('\u0001')}`
    );
  }

  /** 多时间标签：每分组两条折线（调用=左轴实线，token=右轴虚线，同组同色）。 */
  function renderGroupedByTime(g: ChartGroupData) {
    const series: LineSeriesOption[] = [];
    g.names.forEach((name, i) => {
      const color = PALETTE[i % PALETTE.length];
      series.push({
        type: 'line',
        name: `${name} · ${t('trend.calls')}`,
        smooth: true,
        symbolSize: 4,
        itemStyle: { color },
        data: g.labels.map((l) => g.cell(name, l)?.calls ?? 0),
      });
      series.push({
        type: 'line',
        name: `${name} · ${t('trend.tokens')}`,
        yAxisIndex: 1,
        smooth: true,
        symbolSize: 4,
        itemStyle: { color },
        lineStyle: { type: 'dashed', width: 1.5 },
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
            for (const p of params) {
              if (p.value !== null && p.value !== undefined && p.value !== 0) {
                lines.push(`${p.marker}${escapeHtml(p.seriesName)}: ${fmtPrecise(p.value)}`);
              }
            }
            return lines.join('<br/>');
          },
        },
        legend: { top: 0, left: 'center', type: 'scroll', itemWidth: 12, itemHeight: 8, textStyle: { fontSize: 11 } },
        textStyle: { color: chartText },
        grid: { left: 70, right: 80, top: 48, bottom: 28, containLabel: true },
        xAxis: { type: 'category', data: g.labels },
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
    if (g.labels.length > 1) renderGroupedByTime(g);
    else renderGroupedSingleLabel(g);
  }

  onMount(() => {
    chart = echarts.init(el, i18n.locale === 'zh-CN' ? 'ZH' : 'EN');

    // Tooltip：hideDelay 0（ECharts 6 手动 hideTip 也走 hideLater(hideDelay)，不可用大值）+ 离开画布/移出窗口/失焦即隐藏（统一封装）。
    const disposeTipHide = setupTooltipAutoHide(chart!);
    render();
    const onResize = () => chart?.resize();
    window.addEventListener('resize', onResize);
    // 面板显示/隐藏或网格变化时容器尺寸变化（含 display:none 恢复），自动重设画布。
    const observer = new ResizeObserver(() => chart?.resize());
    observer.observe(el);
    return () => {
      disposeTipHide();
      observer.disconnect();
      window.removeEventListener('resize', onResize);
      chart?.dispose();
      chart = null;
    };
  });

  // 数据/维度/语言/主题变化时重绘。
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
{#if dimension === 'total' && activeHours.length === 0}
  <p class="muted">{t('common.empty')}</p>
{/if}
<div bind:this={el} class="hourly"></div>

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
