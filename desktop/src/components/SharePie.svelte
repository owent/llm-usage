<script lang="ts">
  import { onMount } from 'svelte';
  import { mergeNamedValues } from '../lib/derive';
  import { hideTooltipOnBlank, setChartOption, setupTooltipAutoHide, escapeHtml, CHART_PALETTE } from '../lib/chart';
  import * as echarts from 'echarts/core';
  import { PieChart } from 'echarts/charts';
  import { LegendComponent, TooltipComponent } from 'echarts/components';
  import { SVGRenderer } from 'echarts/renderers';
  import { t, i18n, fmtPrecise } from '../lib/i18n.svelte';

  echarts.use([PieChart, LegendComponent, TooltipComponent, SVGRenderer]);

  let {
    data: rawData,
    isDark = false,
    height = 280,
  }: {
    data: { name: string; value: number | null; input?: number | null; output?: number | null }[];
    /** Parent-provided dark theme; changes redraw legend text. */
    isDark?: boolean;
    /** Canvas height in pixels; no data collapses it to a thin strip. */
    height?: number;
  } = $props();
  let metric = $state<'total' | 'input' | 'output'>('total');
  const selectMetric = $derived(rawData.some((r) => r.input !== undefined || r.output !== undefined));
  const values = $derived(rawData.map((r) => ({ name: r.name, value: metric === 'total' ? r.value : r[metric] ?? null })));
  const data = $derived(mergeNamedValues(values.filter((r): r is {name: string; value: number} => r.value !== null && r.value > 0)));
  const missing = $derived(values.filter((r) => r.value === null).map((r) => r.name));

  let el: HTMLDivElement;
  let chart: echarts.ECharts | null = null;

  /** Theme text color inherited by the legend. */
  const chartText = $derived(isDark ? '#b0bfd4' : '#5b6a82');

  const PALETTE = CHART_PALETTE;

  /**
   * Incremental rendering, feedback on 2026-09-26: unchanged names/theme/language/height
   * uses merged setOption updates, animating values without recreating the canvas.
   * A changed structure rebuilds through notMerge.
   */
  let lastRenderKey = '';

  function render() {
    if (!chart) return;
    const key = `${i18n.locale}|${chartText}|${height}|${data.map((d) => d.name).join('\u0001')}`;
    if (key === lastRenderKey) setChartOption(chart, isDark, pieOption());
    else setChartOption(chart, isDark, pieOption(), { notMerge: true });
    lastRenderKey = key;
  }

  function pieOption(): echarts.EChartsCoreOption {
    const radius = Math.max(12, Math.min(chart?.getWidth() ?? 280, height - 60) / 2 - 16);
    return {
      tooltip: {
        trigger: 'item',
        hideDelay: 0, transitionDuration: 0,
        formatter: (p: { name: string; value: number; percent: number }) =>
          `${escapeHtml(p.name)}: ${fmtPrecise(p.value)} (${p.percent}%)`,
      },
      legend: {
        type: 'scroll',
        orient: 'horizontal',
        left: 'center',
        bottom: 0,
        itemWidth: 12,
        itemHeight: 8,
        textStyle: { fontSize: 12, width: 120, overflow: 'truncate', color: chartText },
      },
      color: PALETTE,
      series: [
        {
          type: 'pie',
          radius: [radius * 0.65, radius],
          center: ['50%', (height - 36) / 2],
          data,
          padAngle: 2,
          itemStyle: { borderRadius: 5 },
          label: { show: false },
          emphasis: { label: { show: false } },
        },
      ],
    };
  }

  onMount(() => {
    chart = echarts.init(el, i18n.locale === 'zh-CN' ? 'ZH' : 'EN', { renderer: 'svg' });

    // hideDelay=0 gives immediate hiding; ECharts 6 manual hideTip also honors that delay. Hide on canvas/window exit or blur.
    // Item-triggered charts hide immediately over blank canvas without a hit.
    const disposeTipHide = setupTooltipAutoHide(chart!);
    const disposeBlankHide = hideTooltipOnBlank(chart!);
    render();
    const onResize = () => { chart?.resize(); render(); };
    window.addEventListener('resize', onResize);
    // Resize when panel visibility/grid changes, including return from display:none.
    const observer = new ResizeObserver(onResize);
    observer.observe(el);
    return () => {
      disposeTipHide();
      disposeBlankHide();
      observer.disconnect();
      window.removeEventListener('resize', onResize);
      chart?.dispose();
      chart = null;
    };
  });

  $effect(() => {
    void data;
    void i18n.locale;
    void isDark;
    render();
  });
</script>

{#if selectMetric}
  <div class="metrics" aria-label={t('dashboard.shareMetric')}>
    {#each ['total', 'input', 'output'] as id}
      <button type="button" class:active={metric === id} onclick={() => metric = id as typeof metric}>{t('cards.' + id)}</button>
    {/each}
  </div>
{/if}
{#if missing.length}<p class="muted">{t('dashboard.shareUnknown', {names: missing.join(', ')})}</p>{/if}
{#if data.length === 0}
  <p class="muted">{t('common.empty')}</p>
{/if}
<div bind:this={el} class="pie" style:height="{data.length === 0 ? 4 : height}px"></div>

<style>
  .metrics { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 6px; }
  button { cursor: pointer; border: 1px solid var(--border); border-radius: 6px; background: var(--bg-input); color: var(--text-secondary); padding: 3px 8px; font-size: 12px; }
  button.active { color: var(--accent); background: var(--accent-bg); border-color: var(--accent); }
  .pie {
    width: 100%;
  }
  .muted {
    color: var(--text-muted);
    font-size: 12px;
  }
</style>
