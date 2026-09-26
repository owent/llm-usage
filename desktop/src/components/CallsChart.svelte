<script lang="ts">
  /**
   * 次数趋势图（任务 C5）：调用次数（折线，2026-09-26 由柱改线）+ 会话数（折线）
   * 双 y 轴，分组维度多系列也全部为平滑折线；
   * y 轴单位自动缩放（fmtSmart），tooltip 显示千分位精确值（任务 B3）；
   * 面板底部带平均耗时/总耗时文字摘要（任务 C6）。
   * 分组维度（2026-09-26）：总用量/按模型/按Agent/按Agent+模型；分组数据经
   * chart_series 命令从聚合表直读（低计算量），多系列 legend + 按系列 tooltip；
   * 维度选择为分段按钮组（DimensionPicker，非下拉）。
   * 布局修复：grid.left ≥ 70 给 y 轴标签/轴名留位，legend 顶部居中
   * （top 0 + left center），不再与左上 y 轴名重叠；双 y 轴首轴名左对齐 +
   * nameGap 拉开距离。
   */
  import { onMount } from 'svelte';
  import { setupTooltipAutoHide } from '../lib/chart';
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

  let dimension = $state<ChartDimension>('total');
  let grouped = $state<ChartGroupData | null>(null);
  let groupedError = $state('');

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
    if (key === lastRenderKey) chart.setOption(option);
    else chart.setOption(option, { notMerge: true });
    lastRenderKey = key;
  }

  const duration = $derived(durationStatsOf(periods));

  /** 主题感知色：全局文字（axis/legend 继承）与 y 轴分隔线。 */
  const chartText = $derived(isDark ? '#aaa' : '#555');
  const splitColor = $derived(isDark ? '#3a3b3f' : '#e0e0e0');

  const PALETTE = [
    '#1a56c4', '#3f8f5f', '#c9a227', '#b3601e', '#7a5fb0',
    '#2f8f8f', '#c46a9a', '#8a8f36', '#5d6b9e', '#a05f46',
  ];

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
            const lines = [`<b>${label}</b>`];
            for (const p of params) {
              if (p.value) lines.push(`${p.marker}${p.seriesName}: ${fmtPrecise(p.value)}`);
            }
            return lines.join('<br/>');
          },
        },
        legend: { top: 0, left: 'center', type: 'scroll', itemWidth: 12, itemHeight: 8, textStyle: { fontSize: 11 } },
        textStyle: { color: chartText },
        grid: { left: 70, right: 80, top: 48, bottom: 44, containLabel: true },
        xAxis: { type: 'category', data: g.labels },
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
    font-size: 12.5px;
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
