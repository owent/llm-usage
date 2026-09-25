<script lang="ts">
  import { onMount } from 'svelte';
  import * as echarts from 'echarts/core';
  import { BarChart, LineChart } from 'echarts/charts';
  import { GridComponent, LegendComponent, TooltipComponent } from 'echarts/components';
  import { CanvasRenderer } from 'echarts/renderers';
  import type { PeriodDto } from '../lib/api';
  import { t, i18n, fmtNumber } from '../lib/i18n.svelte';

  echarts.use([BarChart, LineChart, GridComponent, LegendComponent, TooltipComponent, CanvasRenderer]);

  let { periods, granularity }: { periods: PeriodDto[]; granularity: 'day' | 'week' | 'month' } =
    $props();

  let el: HTMLDivElement;
  let chart: echarts.ECharts | null = null;

  function render() {
    if (!chart || !periods.length) return;
    const labels = periods.map((p) => {
      let label = p.label;
      if (p.in_progress) label += t('trend.inProgress');
      else if (p.partial_history) label += t('trend.partial');
      return label;
    });
    chart.setOption(
      {
        title: { text: t('trend.title'), left: 8, top: 4, textStyle: { fontSize: 13 } },
        tooltip: {
          trigger: 'axis',
          formatter: (params: { dataIndex: number }[]) => {
            const p = periods[params[0]?.dataIndex ?? 0];
            if (!p) return '';
            return [
              `${p.label} (${p.start_day} ~ ${p.end_day})`,
              `${t('trend.calls')}: ${fmtNumber(p.sums.call_count)}`,
              `${t('cards.input')}: ${fmtNumber(p.sums.input_total_known)}`,
              `${t('cards.output')}: ${fmtNumber(p.sums.output_total_known)}`,
              `${t('cards.total')}: ${fmtNumber(p.sums.total_tokens_known)}`,
            ].join('<br/>');
          },
        },
        legend: { top: 6, right: 8 },
        grid: { left: 64, right: 72, top: 40, bottom: 44 },
        xAxis: { type: 'category', data: labels },
        yAxis: [
          { type: 'value', name: t('trend.calls') },
          { type: 'value', name: t('trend.tokens'), splitLine: { show: false } },
        ],
        dataZoom: granularity === 'day' ? [{ type: 'inside' }] : [],
        series: [
          {
            type: 'bar',
            name: t('trend.calls'),
            data: periods.map((p) => p.sums.call_count),
          },
          {
            type: 'line',
            name: t('cards.total'),
            yAxisIndex: 1,
            data: periods.map((p) => Number(p.sums.total_tokens_known ?? 0)),
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
    return () => {
      window.removeEventListener('resize', onResize);
      chart?.dispose();
      chart = null;
    };
  });

  $effect(() => {
    void periods;
    void granularity;
    void i18n.locale;
    render();
  });
</script>

<div bind:this={el} class="trend"></div>

<style>
  .trend {
    width: 100%;
    height: 420px;
    margin-top: 8px;
  }
</style>
