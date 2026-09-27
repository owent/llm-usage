<script lang="ts">
  import { onMount } from 'svelte';
  import { mergeNamedValues } from '../lib/derive';
  import { hideTooltipOnBlank, setChartOption, setupTooltipAutoHide, escapeHtml, CHART_PALETTE } from '../lib/chart';
  import * as echarts from 'echarts/core';
  import { PieChart } from 'echarts/charts';
  import { LegendComponent, TooltipComponent } from 'echarts/components';
  import { CanvasRenderer } from 'echarts/renderers';
  import { t, i18n, fmtPrecise } from '../lib/i18n.svelte';

  echarts.use([PieChart, LegendComponent, TooltipComponent, CanvasRenderer]);

  let {
    data: rawData,
    isDark = false,
    height = 280,
  }: {
    data: { name: string; value: number }[];
    /** 深色主题（父级传入；变化时重绘 legend 文字）。 */
    isDark?: boolean;
    /** 画布高度（px；无数据时折叠为细条）。 */
    height?: number;
  } = $props();
  const data = $derived(mergeNamedValues(rawData));

  let el: HTMLDivElement;
  let chart: echarts.ECharts | null = null;

  /** 主题感知色：全局文字（legend 继承）。 */
  const chartText = $derived(isDark ? '#b0bfd4' : '#5b6a82');

  const PALETTE = CHART_PALETTE;

  /**
   * 增量渲染（2026-09-26 用户反馈）：构成（名称集合/主题/语言/高度）未变 →
   * setOption 合并更新（数值变化原地动画过渡，不重建画布）；构成变化 →
   * notMerge 重建。
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
    return {
      tooltip: {
        trigger: 'item',
        hideDelay: 0, transitionDuration: 0,
        formatter: (p: { name: string; value: number; percent: number }) =>
          `${escapeHtml(p.name)}: ${fmtPrecise(p.value)} (${p.percent}%)`,
      },
      legend: {
        type: 'scroll',
        orient: 'vertical',
        right: 0,
        top: 'middle',
        itemWidth: 12,
        itemHeight: 8,
        textStyle: { fontSize: 12, width: 140, overflow: 'truncate', color: chartText },
      },
      color: PALETTE,
      series: [
        {
          type: 'pie',
          radius: ['44%', '68%'],
          center: ['32%', '52%'],
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
    chart = echarts.init(el, i18n.locale === 'zh-CN' ? 'ZH' : 'EN');

    // Tooltip 持续显示（hideDelay 0；ECharts 6 的手动 hideTip 同样被 hideDelay 延迟，不可用大值）+ 离开画布/移出窗口/失焦即隐藏（统一封装）；
    // item 触发：图内空白处（无命中图形）也立即隐藏。
    const disposeTipHide = setupTooltipAutoHide(chart!);
    const disposeBlankHide = hideTooltipOnBlank(chart!);
    render();
    const onResize = () => chart?.resize();
    window.addEventListener('resize', onResize);
    // 面板显示/隐藏或网格变化时容器尺寸变化（含 display:none 恢复），自动重设画布。
    const observer = new ResizeObserver(() => chart?.resize());
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

{#if data.length === 0}
  <p class="muted">{t('common.empty')}</p>
{/if}
<div bind:this={el} class="pie" style:height="{data.length === 0 ? 4 : height}px"></div>

<style>
  .pie {
    width: 100%;
  }
  .muted {
    color: var(--text-muted);
    font-size: 12px;
  }
</style>
