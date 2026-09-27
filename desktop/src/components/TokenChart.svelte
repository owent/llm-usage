<script lang="ts">
  import { onMount } from 'svelte';
  import { setChartOption, setupTooltipAutoHide, escapeHtml, CHART_PALETTE } from '../lib/chart';
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
    onperiodclick,
  }: {
    periods: PeriodDto[];
    /** 当前查询（chart_series 分组数据用；随筛选/范围变化重新拉取）。 */
    query: SummaryQuery;
    granularity: 'hour' | 'day' | 'week' | 'month';
    /** 深色主题（父级传入；变化时重绘轴/legend 文字与分隔线）。 */
    isDark?: boolean;
    /** 点击数据点回调（携带该点的时间轴标签；总用量/分组两模式均生效）。 */
    onperiodclick?: (label: string) => void;
  } = $props();

  let sub = $state<TokenSub>('total');
  let dimension = $state<ChartDimension>('total');
  let grouped = $state<ChartGroupData | null>(null);
  let groupedError = $state('');
  let groupedScope = '';

  let el: HTMLDivElement;
  let chart: echarts.ECharts | null = null;

  /**
   * 增量渲染（2026-09-26 用户反馈）：渲染结构签名（子图/维度/标签/系列名/主题/
   * 语言）与上次相同 → setOption 合并更新（ECharts 内部 diff，不整图重建）；
   * 结构变化 → notMerge 整体重建。数据刷新时避免可见闪烁。
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

    const base = {
      tooltip: {
        trigger: 'axis',
        hideDelay: 0, transitionDuration: 0,
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
              `${t('cards.cacheWrite')}: ${fmtPrecise(p.sums.cache_write_known)}`,
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
      applyOption(
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
        `total-${sub}|${i18n.locale}|${chartText}|${granularity}|${labels.join('\u0001')}`
      );
      return;
    }

    if (sub === 'input') {
      // 输入 token：缓存命中（底）+ 未命中（上）堆叠面积图。
      applyOption(
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
          series: ([
            ['cache_read_known', 'cards.cacheRead', '#21a590'],
            ['cache_write_known', 'cards.cacheWrite', '#eda853'],
            ['uncached_known', 'cards.cacheMiss', '#5470e8'],
          ] as const).map(([field, label, color]) => ({
            type: 'line', name: t(label), stack: 'input', smooth: true,
            symbol: 'none', lineStyle: { width: 1.5 }, itemStyle: { color },
            areaStyle: { opacity: 0.25 },
            data: periods.map((p) => p.sums[field] === null ? null : Number(p.sums[field])),
          })),
        },
        `total-${sub}|${i18n.locale}|${chartText}|${granularity}|${labels.join('\u0001')}`
      );
      return;
    }

    const metric = sub === 'output' ? 'output' : 'total';
    applyOption(
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
                ? p.sums.output_total_known === null ? null : Number(p.sums.output_total_known)
                : p.sums.total_tokens_known === null ? null : Number(p.sums.total_tokens_known)
            ),
          },
        ],
      },
      `total-${sub}|${i18n.locale}|${chartText}|${granularity}|${labels.join('\u0001')}`
    );
  }

  /** 分组模式通用 tooltip：按系列显示精确值（null 点不参与）。 */
  function groupedTooltip(g: ChartGroupData) {
    return (params: { dataIndex: number; marker: string; seriesName?: string; value: number | null }[]) => {
      const label = g.labels[params[0]?.dataIndex ?? 0] ?? '';
      const lines = [`<b>${escapeHtml(label)}</b>`];
      for (const p of params) {
        if (p.value !== null && p.value !== undefined && p.value !== 0) {
          lines.push(`${p.marker}${escapeHtml(p.seriesName)}: ${fmtPrecise(p.value)}`);
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
      lastRenderKey = '';
      return;
    }
    const base = {
      tooltip: { trigger: 'axis', hideDelay: 0, transitionDuration: 0, formatter: groupedTooltip(g) },
      legend: { top: 0, left: 'center', type: 'scroll', itemWidth: 12, itemHeight: 8, textStyle: { fontSize: 11 } },
      textStyle: { color: chartText },
      grid: { left: 70, right: 80, top: 48, bottom: 44, containLabel: true },
      xAxis: { type: 'category', data: g.labels },
      dataZoom: [{ type: 'inside' }],
      color: PALETTE,
    };

    if (sub === 'ratio') {
      applyOption(
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
                return c?.cacheRatio == null ? null : c.cacheRatio * 100;
              }),
            })
          ),
        },
        `grp-${sub}|${dimension}|${i18n.locale}|${chartText}|${g.labels.join('\u0001')}|${g.names.join('\u0001')}`
      );
      return;
    }

    if (sub === 'input') {
      const series: LineSeriesOption[] = g.names.flatMap((name, i) =>
        ([
          ['cacheRead', 'cards.cacheRead', 0.45],
          ['cacheWrite', 'cards.cacheWrite', 0.7],
          ['uncached', 'cards.cacheMiss', 1],
        ] as const).map(([field, label, opacity]) => ({
          type: 'line', name: `${name} · ${t(label)}`, stack: `input-${name}`,
          smooth: true, symbol: 'none', lineStyle: { width: 1.5 },
          itemStyle: { color: PALETTE[i % PALETTE.length], opacity },
          areaStyle: { opacity: 0.25 },
          data: g.labels.map((label) => g.cell(name, label)?.[field] ?? null),
        }))
      );
      applyOption(
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
        `grp-${sub}|${dimension}|${i18n.locale}|${chartText}|${g.labels.join('\u0001')}|${g.names.join('\u0001')}`
      );
      return;
    }

    const metric = sub === 'output' ? 'output' : 'total';
    applyOption(
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
            data: g.labels.map((label) => (g.cell(name, label)?.[metric] as number | null) ?? null),
          })
        ),
      },
      `grp-${sub}|${dimension}|${i18n.locale}|${chartText}|${g.labels.join('\u0001')}|${g.names.join('\u0001')}`
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
    chart?.getZr().on('click', (e: { offsetX: number; offsetY: number }) => {
      if (!onperiodclick || !chart) return;
      const labels =
        dimension === 'total' ? periods.map((p) => p.label) : (grouped?.labels ?? []);
      if (!labels.length) return;
      try {
        if (!chart.containPixel('grid', [e.offsetX, e.offsetY])) return;
        const raw = chart.convertFromPixel({ xAxisIndex: 0 }, e.offsetX);
        if (typeof raw !== 'number' || !Number.isFinite(raw)) return;
        const idx = Math.max(0, Math.min(labels.length - 1, Math.round(raw)));
        if (labels[idx]) {
          onperiodclick(labels[idx]);
          // 点击即选点：汇总条展开使布局位移，tooltip 位置随即过期，主动隐藏。
          chart.dispatchAction({ type: 'hideTip' });
        }
      } catch {
        /* 选项未就绪/像素转换失败时忽略点击 */
      }
    });
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
    font-size: 13px;
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
    font-size: 13px;
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
