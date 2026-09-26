<script lang="ts">
  import { onMount } from 'svelte';
  import * as echarts from 'echarts/core';
  import { PieChart } from 'echarts/charts';
  import { LegendComponent, TooltipComponent } from 'echarts/components';
  import { CanvasRenderer } from 'echarts/renderers';
  import { t, i18n, fmtPrecise } from '../lib/i18n.svelte';

  echarts.use([PieChart, LegendComponent, TooltipComponent, CanvasRenderer]);

  let { data }: { data: { name: string; value: number }[] } = $props();

  let el: HTMLDivElement;
  let chart: echarts.ECharts | null = null;

  const PALETTE = [
    '#1a56c4', '#3f8f5f', '#c9a227', '#b3601e', '#7a5fb0',
    '#2f8f8f', '#c46a9a', '#8a8f36', '#5d6b9e', '#a05f46',
  ];

  function render() {
    if (!chart) return;
    chart.setOption(
      {
        tooltip: {
          trigger: 'item',
          formatter: (p: { name: string; value: number; percent: number }) =>
            `${p.name}: ${fmtPrecise(p.value)} (${p.percent}%)`,
        },
        legend: {
          type: 'scroll',
          orient: 'vertical',
          right: 0,
          top: 'middle',
          itemWidth: 12,
          itemHeight: 8,
          textStyle: { fontSize: 11, width: 120, overflow: 'truncate' },
        },
        color: PALETTE,
        series: [
          {
            type: 'pie',
            radius: ['0%', '62%'],
            center: ['32%', '52%'],
            data,
            label: { show: false },
            emphasis: { label: { show: false } },
          },
        ],
      },
      { notMerge: true }
    );
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
    void data;
    void i18n.locale;
    render();
  });
</script>

{#if data.length === 0}
  <p class="muted">{t('common.empty')}</p>
{/if}
<div bind:this={el} class="pie" class:collapsed={data.length === 0}></div>

<style>
  .pie {
    width: 100%;
    height: 300px;
  }
  .pie.collapsed {
    height: 4px;
  }
  .muted {
    color: #666;
    font-size: 12px;
  }
</style>
