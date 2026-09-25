<script lang="ts">
  import { onMount } from 'svelte';
  import * as echarts from 'echarts/core';
  import { BarChart } from 'echarts/charts';
  import { GridComponent, TooltipComponent } from 'echarts/components';
  import { CanvasRenderer } from 'echarts/renderers';
  import { t, i18n, fmtNumber } from '../lib/i18n.svelte';

  echarts.use([BarChart, GridComponent, TooltipComponent, CanvasRenderer]);

  let {
    hourly,
  }: {
    hourly: { hour: number; calls: number; total_tokens: string | null }[];
  } = $props();

  let el: HTMLDivElement;
  let chart: echarts.ECharts | null = null;

  function render() {
    if (!chart) return;
    const hours = Array.from({ length: 24 }, (_, h) => `${String(h).padStart(2, '0')}:00`);
    const calls = Array.from({ length: 24 }, (_, h) => hourly.find((x) => x.hour === h)?.calls ?? 0);
    const tokens = Array.from({ length: 24 }, (_, h) =>
      Number(hourly.find((x) => x.hour === h)?.total_tokens ?? 0)
    );
    chart.setOption({
      title: { text: t('hourly.title'), left: 8, top: 4, textStyle: { fontSize: 13 } },
      tooltip: {
        trigger: 'axis',
        formatter: (params: { dataIndex: number }[]) => {
          const i = params[0]?.dataIndex ?? 0;
          return `${hours[i]}<br/>${t('trend.calls')}: ${fmtNumber(calls[i])}<br/>${t('trend.tokens')}: ${fmtNumber(hourly.find((x) => x.hour === i)?.total_tokens ?? null)}`;
        },
      },
      grid: { left: 56, right: 56, top: 40, bottom: 28 },
      xAxis: { type: 'category', data: hours },
      yAxis: [
        { type: 'value', name: t('trend.calls') },
        { type: 'value', name: t('trend.tokens'), splitLine: { show: false } },
      ],
      series: [
        { type: 'bar', name: t('trend.calls'), data: calls },
        {
          type: 'bar',
          name: t('trend.tokens'),
          yAxisIndex: 1,
          data: tokens,
          itemStyle: { opacity: 0.45 },
        },
      ],
    });
  }

  onMount(() => {
    chart = echarts.init(el, i18n.locale === 'zh-CN' ? 'ZH' : 'EN');
    render();
    const onResize = () => chart?.resize();
    window.addEventListener('resize', onResize);
    return () => {
      window.removeEventListener('resize', onResize);
      chart?.dispose();
      chart = null;
    };
  });

  // 数据或语言变化时重绘。
  $effect(() => {
    void hourly;
    void i18n.locale;
    render();
  });
</script>

<div bind:this={el} class="hourly"></div>

<style>
  .hourly {
    width: 100%;
    height: 280px;
    margin-top: 8px;
  }
</style>
