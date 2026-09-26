<script lang="ts">
  /**
   * 次数趋势图（任务 C5）：调用次数（柱）+ 会话数（折线）双 y 轴；
   * y 轴单位自动缩放（fmtSmart），tooltip 显示千分位精确值（任务 B3）；
   * 面板底部带平均耗时/总耗时文字摘要（任务 C6）。
   * 分组维度（2026-09-26）：总用量/按模型/按Agent/按Agent+模型；分组数据经
   * chart_series 命令从聚合表直读（低计算量），多系列 legend + 按系列 tooltip。
   * 布局修复：grid.right ≥ 80 给右侧 y 轴名留位，legend 左对齐可滚动，
   * 不再与右侧 y 轴文字重叠。
   */
  import { onMount } from 'svelte';
  import * as echarts from 'echarts/core';
  import { BarChart, LineChart } from 'echarts/charts';
  import type { BarSeriesOption } from 'echarts/charts';
  import { GridComponent, LegendComponent, TooltipComponent } from 'echarts/components';
  import { CanvasRenderer } from 'echarts/renderers';
  import { api, parseError } from '../lib/api';
  import type { ChartDimension, PeriodDto, SummaryQuery } from '../lib/api';
  import { t, i18n, fmtSmart, fmtPrecise, fmtDurationShort } from '../lib/i18n.svelte';
  import { durationStatsOf, pivotChartSeries, type ChartGroupData } from '../lib/derive';

  echarts.use([BarChart, LineChart, GridComponent, LegendComponent, TooltipComponent, CanvasRenderer]);

  let {
    periods,
    query,
    granularity,
  }: {
    periods: PeriodDto[];
    /** 当前查询（chart_series 分组数据用；随筛选/范围变化重新拉取）。 */
    query: SummaryQuery;
    granularity: 'hour' | 'day' | 'week' | 'month';
  } = $props();

  let dimension = $state<ChartDimension>('total');
  let grouped = $state<ChartGroupData | null>(null);
  let groupedError = $state('');

  let el: HTMLDivElement;
  let chart: echarts.ECharts | null = null;

  const duration = $derived(durationStatsOf(periods));

  const PALETTE = [
    '#1a56c4', '#3f8f5f', '#c9a227', '#b3601e', '#7a5fb0',
    '#2f8f8f', '#c46a9a', '#8a8f36', '#5d6b9e', '#a05f46',
  ];

  const dimensionOptions = $derived.by(
    () =>
      [
        ['total', t('chart.dimension.total')],
        ['model', t('chart.dimension.model')],
        ['agent', t('chart.dimension.agent')],
        ['agent_model', t('chart.dimension.agentModel')],
      ] as [ChartDimension, string][]
  );

  // 分组数据：dimension/query 变化时经 chart_series 拉取；失败时保底显示错误文案。
  $effect(() => {
    const q = query;
    void dimension;
    grouped = null;
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
    if (!chart || !periods.length) return;
    const labels = periods.map((p) => {
      let label = p.label;
      if (p.in_progress) label += t('trend.inProgress');
      else if (p.partial_history) label += t('trend.partial');
      return label;
    });
    const calls = periods.map((p) => p.sums.call_count);
    const sessions = periods.map((p) => p.distinct_sessions ?? 0);
    chart.setOption(
      {
        tooltip: {
          trigger: 'axis',
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
        legend: { top: 0, left: 8, type: 'scroll', itemWidth: 12, itemHeight: 8, textStyle: { fontSize: 11 } },
        grid: { left: 64, right: 80, top: 36, bottom: 44 },
        xAxis: { type: 'category', data: labels },
        yAxis: [
          { type: 'value', name: t('trend.calls'), axisLabel: { formatter: (v: number) => fmtSmart(v) } },
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
            type: 'bar',
            name: t('trend.calls'),
            barMaxWidth: 26,
            itemStyle: { color: '#1a56c4' },
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
      { notMerge: true }
    );
  }

  function renderGrouped() {
    if (!chart || !grouped) return;
    const g = grouped;
    if (!g.labels.length) {
      chart.clear();
      return;
    }
    chart.setOption(
      {
        tooltip: {
          trigger: 'axis',
          formatter: (params: { dataIndex: number; marker: string; seriesName?: string; value: number }[]) => {
            const label = g.labels[params[0]?.dataIndex ?? 0] ?? '';
            const lines = [`<b>${label}</b>`];
            for (const p of params) {
              if (p.value) lines.push(`${p.marker}${p.seriesName}: ${fmtPrecise(p.value)}`);
            }
            return lines.join('<br/>');
          },
        },
        legend: { top: 0, left: 8, type: 'scroll', itemWidth: 12, itemHeight: 8, textStyle: { fontSize: 11 } },
        grid: { left: 64, right: 80, top: 36, bottom: 44 },
        xAxis: { type: 'category', data: g.labels },
        yAxis: { type: 'value', name: t('trend.calls'), axisLabel: { formatter: (v: number) => fmtSmart(v) } },
        dataZoom: [{ type: 'inside' }],
        color: PALETTE,
        series: g.names.map(
          (name): BarSeriesOption => ({
            type: 'bar',
            name,
            barMaxWidth: 26,
            data: g.labels.map((label) => g.cell(name, label)?.calls ?? 0),
          })
        ),
      },
      { notMerge: true }
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
      return;
    }
    renderGrouped();
  }

  onMount(() => {
    chart = echarts.init(el, i18n.locale === 'zh-CN' ? 'ZH' : 'EN');
    render();
    const onResize = () => chart?.resize();
    window.addEventListener('resize', onResize);
    // 面板显示/隐藏或网格变化时容器尺寸变化（含 display:none 恢复），自动重设画布。
    const observer = new ResizeObserver(() => chart?.resize());
    observer.observe(el);
    return () => {
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
    render();
  });
</script>

<div class="dim-row">
  <label>{t('chart.dimension.label')}
    <select bind:value={dimension}>
      {#each dimensionOptions as [id, label] (id)}
        <option value={id}>{label}</option>
      {/each}
    </select>
  </label>
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
  /* 分组维度选择行（面板顶部）。 */
  .dim-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 0 0;
    flex-wrap: wrap;
    font-size: 12.5px;
  }
  .dim-row label {
    display: flex;
    align-items: center;
    gap: 4px;
    color: #444;
  }
  .dim-row select {
    padding: 2px 4px;
    font-size: 12.5px;
  }
  .dim-error {
    color: #b3261e;
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
    color: #777;
    font-variant-numeric: tabular-nums;
  }
</style>
