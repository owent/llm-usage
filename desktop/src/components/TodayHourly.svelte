<script lang="ts">
  /**
   * 今日按小时趋势（任务 D8）：双 y 轴——调用次数（柱）+ token（折线）；
   * y 轴 fmtSmart 缩放、tooltip 精确值；tooltip 附输入/缓存命中/输出/会话明细。
   */
  import { onMount } from 'svelte';
  import * as echarts from 'echarts/core';
  import { BarChart, LineChart } from 'echarts/charts';
  import { GridComponent, LegendComponent, TooltipComponent } from 'echarts/components';
  import { CanvasRenderer } from 'echarts/renderers';
  import { t, i18n, fmtSmart, fmtPrecise } from '../lib/i18n.svelte';

  echarts.use([BarChart, LineChart, GridComponent, LegendComponent, TooltipComponent, CanvasRenderer]);

  let {
    hourly,
  }: {
    hourly: {
      hour: number;
      calls: number;
      total_tokens: string | null;
      input_total: string | null;
      cache_read: string | null;
      output_total: string | null;
      sessions: number;
      avg_duration_ms: string | null;
    }[];
  } = $props();

  let el: HTMLDivElement;
  let chart: echarts.ECharts | null = null;

  function render() {
    if (!chart) return;
    const hours = Array.from({ length: 24 }, (_, h) => `${String(h).padStart(2, '0')}:00`);
    const byHour = new Map(hourly.map((x) => [x.hour, x]));
    const calls = Array.from({ length: 24 }, (_, h) => byHour.get(h)?.calls ?? 0);
    const tokens = Array.from({ length: 24 }, (_, h) => Number(byHour.get(h)?.total_tokens ?? 0));
    chart.setOption(
      {
        tooltip: {
          trigger: 'axis',
          formatter: (params: { dataIndex: number }[]) => {
            const i = params[0]?.dataIndex ?? 0;
            const h = byHour.get(i);
            if (!h) return `${hours[i]}<br/>${t('trend.calls')}: 0`;
            return [
              `<b>${hours[i]}</b>`,
              `${t('trend.calls')}: ${fmtPrecise(h.calls)}`,
              `${t('cards.total')}: ${fmtPrecise(h.total_tokens)}`,
              `${t('cards.input')}: ${fmtPrecise(h.input_total)}`,
              `${t('cards.cacheRead')}: ${fmtPrecise(h.cache_read)}`,
              `${t('cards.output')}: ${fmtPrecise(h.output_total)}`,
              `${t('trend.sessions')}: ${fmtPrecise(h.sessions)}`,
            ].join('<br/>');
          },
        },
        legend: { top: 0, left: 8, type: 'scroll', itemWidth: 12, itemHeight: 8, textStyle: { fontSize: 11 } },
        grid: { left: 56, right: 80, top: 36, bottom: 28 },
        xAxis: { type: 'category', data: hours },
        yAxis: [
          {
            type: 'value',
            name: t('trend.calls'),
            axisLabel: { formatter: (v: number) => fmtSmart(v) },
          },
          {
            type: 'value',
            name: t('trend.tokens'),
            splitLine: { show: false },
            axisLabel: { formatter: (v: number) => fmtSmart(v) },
          },
        ],
        series: [
          { type: 'bar', name: t('trend.calls'), barMaxWidth: 18, itemStyle: { color: '#1a56c4' }, data: calls },
          {
            type: 'line',
            name: t('trend.tokens'),
            yAxisIndex: 1,
            smooth: true,
            symbolSize: 4,
            itemStyle: { color: '#3f8f5f' },
            data: tokens,
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
    margin-top: 4px;
  }
</style>
