<script lang="ts">
  import { onMount } from 'svelte';
  import * as echarts from 'echarts/core';
  import { BarChart } from 'echarts/charts';
  import { GridComponent, TooltipComponent } from 'echarts/components';
  import { CanvasRenderer } from 'echarts/renderers';
  import { t, i18n, fmtSmart, fmtPrecise } from '../lib/i18n.svelte';

  echarts.use([BarChart, GridComponent, TooltipComponent, CanvasRenderer]);

  let {
    cells,
    isDark = false,
  }: {
    cells: { weekday: number; calls: number }[];
    /** 深色主题（父级传入；变化时重绘轴文字与分隔线）。 */
    isDark?: boolean;
  } = $props();

  let el: HTMLDivElement;
  let chart: echarts.ECharts | null = null;

  /** 主题感知色：全局文字（axis 继承）与 y 轴分隔线。 */
  const chartText = $derived(isDark ? '#aaa' : '#555');
  const splitColor = $derived(isDark ? '#3a3b3f' : '#e0e0e0');

  // 周一=0 … 周日=6（后端 weekday 1–7）；周名按当前语言 Intl 生成。
  // 2024-01-01 是周一，作为周名基准日。
  const weekdayLabels = $derived(
    Array.from({ length: 7 }, (_, i) =>
      new Intl.DateTimeFormat(i18n.locale, { weekday: 'short' }).format(
        new Date(Date.UTC(2024, 0, 1 + i))
      )
    )
  );

  const totals = $derived.by(() => {
    const sums = Array.from({ length: 7 }, () => 0);
    for (const c of cells) {
      const idx = c.weekday - 1;
      if (idx >= 0 && idx < 7) sums[idx] += c.calls;
    }
    return sums;
  });

  function render() {
    if (!chart) return;
    chart.setOption(
      {
        tooltip: {
          trigger: 'axis',
          hideDelay: 999999, transitionDuration: 0,
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
    void cells;
    void i18n.locale;
    void isDark;
    render();
  });
</script>

<div bind:this={el} class="weekday"></div>

<style>
  .weekday {
    width: 100%;
    height: 260px;
  }
</style>
