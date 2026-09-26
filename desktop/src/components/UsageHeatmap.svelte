<script lang="ts">
  import { onMount } from 'svelte';
  import * as echarts from 'echarts/core';
  import { HeatmapChart } from 'echarts/charts';
  import { GridComponent, TooltipComponent, VisualMapComponent } from 'echarts/components';
  import { CanvasRenderer } from 'echarts/renderers';
  import { api } from '../lib/api';
  import type { SummaryQuery } from '../lib/api';
  import { t, i18n, fmtPrecise } from '../lib/i18n.svelte';

  echarts.use([HeatmapChart, GridComponent, TooltipComponent, VisualMapComponent, CanvasRenderer]);

  let {
    query,
    reloadKey = 0,
    oncells,
  }: {
    query: SummaryQuery;
    /** 用户切换/导入等不改变 query 的强制重查信号。 */
    reloadKey?: number;
    /** 载入后把 cells 交给父级（周分布图复用同一数据）。 */
    oncells?: (cells: { weekday: number; hour: number; calls: number; total_tokens: string | null }[]) => void;
  } = $props();

  let el: HTMLDivElement;
  let chart: echarts.ECharts | null = null;
  let cells = $state<{ weekday: number; hour: number; calls: number; total_tokens: string | null }[]>([]);
  let failed = $state(false);

  const weekdayLabels = ['一', '二', '三', '四', '五', '六', '日'];
  const hours = Array.from({ length: 24 }, (_, h) => h);

  async function load() {
    try {
      const r = await api.heatmap(query);
      cells = r.cells;
      failed = false;
      oncells?.(r.cells);
    } catch {
      failed = true;
    }
  }

  function render() {
    if (!chart) return;
    const data = cells.map((c) => [c.hour, c.weekday - 1, c.calls]);
    const max = Math.max(1, ...cells.map((c) => c.calls));
    chart.setOption(
      {
        tooltip: {
          formatter: (p: { value: [number, number, number] }) => {
            const cell = cells.find((c) => c.hour === p.value[0] && c.weekday - 1 === p.value[1]);
            return `${weekdayLabels[p.value[1]]} ${String(p.value[0]).padStart(2, '0')}:00<br/>${t('trend.calls')}: ${fmtPrecise(p.value[2])}<br/>${t('trend.tokens')}: ${fmtPrecise(cell?.total_tokens ?? null)}`;
          },
        },
        grid: { left: 44, right: 24, top: 16, bottom: 60 },
        xAxis: { type: 'category', data: hours.map(String), splitArea: { show: true } },
        yAxis: { type: 'category', data: weekdayLabels, splitArea: { show: true } },
        visualMap: {
          min: 0,
          max,
          calculable: false,
          orient: 'horizontal',
          left: 'center',
          bottom: 8,
          inRange: { color: ['#f2f6fc', '#9db8e8', '#1a56c4'] },
        },
        series: [
          {
            type: 'heatmap',
            data,
            emphasis: { itemStyle: { shadowBlur: 4 } },
          },
        ],
      },
      { notMerge: true }
    );
  }

  onMount(() => {
    chart = echarts.init(el, i18n.locale === 'zh-CN' ? 'ZH' : 'EN');
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

  let timer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    void query;
    void reloadKey;
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => void load(), 300);
  });
  $effect(() => {
    void cells;
    void i18n.locale;
    render();
  });
</script>

{#if failed}
  <p class="muted">—</p>
{/if}
<div bind:this={el} class="heatmap"></div>

<style>
  .heatmap {
    width: 100%;
    height: 260px;
    margin-top: 8px;
  }
  .muted {
    color: #999;
    font-size: 12px;
  }
</style>
