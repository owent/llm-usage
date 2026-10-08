<script lang="ts">
  import { onMount } from 'svelte';
  import { setChartOption, setupTooltipAutoHide } from '../lib/chart';
  import * as echarts from 'echarts/core';
  import { BarChart } from 'echarts/charts';
  import { GridComponent, TooltipComponent } from 'echarts/components';
  import { SVGRenderer } from 'echarts/renderers';
  import { t, i18n, fmtSmart, fmtPrecise } from '../lib/i18n.svelte';
  import { api, parseError, type HeatmapDto, type SummaryQuery } from '../lib/api';

  echarts.use([BarChart, GridComponent, TooltipComponent, SVGRenderer]);

  let {
    cells = [], query, reloadKey = 0,
    isDark = false,
  }: {
    cells?: { weekday: number; calls: number; available?: boolean; partial?: boolean }[];
    query?: SummaryQuery;
    reloadKey?: number;
    /** Parent-provided dark theme; changes redraw axis labels and separators. */
    isDark?: boolean;
  } = $props();

  let el: HTMLDivElement;
  let chart: echarts.ECharts | null = null;
  let queriedCells = $state<HeatmapDto['cells']>([]);
  let loading = $state(false);
  let error = $state('');
  const activeCells = $derived(query ? queriedCells : cells);
  $effect(() => {
    const q = query;
    void reloadKey;
    if (!q) return;
    let cancelled = false;
    queriedCells = [];
    loading = true; error = '';
    api.heatmap({ ...q, granularity: 'day' }).then((result) => {
      if (!cancelled) queriedCells = result.cells;
    }).catch((e) => { if (!cancelled) error = parseError(e); })
      .finally(() => { if (!cancelled) loading = false; });
    return () => { cancelled = true; };
  });

  /** Theme colors for inherited axis text and y-axis separators. */
  const chartText = $derived(isDark ? '#b0bfd4' : '#5b6a82');
  const splitColor = $derived(isDark ? '#2c3a52' : '#e8edf5');

  // Monday=0 through Sunday=6; backend uses 1–7. Intl generates localized weekday names.
  // 2024-01-01 is Monday, the reference date for weekday names.
  const weekdayLabels = $derived(
    Array.from({ length: 7 }, (_, i) =>
      new Intl.DateTimeFormat(i18n.locale, { weekday: 'short', timeZone: 'UTC' }).format(
        new Date(Date.UTC(2024, 0, 1 + i))
      )
    )
  );

  const totals = $derived.by(() => {
    const sums: (number | null)[] = Array.from({ length: 7 }, () => null);
    for (const c of activeCells) {
      const idx = c.weekday - 1;
      if (idx >= 0 && idx < 7 && c.available !== false) sums[idx] = (sums[idx] ?? 0) + c.calls;
    }
    return sums;
  });

  function render() {
    if (!chart) return;
    setChartOption(chart, isDark,
      {
        tooltip: {
          trigger: 'axis',
          hideDelay: 0, transitionDuration: 0,
          formatter: (params: { dataIndex: number }[]) => {
            const i = params[0]?.dataIndex ?? 0;
            return `${weekdayLabels[i]}<br/>${t('trend.calls')}: ${fmtPrecise(totals[i])}`;
          },
        },
        grid: { left: 8, right: 16, top: 24, bottom: 8, containLabel: true },
        textStyle: { color: chartText },
        xAxis: { type: 'category', data: weekdayLabels, axisLabel: { fontSize: 11 } },
        yAxis: {
          type: 'value',
          axisLabel: { formatter: (v: number) => fmtSmart(v) },
          splitLine: { lineStyle: { color: splitColor } },
        },
        series: [
          {
            type: 'bar',
            name: t('trend.calls'),
            barMaxWidth: 26,
            itemStyle: { color: '#1a56c4' },
            data: totals,
          },
        ],
      },
      { notMerge: true }
    );
  }

  onMount(() => {
    chart = echarts.init(el, i18n.locale === 'zh-CN' ? 'ZH' : 'EN', { renderer: 'svg' });

    // Use hideDelay=0: ECharts 6 hideTip also uses hideLater. Hide on canvas/window exit or blur.
    const disposeTipHide = setupTooltipAutoHide(chart!);
    render();
    const onResize = () => chart?.resize();
    window.addEventListener('resize', onResize);
    // Resize the canvas when panel visibility or grid changes, including recovery from display:none.
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
    void cells;
    void i18n.locale;
    void isDark;
    render();
  });
</script>

<div bind:this={el} class="weekday"></div>
{#if loading}<p class="coverage">{t('common.loading')}</p>{/if}
{#if error}<p class="coverage" role="alert">{error}</p>{/if}
{#if activeCells.some((cell) => cell.available === false || cell.partial)}
  <p class="coverage">{t('trend.weekdayCoverage')}</p>
{/if}

<style>
  .weekday {
    width: 100%;
    height: 260px;
  }
  .coverage { color: var(--text-muted); font-size: 12px; margin: 2px 0 0; }
</style>
