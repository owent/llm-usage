<script lang="ts">
  /**
   * token 趋势图（任务 C5）：四个子图（tab 切换）——
   * 总 token / 输入 token（缓存命中 + 未命中缓存堆叠面积）/ 输出 token / 缓存命中率%；
   * token 相关子图全部为平滑曲线（任务：曲线替代柱状；输入为堆叠面积图）；
   * y 轴 fmtSmart 缩放、tooltip 精确值（任务 B3）；
   * 面板底部带平均耗时/总耗时文字摘要（任务 C6）。
   * 分组维度（2026-09-26）：总用量/按模型/按Agent/按Agent+模型；分组数据经
   * chart_series 命令从聚合表直读，各分组按子图分别渲染（输入=命中+未命中堆叠
   * 面积、命中率=cache_read/input 折线），legend 显示分组名、tooltip 按系列显示；
   * 维度选择为分段按钮组（DimensionPicker，非下拉）。
   * 布局修复：grid.left ≥ 70 + containLabel 给 y 轴标签/轴名留位，legend 顶部
   * 居中（top 0 + left center），不再与左上 y 轴名重叠。
   */
  import { onMount } from 'svelte';
  import * as echarts from 'echarts/core';
  import { LineChart } from 'echarts/charts';
  import type { LineSeriesOption } from 'echarts/charts';
  import { DataZoomComponent, GridComponent, LegendComponent, TooltipComponent } from 'echarts/components';
  import { CanvasRenderer } from 'echarts/renderers';
  import { api, parseError } from '../lib/api';
  import type { ChartDimension, PeriodDto, SummaryQuery } from '../lib/api';
  import { t, i18n, fmtSmart, fmtPrecise, fmtPercent, fmtDurationShort } from '../lib/i18n.svelte';
  import { durationStatsOf, pivotChartSeries, type ChartGroupData } from '../lib/derive';
  import DimensionPicker from './DimensionPicker.svelte';

  echarts.use([LineChart, GridComponent, LegendComponent, TooltipComponent, DataZoomComponent, CanvasRenderer]);

  type TokenSub = 'total' | 'input' | 'output' | 'ratio';

  let {
    periods,
    query,
    granularity,
    isDark = false,
  }: {
    periods: PeriodDto[];
    /** 当前查询（chart_series 分组数据用；随筛选/范围变化重新拉取）。 */
    query: SummaryQuery;
    granularity: 'hour' | 'day' | 'week' | 'month';
    /** 深色主题（父级传入；变化时重绘轴/legend 文字与分隔线）。 */
    isDark?: boolean;
  } = $props();

  let sub = $state<TokenSub>('total');
  let dimension = $state<ChartDimension>('total');
  let grouped = $state<ChartGroupData | null>(null);
  let groupedError = $state('');

  let el: HTMLDivElement;
  let chart: echarts.ECharts | null = null;

  const duration = $derived(durationStatsOf(periods));

  /** 主题感知色：全局文字（axis/legend 继承）与 y 轴分隔线。 */
  const chartText = $derived(isDark ? '#aaa' : '#555');
  const splitColor = $derived(isDark ? '#3a3b3f' : '#e0e0e0');

  const PALETTE = [
    '#1a56c4', '#3f8f5f', '#c9a227', '#b3601e', '#7a5fb0',
    '#2f8f8f', '#c46a9a', '#8a8f36', '#5d6b9e', '#a05f46',
  ];

  const subTabs = $derived.by(
    () =>
      [
        ['total', t('trend.metric.total')],
        ['input', t('trend.metric.input')],
        ['output', t('trend.metric.output')],
        ['ratio', t('trend.metric.ratio')],
      ] as [TokenSub, string][]
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

    const base = {
      tooltip: {
        trigger: 'axis',
        hideDelay: 999999, transitionDuration: 0,
        formatter: (params: { dataIndex: number }[]) => {
          const p = periods[params[0]?.dataIndex ?? 0];
          if (!p) return '';
          const lines = [`<b>${p.label}</b> (${p.start_day} ~ ${p.end_day})`];
          if (sub === 'ratio') {
            lines.push(
              `${t('trend.metric.ratio')}: ${
                p.sums.cache_input_ratio === null ? '—' : fmtPercent(p.sums.cache_input_ratio)
              }`
            );
          } else if (sub === 'input') {
            lines.push(
              `${t('cards.input')}: ${fmtPrecise(p.sums.input_total_known)}`,
              `${t('cards.cacheRead')}: ${fmtPrecise(p.sums.cache_read_known)}`,
              `${t('cards.cacheMiss')}: ${fmtPrecise(p.sums.uncached_known)}`
            );
          } else if (sub === 'output') {
            lines.push(`${t('cards.output')}: ${fmtPrecise(p.sums.output_total_known)}`);
          } else {
            lines.push(`${t('cards.total')}: ${fmtPrecise(p.sums.total_tokens_known)}`);
          }
          return lines.join('<br/>');
        },
      },
      grid: { left: 70, right: 80, top: 48, bottom: 44, containLabel: true },
      textStyle: { color: chartText },
      xAxis: { type: 'category', data: labels },
      dataZoom: granularity === 'hour' || granularity === 'day' ? [{ type: 'inside' }] : [],
    };

    if (sub === 'ratio') {
      chart.setOption(
        {
          ...base,
          yAxis: {
            type: 'value',
            name: t('trend.metric.ratio'),
            nameGap: 14,
            nameTextStyle: { align: 'left' },
            axisLabel: { formatter: (v: number) => `${v}%` },
            splitLine: { lineStyle: { color: splitColor } },
          },
          series: [
            {
              type: 'line',
              name: t('trend.metric.ratio'),
              smooth: true,
              symbolSize: 5,
              connectNulls: false,
              itemStyle: { color: '#c9a227' },
              areaStyle: { opacity: 0.08 },
              data: periods.map((p) =>
                p.sums.cache_input_ratio === null ? null : p.sums.cache_input_ratio * 100
              ),
            },
          ],
        },
        { notMerge: true }
      );
      return;
    }

    if (sub === 'input') {
      // 输入 token：缓存命中（底）+ 未命中（上）堆叠面积图。
      chart.setOption(
        {
          ...base,
          legend: { top: 0, left: 'center', type: 'scroll', itemWidth: 12, itemHeight: 8, textStyle: { fontSize: 11 } },
          yAxis: {
            type: 'value',
            name: t('trend.metric.input'),
            nameGap: 14,
            nameTextStyle: { align: 'left' },
            axisLabel: { formatter: (v: number) => fmtSmart(v) },
            splitLine: { lineStyle: { color: splitColor } },
          },
          series: [
            {
              type: 'line',
              name: t('cards.cacheRead'),
              stack: 'input',
              smooth: true,
              symbol: 'none',
              lineStyle: { width: 1.5 },
              itemStyle: { color: '#7aa5e8' },
              areaStyle: { opacity: 0.45 },
              data: periods.map((p) => Number(p.sums.cache_read_known ?? 0)),
            },
            {
              type: 'line',
              name: t('cards.cacheMiss'),
              stack: 'input',
              smooth: true,
              symbol: 'none',
              lineStyle: { width: 1.5 },
              itemStyle: { color: '#1a56c4' },
              areaStyle: { opacity: 0.35 },
              data: periods.map((p) => Number(p.sums.uncached_known ?? 0)),
            },
          ],
        },
        { notMerge: true }
      );
      return;
    }

    const metric = sub === 'output' ? 'output' : 'total';
    chart.setOption(
      {
        ...base,
        yAxis: {
          type: 'value',
          name: sub === 'output' ? t('trend.metric.output') : t('trend.metric.total'),
          nameGap: 14,
          nameTextStyle: { align: 'left' },
          axisLabel: { formatter: (v: number) => fmtSmart(v) },
          splitLine: { lineStyle: { color: splitColor } },
        },
        series: [
          {
            type: 'line',
            name: sub === 'output' ? t('trend.metric.output') : t('trend.metric.total'),
            smooth: true,
            symbolSize: 3,
            lineStyle: { width: 2 },
            itemStyle: { color: sub === 'output' ? '#3f8f5f' : '#1a56c4' },
            data: periods.map((p) =>
              metric === 'output'
                ? Number(p.sums.output_total_known ?? 0)
                : Number(p.sums.total_tokens_known ?? 0)
            ),
          },
        ],
      },
      { notMerge: true }
    );
  }

  /** 分组模式通用 tooltip：按系列显示精确值（null 点不参与）。 */
  function groupedTooltip(g: ChartGroupData) {
    return (params: { dataIndex: number; marker: string; seriesName?: string; value: number | null }[]) => {
      const label = g.labels[params[0]?.dataIndex ?? 0] ?? '';
      const lines = [`<b>${label}</b>`];
      for (const p of params) {
        if (p.value !== null && p.value !== undefined && p.value !== 0) {
          lines.push(`${p.marker}${p.seriesName}: ${fmtPrecise(p.value)}`);
        }
      }
      return lines.join('<br/>');
    };
  }

  function renderGrouped() {
    if (!chart || !grouped) return;
    const g = grouped;
    if (!g.labels.length) {
      chart.clear();
      return;
    }
    const base = {
      tooltip: { trigger: 'axis', hideDelay: 999999, transitionDuration: 0, formatter: groupedTooltip(g) },
      legend: { top: 0, left: 'center', type: 'scroll', itemWidth: 12, itemHeight: 8, textStyle: { fontSize: 11 } },
      textStyle: { color: chartText },
      grid: { left: 70, right: 80, top: 48, bottom: 44, containLabel: true },
      xAxis: { type: 'category', data: g.labels },
      dataZoom: [{ type: 'inside' }],
      color: PALETTE,
    };

    if (sub === 'ratio') {
      chart.setOption(
        {
          ...base,
          yAxis: {
            type: 'value',
            name: t('trend.metric.ratio'),
            nameGap: 14,
            nameTextStyle: { align: 'left' },
            axisLabel: { formatter: (v: number) => `${v}%` },
            splitLine: { lineStyle: { color: splitColor } },
          },
          series: g.names.map(
            (name): LineSeriesOption => ({
              type: 'line',
              name,
              smooth: true,
              symbolSize: 4,
              connectNulls: false,
              data: g.labels.map((label) => {
                const c = g.cell(name, label);
                if (!c || c.input === null || c.input <= 0 || c.cacheRead === null) return null;
                return (c.cacheRead / c.input) * 100;
              }),
            })
          ),
        },
        { notMerge: true }
      );
      return;
    }

    if (sub === 'input') {
      // 每个分组两层堆叠面积：缓存命中（浅色，底）+ 未命中（input - cache_read，上）。
      const series: LineSeriesOption[] = [];
      g.names.forEach((name, i) => {
        const color = PALETTE[i % PALETTE.length];
        series.push({
          type: 'line',
          name: `${name} · ${t('cards.cacheRead')}`,
          stack: `input-${name}`,
          smooth: true,
          symbol: 'none',
          lineStyle: { width: 1.5 },
          itemStyle: { color, opacity: 0.55 },
          areaStyle: { color, opacity: 0.45 },
          data: g.labels.map((label) => g.cell(name, label)?.cacheRead ?? 0),
        });
        series.push({
          type: 'line',
          name: `${name} · ${t('cards.cacheMiss')}`,
          stack: `input-${name}`,
          smooth: true,
          symbol: 'none',
          lineStyle: { width: 1.5 },
          itemStyle: { color },
          areaStyle: { color, opacity: 0.3 },
          data: g.labels.map((label) => {
            const c = g.cell(name, label);
            if (!c || c.input === null) return 0;
            return c.input - (c.cacheRead ?? 0);
          }),
        });
      });
      chart.setOption(
        {
          ...base,
          yAxis: {
            type: 'value',
            name: t('trend.metric.input'),
            nameGap: 14,
            nameTextStyle: { align: 'left' },
            axisLabel: { formatter: (v: number) => fmtSmart(v) },
            splitLine: { lineStyle: { color: splitColor } },
          },
          series,
        },
        { notMerge: true }
      );
      return;
    }

    const metric = sub === 'output' ? 'output' : 'total';
    chart.setOption(
      {
        ...base,
        yAxis: {
          type: 'value',
          name: sub === 'output' ? t('trend.metric.output') : t('trend.metric.total'),
          nameGap: 14,
          nameTextStyle: { align: 'left' },
          axisLabel: { formatter: (v: number) => fmtSmart(v) },
          splitLine: { lineStyle: { color: splitColor } },
        },
        series: g.names.map(
          (name): LineSeriesOption => ({
            type: 'line',
            name,
            smooth: true,
            symbolSize: 3,
            lineStyle: { width: 1.5 },
            data: g.labels.map((label) => (g.cell(name, label)?.[metric] as number | null) ?? 0),
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

      // Tooltip 持续显示：hideDelay 999999 防止自动隐藏；
      // 鼠标离开图表时立即手动隐藏（globalout 事件）。
      chart?.on('globalout', () => {
        chart?.dispatchAction({ type: 'hideTip' });
      });    render();
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
    void sub;
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
<div class="token-tabs">
  {#each subTabs as [id, label] (id)}
    <button type="button" class:active={sub === id} onclick={() => (sub = id)}>{label}</button>
  {/each}
</div>
<div bind:this={el} class="token-chart"></div>
<p class="dur-summary">
  {t('panel.durationSummary', {
    avg: fmtDurationShort(duration.avgMs),
    total: fmtDurationShort(duration.totalMs),
  })}
</p>

<style>
  /* 分组维度选择行（面板顶部，位于子图切换之上；维度为分段按钮组）。 */
  .dim-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 0 0;
    flex-wrap: wrap;
    font-size: 12.5px;
  }
  .dim-error {
    color: var(--danger);
    font-size: 12px;
    overflow-wrap: anywhere;
  }
  .token-tabs {
    display: flex;
    gap: 6px;
    padding: 6px 0 0;
    flex-wrap: wrap;
  }
  .token-tabs button {
    border: 1px solid var(--border);
    background: var(--bg-input);
    border-radius: 6px;
    padding: 3px 12px;
    cursor: pointer;
    font-size: 12.5px;
    color: var(--text-secondary);
  }
  .token-tabs button.active {
    background: var(--accent-bg);
    color: var(--accent);
    border-color: var(--accent);
    font-weight: 600;
  }
  .token-chart {
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
